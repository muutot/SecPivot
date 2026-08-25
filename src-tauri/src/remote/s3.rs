//! S3 transport: hand-rolled AWS Signature V4 over the shared blocking
//! reqwest/rustls client (no rust-s3 / native-tls / openssl dependency).
//! Path-style URLs so custom endpoints such as MinIO work.
//!
//! Only the four operations the app needs are implemented: `ListObjectsV2`
//! (list), `GetObject` (get), `PutObject` (put), plus `CreateBucket` /
//! `DeleteObject` / `DeleteBucket` used by the live-server round-trip test.

use super::shared_blocking_client;
use super::RemoteObject;
use super::RemoteStorage;
use super::{REMOTE_CONFLICT_MARKER, REMOTE_IO_TIMEOUT, REMOTE_LIST_TIMEOUT};
use crate::config::RemoteSettings;
use crate::crypto::{hex, hmac_sha256, sha256_bytes};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::time::Duration;
use url::Url;
// ---------------------------------------------------------------------------
// S3 transport (SigV4 over blocking reqwest/rustls, path-style)
// ---------------------------------------------------------------------------

/// S3 storage against one bucket on one endpoint. Object keys are vault
/// paths like `vaults/a.kdbx`; every request is signed with SigV4 and sent
/// path-style (`{endpoint}/{bucket}/{key}`).
pub struct S3Storage {
    client: reqwest::blocking::Client,
    /// `scheme://host[:port]` with no trailing slash.
    endpoint: String,
    region: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    list_timeout: Duration,
    io_timeout: Duration,
}

impl S3Storage {
    pub fn new(cfg: &RemoteSettings) -> Result<Self, String> {
        Self::with_timeouts(cfg, REMOTE_LIST_TIMEOUT, REMOTE_IO_TIMEOUT)
    }

    pub(crate) fn with_timeouts(
        cfg: &RemoteSettings,
        list_timeout: Duration,
        io_timeout: Duration,
    ) -> Result<Self, String> {
        let endpoint = cfg.endpoint.trim();
        let region = cfg.region.trim();
        let bucket_name = cfg.bucket.trim();
        let access_key = cfg.access_key.trim();
        let secret_key = cfg.secret_key.trim();
        if endpoint.is_empty() {
            return Err("请先在设置中配置 S3 服务地址".to_owned());
        }
        // The endpoint must parse as an absolute http(s) URL: SigV4 signs the
        // `host` header from it and every request is sent to
        // `{endpoint}/{bucket}/{key}`. Rejecting scheme-less strings here turns
        // a would-be panic in `host_header` (or a cryptic reqwest error) into a
        // clear settings-time message.
        let parsed_endpoint = Url::parse(endpoint);
        let has_http_scheme_and_host = parsed_endpoint
            .as_ref()
            .map(|u| (u.scheme() == "http" || u.scheme() == "https") && u.host_str().is_some())
            .unwrap_or(false);
        if !has_http_scheme_and_host {
            return Err(format!(
                "S3 服务地址无效: {endpoint}（需要形如 https://host:port 的完整地址）"
            ));
        }
        if region.is_empty() {
            return Err("请先在设置中配置 S3 区域".to_owned());
        }
        if bucket_name.is_empty() {
            return Err("请先在设置中配置 S3 存储桶".to_owned());
        }
        // An empty or `/`-containing access key would emit a malformed
        // `X-Amz-Credential` (SigV4 splits it on `/`) and fail with a cryptic
        // AuthorizationQueryParametersError HTTP 400. Fail early with a clear
        // message instead; AWS access keys never contain `/`.
        if access_key.is_empty() {
            return Err("请先在设置中配置 S3 Access Key".to_owned());
        }
        if access_key.contains('/') {
            return Err("S3 Access Key 无效：包含非法字符 `/`".to_owned());
        }
        if secret_key.is_empty() {
            return Err("请先在设置中配置 S3 Secret Key".to_owned());
        }
        Ok(Self {
            client: shared_blocking_client()?.clone(),
            endpoint: endpoint.trim_end_matches('/').to_owned(),
            region: region.to_owned(),
            bucket: bucket_name.to_owned(),
            access_key: access_key.to_owned(),
            secret_key: secret_key.to_owned(),
            list_timeout,
            io_timeout,
        })
    }

