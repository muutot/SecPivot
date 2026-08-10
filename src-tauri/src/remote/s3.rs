//! S3 transport (rust-s3, path-style so custom endpoints such as MinIO work;
//! extracted from remote/mod.rs).

use super::shared_runtime;
use super::RemoteObject;
use super::RemoteStorage;
use super::{REMOTE_CONNECT_TIMEOUT, REMOTE_IO_TIMEOUT, REMOTE_LIST_TIMEOUT};
use crate::config::RemoteSettings;
use std::time::Duration;
// ---------------------------------------------------------------------------
// S3 transport (rust-s3, path-style so custom endpoints such as MinIO work)
// ---------------------------------------------------------------------------

pub struct S3Storage {
    bucket: s3::Bucket,
    runtime: &'static tokio::runtime::Runtime,
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
        let region = s3::Region::Custom {
            region: region.to_owned(),
            endpoint: endpoint.to_owned(),
        };
        let credentials =
            s3::creds::Credentials::new(Some(access_key), Some(secret_key), None, None, None)
                .map_err(|e| format!("S3 凭据无效: {e}"))?;
        let mut bucket = s3::Bucket::new(bucket_name, region, credentials)
            .map_err(|e| format!("S3 配置无效: {e}"))?
            .with_request_timeout(REMOTE_CONNECT_TIMEOUT)
            .map_err(|e| format!("S3 配置无效: {e}"))?;
        bucket.set_path_style();
        Ok(Self {
            bucket,
            runtime: shared_runtime(),
            list_timeout,
            io_timeout,
        })
    }

    fn object_key(key: &str) -> String {
        format!("/{}", key.trim_start_matches('/'))
    }
}

impl RemoteStorage for S3Storage {
    fn list(&self, prefix: &str) -> Result<Vec<RemoteObject>, String> {
        self.runtime.block_on(async {
            let result = tokio::time::timeout(self.list_timeout, async {
                let results = self
                    .bucket
                    .list(prefix.trim_start_matches('/').to_owned(), None)
                    .await
                    .map_err(|e| format!("S3 列表请求失败: {e}"))?;
                let mut objects = Vec::new();
                for page in results {
                    for item in page.contents {
                        objects.push(RemoteObject {
                            key: item.key.trim_start_matches('/').to_owned(),
                            size: item.size as usize,
                            modified: Some(item.last_modified),
                        });
                    }
                }
                Ok(objects)
            })
            .await;
            result.map_err(|_| "S3 列表请求超时，请检查网络与服务地址".to_owned())?
        })
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, String> {
        self.runtime.block_on(async {
            let result = tokio::time::timeout(self.io_timeout, async {
                let response = self
                    .bucket
                    .get_object(&Self::object_key(key))
                    .await
                    .map_err(|e| format!("S3 下载失败: {e}"))?;
                if response.status_code() != 200 {
                    return Err(format!("S3 下载失败: HTTP {}", response.status_code()));
                }
                Ok(response.to_vec())
            })
            .await;
            result.map_err(|_| "S3 下载超时，请检查网络与服务地址".to_owned())?
        })
    }

    fn put(&self, key: &str, data: &[u8]) -> Result<(), String> {
        self.runtime.block_on(async {
            let result = tokio::time::timeout(self.io_timeout, async {
                let response = self
                    .bucket
                    .put_object_with_content_type(
                        &Self::object_key(key),
                        data,
                        "application/octet-stream",
                    )
                    .await
                    .map_err(|e| format!("S3 上传失败: {e}"))?;
                if response.status_code() != 200 {
                    return Err(format!("S3 上传失败: HTTP {}", response.status_code()));
                }
                Ok(())
            })
            .await;
            result.map_err(|_| "S3 上传超时，请检查网络与服务地址".to_owned())?
        })
    }
}