    /// Percent-encode per RFC 3986, keeping unreserved chars and optionally `/`.
    /// AWS SigV4 canonical query values and path segments forbid `+`/space, so
    /// spaces must become `%20` (never `+`).
    fn encode(s: &str, keep_slash: bool) -> String {
        let mut out = String::with_capacity(s.len());
        for byte in s.bytes() {
            let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
            if keep || (byte == b'/' && keep_slash) {
                out.push(byte as char);
            } else {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
        out
    }

    /// `host[:port]` header value matching the configured endpoint. Returns an
    /// error instead of panicking so a malformed stored endpoint can never take
    /// down a request (construction validates the endpoint, but the stored
    /// `ConfigStore` value is user-editable at any time).
    pub(crate) fn host_header(&self) -> Result<String, String> {
        let url = Url::parse(&self.endpoint)
            .map_err(|e| format!("S3 服务地址无效: {}（{e}）", self.endpoint))?;
        match url.port() {
            Some(port) => Ok(format!("{}:{port}", url.host_str().unwrap_or_default())),
            None => Ok(url.host_str().unwrap_or_default().to_owned()),
        }
    }

    /// Sign and send one request, returning the raw response. `path` is the
    /// canonical (already encoded) request path starting with `/`; `query` is
    /// a sorted list of (name, value) pairs encoded here. `extra_headers` are
    /// sent but not signed (SigV4 only requires `host`/`x-amz-*` in the
    /// canonical request; e.g. `If-Match` stays unsigned).
    fn send(
        &self,
        method: &str,
        path: &str,
        query: &[(&str, &str)],
        body: &[u8],
        timeout: Duration,
        extra_headers: &[(&str, &str)],
    ) -> Result<reqwest::blocking::Response, String> {
        let now = chrono::Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let mut pairs: Vec<String> = query
            .iter()
            .map(|(k, v)| format!("{}={}", Self::encode(k, false), Self::encode(v, false)))
            .collect();
        pairs.sort();
        let canonical_query = pairs.join("&");

        let payload_hash = hex(&sha256_bytes(body));

        let host = self.host_header()?;
        let canonical_headers =
            format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "{method}\n{path}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );

        let scope = format!("{date_stamp}/{}/s3/aws4_request", self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            hex(&sha256_bytes(canonical_request.as_bytes()))
        );

        let k_date = hmac_sha256(
            format!("AWS4{}", self.secret_key).as_bytes(),
            date_stamp.as_bytes(),
        );
        let k_region = hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256(&k_region, b"s3");
        let k_signing = hmac_sha256(&k_service, b"aws4_request");
        let signature = hex(&hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key
        );

        let mut url = format!("{}{}", self.endpoint, path);
        if !canonical_query.is_empty() {
            url.push('?');
            url.push_str(&canonical_query);
        }

        let mut request = self
            .client
            .request(
                reqwest::Method::from_bytes(method.as_bytes()).expect("valid method"),
                &url,
            )
            .header("x-amz-content-sha256", &payload_hash)
            .header("x-amz-date", &amz_date)
            .header("Authorization", authorization)
            .timeout(timeout);
        if !body.is_empty() {
            request = request
                .header("Content-Type", "application/octet-stream")
                .body(body.to_vec());
        }
        for (name, value) in extra_headers {
            request = request.header(*name, *value);
        }
        request
            .send()
            .map_err(|e| Self::map_error(method, e, timeout))
    }

    /// Translate a reqwest error into the user-facing message, distinguishing
    /// the request-level timeout from other connect/TLS errors.
    fn map_error(method: &str, e: reqwest::Error, _timeout: Duration) -> String {
        if e.is_timeout() {
            if method == "GET" {
                "S3 列表请求超时，请检查网络与服务地址".to_owned()
            } else {
                "S3 传输超时，请检查网络与服务地址".to_owned()
            }
        } else {
            format!("S3 请求失败: {e}")
        }
    }

    /// Path-style canonical path for an object key (`/{bucket}/{key}`).
    fn object_path(&self, key: &str) -> String {
        let key = key.trim_start_matches('/');
        let encoded = Self::encode(key, true);
        format!("/{}/{}", self.bucket, encoded)
    }
}

impl RemoteStorage for S3Storage {
    fn list(&self, prefix: &str) -> Result<Vec<RemoteObject>, String> {
        let path = format!("/{}", self.bucket);
        let prefix = prefix.trim_start_matches('/');
        let response = self.send(
            "GET",
            &path,
            &[("list-type", "2"), ("prefix", prefix)],
            b"",
            self.list_timeout,
            &[],
        )?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("S3 列表失败: HTTP {status}"));
        }
        let text = response
            .text()
            .map_err(|e| format!("S3 列表响应读取失败: {e}"))?;
        parse_list_v2(&text)
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, String> {
        self.get_with_etag(key).map(|(data, _)| data)
    }

    fn get_with_etag(&self, key: &str) -> Result<(Vec<u8>, Option<String>), String> {
        let path = self.object_path(key);
        let response = self.send("GET", &path, &[], b"", self.io_timeout, &[])?;
        if response.status() == reqwest::StatusCode::OK {
            let etag = response
                .headers()
                .get("ETag")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            response
                .bytes()
                .map(|b| (b.to_vec(), etag))
                .map_err(|e| format!("S3 下载响应读取失败: {e}"))
        } else {
            Err(format!("S3 下载失败: HTTP {}", response.status()))
        }
    }

    fn put(&self, key: &str, data: &[u8]) -> Result<(), String> {
        self.put_if_match(key, data, None)
    }

    fn put_if_match(&self, key: &str, data: &[u8], etag: Option<&str>) -> Result<(), String> {
        let path = self.object_path(key);
        let headers: &[(&str, &str)] = match etag {
            Some(etag) => &[("If-Match", etag)],
            None => &[],
        };
        let response = self.send("PUT", &path, &[], data, self.io_timeout, headers)?;
        let status = response.status();
        if status.as_u16() == 412 {
            return Err(format!(
                "{REMOTE_CONFLICT_MARKER}远程库已被其他设备修改（条件写入被拒绝 HTTP 412），请选择合并、覆盖远程、下载远程或保留本地"
            ));
        }
        if status.is_success() {
            Ok(())
        } else {
            Err(format!("S3 上传失败: HTTP {status}"))
        }
    }
}

/// Test-only operations: bucket create / object delete / bucket delete, used
/// by the live-server round-trip test so no third-party S3 crate is needed.
#[cfg(test)]
impl S3Storage {
    pub(crate) fn create_bucket(&self) -> Result<(), String> {
        let path = format!("/{}", self.bucket);
        let response = self.send("PUT", &path, &[], b"", self.io_timeout, &[])?;
        let status = response.status().as_u16();
        if status == 200 || status == 409 {
            Ok(())
        } else {
            Err(format!("S3 创建桶失败: HTTP {status}"))
        }
    }

    pub(crate) fn delete_object(&self, key: &str) -> Result<(), String> {
        let path = self.object_path(key);
        let response = self.send("DELETE", &path, &[], b"", self.io_timeout, &[])?;
        let status = response.status().as_u16();
        if status == 204 || status == 404 {
            Ok(())
        } else {
            Err(format!("S3 删除对象失败: HTTP {status}"))
        }
    }

    pub(crate) fn delete_bucket(&self) -> Result<(), String> {
        let path = format!("/{}", self.bucket);
        let response = self.send("DELETE", &path, &[], b"", self.io_timeout, &[])?;
        let status = response.status().as_u16();
        if status == 204 || status == 404 {
            Ok(())
        } else {
            Err(format!("S3 删除桶失败: HTTP {status}"))
        }
    }
}

/// Parse a `ListBucketResult` (ListObjectsV2) body into objects.
fn parse_list_v2(body: &str) -> Result<Vec<RemoteObject>, String> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut objects = Vec::new();
    let mut in_contents = false;
    let mut current = String::new();
    let mut key = None;
    let mut size = None;
    let mut modified = None;

    loop {
        match reader.read_event() {
            Err(e) => return Err(format!("S3 列表解析失败: {e}")),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                current = String::from_utf8_lossy(e.local_name().as_ref())
                    .into_owned()
                    .to_ascii_lowercase();
                if current == "contents" {
                    in_contents = true;
                    key = None;
                    size = None;
                    modified = None;
                }
            }
            Ok(Event::Text(t)) => {
                if in_contents {
                    let text = t.xml10_content().unwrap_or_default().trim().to_owned();
                    match current.as_str() {
                        "key" => key = Some(text),
                        "size" => size = text.parse().ok(),
                        "lastmodified" => modified = Some(text),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref())
                    .into_owned()
                    .to_ascii_lowercase();
                if name == "contents" && in_contents {
                    if let Some(key) = key.as_deref() {
                        objects.push(RemoteObject {
                            key: key.trim_start_matches('/').to_owned(),
                            size: size.unwrap_or(0),
                            modified: modified.clone(),
                        });
                    }
                    in_contents = false;
                } else {
                    current.clear();
                }
            }
            Ok(_) => {}
        }
    }
    Ok(objects)
}
