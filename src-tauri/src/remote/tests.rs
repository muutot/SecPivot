//! Remote transport tests: in-memory fake, S3 and WebDAV local TCP mocks
//! (extracted from remote/mod.rs).

use super::local::{profile_storage_parts, sanitize_dir_name};
use super::s3::S3Storage;
use super::shared_runtime;
use super::webdav::WebDavStorage;
use super::webdav::{href_to_key, parse_multistatus};
use super::*;
use crate::config::RemoteSettings;

#[test]
fn memory_storage_lists_gets_puts() {
    let storage = MemoryStorage::default();
    storage.seed("vaults/a.kdbx", vec![1, 2, 3]);
    storage.seed("notes/b.txt", vec![4]);

    let objects = storage.list("vaults/").unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].key, "vaults/a.kdbx");
    assert_eq!(objects[0].size, 3);

    let objects = storage.list("").unwrap();
    assert_eq!(objects.len(), 2);

    assert_eq!(storage.get("vaults/a.kdbx").unwrap(), vec![1, 2, 3]);
    assert!(storage.get("missing.kdbx").is_err());

    storage.put("vaults/b.kdbx", &[9, 9]).unwrap();
    assert_eq!(storage.get("vaults/b.kdbx").unwrap(), vec![9, 9]);
}

#[test]
fn local_dir_name_is_sanitized() {
    assert_eq!(sanitize_dir_name("  "), "remote");
    assert_eq!(sanitize_dir_name("my vaults"), "my_vaults");
    assert_eq!(sanitize_dir_name("..\\..\\evil"), "______evil");
    assert_eq!(sanitize_dir_name("safe-dir_1"), "safe-dir_1");
    // Unicode (Chinese) profile names survive the sanitizer.
    assert_eq!(sanitize_dir_name("阿里云"), "阿里云");
    assert_eq!(sanitize_dir_name("默认 备份"), "默认_备份");
    assert_eq!(sanitize_dir_name("vault/备份"), "vault_备份");

    assert_eq!(
        profile_storage_parts("s3/config_1").unwrap(),
        ("s3", "config_1".to_owned())
    );
    assert_eq!(
        profile_storage_parts("webdav/默认 备份").unwrap(),
        ("webdav", "默认_备份".to_owned())
    );
    assert!(profile_storage_parts("ftp/config_1").is_err());
    assert!(profile_storage_parts("s3").is_err());
}

/// S3 transports must share one process-wide runtime: a fresh thread pool
/// per command would defeat its purpose. Both calls must resolve to the
/// exact same instance.
#[test]
fn s3_uses_a_single_shared_runtime() {
    let a = shared_runtime() as *const _;
    let b = shared_runtime() as *const _;
    assert!(std::ptr::eq(a, b));
}

// -----------------------------------------------------------------------
// Local mock S3 server: turns the "no live S3 evidence" gap into an
// offline HTTP-level round trip through the real S3Storage transport
// (path-style signing, ListObjectsV2 XML parsing, get/put).
// -----------------------------------------------------------------------

const LIST_V2_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">\
<Name>test-bucket</Name><Prefix>vaults/</Prefix><KeyCount>1</KeyCount><MaxKeys>1000</MaxKeys>\
<IsTruncated>false</IsTruncated>\
<Contents><Key>vaults/a.kdbx</Key>\
<LastModified>2024-01-01T00:00:00.000Z</LastModified>\
<ETag>&quot;abc&quot;</ETag><Size>3</Size><StorageClass>STANDARD</StorageClass></Contents>\
</ListBucketResult>";

/// Mixed bucket for the ordering test: two databases plus one other file.
const MIXED_V2_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">\
<Name>test-bucket</Name><Prefix>vaults/</Prefix><KeyCount>3</KeyCount><MaxKeys>1000</MaxKeys>\
<IsTruncated>false</IsTruncated>\
<Contents><Key>vaults/notes.txt</Key><LastModified>2024-01-01T00:00:00.000Z</LastModified>\
<ETag>&quot;1&quot;</ETag><Size>4</Size><StorageClass>STANDARD</StorageClass></Contents>\
<Contents><Key>vaults/z.kdbx</Key><LastModified>2024-01-01T00:00:00.000Z</LastModified>\
<ETag>&quot;2&quot;</ETag><Size>5</Size><StorageClass>STANDARD</StorageClass></Contents>\
<Contents><Key>vaults/a.kdbx</Key><LastModified>2024-01-01T00:00:00.000Z</LastModified>\
<ETag>&quot;3&quot;</ETag><Size>3</Size><StorageClass>STANDARD</StorageClass></Contents>\
</ListBucketResult>";

/// Serve one HTTP/1.1 request per connection (no keep-alive): list-type=2
/// returns `list_xml`, PUTs are acknowledged, other GETs return `[1,2,3]`.
fn s3_mock_handle(mut stream: std::net::TcpStream, list_xml: &'static str) {
    use std::io::{Read, Write};
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf[..header_end]);
    let head = text.lines().next().unwrap_or("").to_string();
    let is_list = head.starts_with("GET") && text.contains("list-type=2");
    let is_put = head.starts_with("PUT");
    let content_length: usize = text
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1).and_then(|v| v.trim().parse().ok()))
        .unwrap_or(0);
    while buf.len() < header_end + content_length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body: Vec<u8> = if is_list {
        list_xml.as_bytes().to_vec()
    } else if is_put {
        Vec::new()
    } else {
        vec![1u8, 2, 3]
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(&body);
}

fn spawn_s3_mock_with_xml(list_xml: &'static str) -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock listener");
    let addr = listener.local_addr().expect("mock addr");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || s3_mock_handle(stream, list_xml));
        }
    });
    addr
}

fn spawn_s3_mock() -> std::net::SocketAddr {
    spawn_s3_mock_with_xml(LIST_V2_XML)
}
fn mock_config(addr: std::net::SocketAddr) -> RemoteSettings {
    RemoteSettings {
        kind: "s3".into(),
        endpoint: format!("http://{addr}"),
        region: "us-east-1".to_owned(),
        bucket: "test-bucket".to_owned(),
        access_key: "AK".to_owned(),
        secret_key: "SK".to_owned(),
        ..Default::default()
    }
}

#[test]
fn s3_transport_round_trips_against_local_mock() {
    let storage = S3Storage::with_timeouts(
        &mock_config(spawn_s3_mock()),
        Duration::from_secs(10),
        Duration::from_secs(10),
    )
    .expect("storage");

    let objects = storage.list("vaults/").expect("list");
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].key, "vaults/a.kdbx");
    assert_eq!(objects[0].size, 3);

    assert_eq!(storage.get("vaults/a.kdbx").expect("get"), vec![1, 2, 3]);
    storage.put("vaults/b.kdbx", &[9, 9]).expect("put");
}

/// The Tauri commands run on async runtime worker threads, where
/// `S3Storage`'s sync methods (`Runtime::block_on`) panic — the command
/// future aborts and the invoke never resolves, leaving the UI on an
/// endless "正在加载…". Commands must hop to the blocking pool first, as
/// `list_objects_async` does; this test pins that path end-to-end.
#[test]
fn s3_list_objects_async_works_from_runtime_worker_thread() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("rt");
    let cfg = mock_config(spawn_s3_mock());
    rt.block_on(async move {
        let objects = list_objects_async(cfg).await.expect("list");
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].key, "vaults/a.kdbx");
        assert_eq!(objects[0].size, 3);
    });
}

/// The list must not filter to `.kdbx` only: every object shows, with
/// databases first and then keys descending.
#[test]
fn s3_list_objects_async_lists_all_objects_kdbx_first() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("rt");
    let cfg = mock_config(spawn_s3_mock_with_xml(MIXED_V2_XML));
    rt.block_on(async move {
        let objects = list_objects_async(cfg).await.expect("list");
        let keys: Vec<&str> = objects.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, ["vaults/z.kdbx", "vaults/a.kdbx", "vaults/notes.txt"]);
    });
}

/// A server that accepts the connection but never answers must surface a
/// bounded error instead of hanging the list forever.
#[test]
fn s3_list_surfaces_timeout_error_instead_of_hanging() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock listener");
    let addr = listener.local_addr().expect("mock addr");
    std::thread::spawn(move || {
        for _stream in listener.incoming().flatten() {
            std::thread::sleep(Duration::from_secs(30));
        }
    });
    let storage = S3Storage::with_timeouts(
        &mock_config(addr),
        Duration::from_millis(500),
        Duration::from_secs(10),
    )
    .expect("storage");
    let err = storage.list("vaults/").expect_err("list must time out");
    assert!(err.contains("超时"), "unexpected error: {err}");
}

// -----------------------------------------------------------------------
// WebDAV transport tests (offline mock HTTP server + pure parsers)
// -----------------------------------------------------------------------

const WEBDAV_MULTISTATUS: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<d:multistatus xmlns:d=\"DAV:\">\
<d:response><d:href>/dav/vaults/</d:href><d:propstat><d:prop>\
<d:resourcetype><d:collection/></d:resourcetype></d:prop>\
<d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
<d:response><d:href>/dav/vaults/a.kdbx</d:href><d:propstat><d:prop>\
<d:getcontentlength>3</d:getcontentlength>\
<d:getlastmodified>Mon, 01 Jan 2024 00:00:00 GMT</d:getlastmodified></d:prop>\
<d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
<d:response><d:href>/dav/vaults/sub/</d:href><d:propstat><d:prop>\
<d:resourcetype><d:collection/></d:resourcetype></d:prop>\
<d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
<d:response><d:href>/dav/vaults/z.kdbx</d:href><d:propstat><d:prop>\
<d:getcontentlength>5</d:getcontentlength></d:prop>\
<d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
</d:multistatus>";

/// Serve one WebDAV request per connection: PROPFIND → multistatus,
/// GET `a.kdbx` → `[1,2,3]`, PUT → 201, anything else → 404.
fn webdav_mock_handle(mut stream: std::net::TcpStream) {
    use std::io::{Read, Write};
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let head = text.lines().next().unwrap_or("").to_string();
    let content_length: usize = text
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1).and_then(|v| v.trim().parse().ok()))
        .unwrap_or(0);
    while buf.len() < header_end + content_length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let is_propfind = head.starts_with("PROPFIND");
    let is_put = head.starts_with("PUT");
    let is_get = head.starts_with("GET");
    let (status, body): (&str, Vec<u8>) = if is_propfind {
        ("207 Multi-Status", WEBDAV_MULTISTATUS.as_bytes().to_vec())
    } else if is_get && text.contains("/a.kdbx") {
        ("200 OK", vec![1u8, 2, 3])
    } else if is_put {
        ("201 Created", Vec::new())
    } else {
        ("404 Not Found", Vec::new())
    };
    let content_type = if is_propfind {
        "application/xml; charset=utf-8"
    } else {
        "application/octet-stream"
    };
    let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(&body);
}

fn spawn_webdav_mock() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock listener");
    let addr = listener.local_addr().expect("mock addr");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || webdav_mock_handle(stream));
        }
    });
    addr
}

fn mock_webdav_config(addr: std::net::SocketAddr) -> RemoteSettings {
    RemoteSettings {
        kind: "webdav".into(),
        endpoint: format!("http://{addr}/dav"),
        access_key: "user".into(),
        secret_key: "pass".into(),
        prefix: "vaults/".into(),
        ..RemoteSettings::webdav_default()
    }
}

#[test]
fn webdav_transport_round_trips_against_local_mock() {
    let storage = WebDavStorage::with_timeouts(
        &mock_webdav_config(spawn_webdav_mock()),
        Duration::from_secs(10),
        Duration::from_secs(10),
    )
    .expect("storage");

    let mut objects = storage.list("vaults/").expect("list");
    objects.sort_by(|a, b| a.key.cmp(&b.key));
    assert_eq!(objects.len(), 2, "collections and self must be dropped");
    assert_eq!(objects[0].key, "vaults/a.kdbx");
    assert_eq!(objects[0].size, 3);
    assert_eq!(
        objects[0].modified.as_deref(),
        Some("Mon, 01 Jan 2024 00:00:00 GMT")
    );
    assert_eq!(objects[1].key, "vaults/z.kdbx");
    assert_eq!(objects[1].size, 5);

    assert_eq!(storage.get("vaults/a.kdbx").expect("get"), vec![1, 2, 3]);
    storage.put("vaults/b.kdbx", &[9, 9]).expect("put");
}

/// The command path (factory + blocking-pool hop) must work end-to-end for
/// WebDAV exactly as it does for S3.
#[test]
fn webdav_list_objects_async_works_from_runtime_worker_thread() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("rt");
    let cfg = mock_webdav_config(spawn_webdav_mock());
    rt.block_on(async move {
        let objects = list_objects_async(cfg).await.expect("list");
        let keys: Vec<&str> = objects.iter().map(|o| o.key.as_str()).collect();
        // .kdbx first (both are), then key descending.
        assert_eq!(keys, ["vaults/z.kdbx", "vaults/a.kdbx"]);
    });
}

#[test]
fn webdav_parser_keeps_files_and_drops_collections() {
    let storage = WebDavStorage {
        client: reqwest::blocking::Client::new(),
        auth: None,
        base_url: "http://dav.example.com/dav".into(),
        list_timeout: Duration::from_secs(10),
        io_timeout: Duration::from_secs(10),
    };
    let objects = parse_multistatus(
        WEBDAV_MULTISTATUS,
        &storage.base_url,
        "http://dav.example.com/dav/vaults/",
    )
    .expect("parse");
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0].key, "vaults/a.kdbx");
    assert_eq!(objects[1].key, "vaults/z.kdbx");
}

/// Cloud-drive gateways often omit `getcontentlength` (or return it in a
/// 404 propstat) and may use uppercase element names. Both must not cause
/// every file to be dropped — the empty-list symptom reported against
/// 123pan's WebDAV.
#[test]
fn webdav_parser_keeps_files_without_content_length_and_uppercase_tags() {
    let body = "<?xml version=\"1.0\"?><D:MULTISTATUS xmlns:D=\"DAV:\">\
<D:RESPONSE><D:HREF>/dav/</D:HREF><D:PROPSTAT><D:PROP>\
<D:RESOURCETYPE><D:COLLECTION/></D:RESOURCETYPE></D:PROP>\
<D:STATUS>HTTP/1.1 200 OK</D:STATUS></D:PROPSTAT></D:RESPONSE>\
<D:RESPONSE><D:HREF>/dav/a.kdbx</D:HREF><D:PROPSTAT><D:PROP>\
<D:GETCONTENTLENGTH>3</D:GETCONTENTLENGTH></D:PROP>\
<D:STATUS>HTTP/1.1 200 OK</D:STATUS></D:PROPSTAT></D:RESPONSE>\
<D:RESPONSE><D:HREF>/dav/z.kdbx</D:HREF><D:PROPSTAT>\
<D:STATUS>HTTP/1.1 404 Not Found</D:STATUS></D:PROPSTAT></D:RESPONSE>\
</D:MULTISTATUS>";
    let objects = parse_multistatus(
        body,
        "http://dav.example.com/dav",
        "http://dav.example.com/dav/",
    )
    .expect("parse");
    let keys: Vec<&str> = objects.iter().map(|o| o.key.as_str()).collect();
    // a.kdbx keeps its size; z.kdbx survives without any getcontentlength.
    assert_eq!(keys, ["a.kdbx", "z.kdbx"]);
    assert_eq!(objects[0].size, 3);
    assert_eq!(objects[1].size, 0);
}

/// A 200 response that is not a multistatus (an HTML/JSON error page from a
/// misconfigured endpoint) must surface an actionable error instead of
/// silently reporting "no files".
#[test]
fn webdav_parser_rejects_non_multistatus_body() {
    let err = parse_multistatus(
        "<html><body>Invalid request</body></html>",
        "http://dav.example.com/dav",
        "http://dav.example.com/dav/",
    )
    .expect_err("must fail");
    assert!(err.contains("multistatus"), "unexpected error: {err}");
}

#[test]
fn webdav_href_to_key_handles_absolute_and_relative_hrefs() {
    assert_eq!(
        href_to_key("http://h/dav", "http://h/dav/vaults/a.kdbx"),
        "vaults/a.kdbx"
    );
    assert_eq!(
        href_to_key("http://h/dav", "/dav/vaults/a.kdbx"),
        "vaults/a.kdbx"
    );
    assert_eq!(
        href_to_key("http://h/dav", "vaults/a.kdbx"),
        "vaults/a.kdbx"
    );
    assert_eq!(href_to_key("http://h", "/vaults/a.kdbx"), "vaults/a.kdbx");
    assert_eq!(href_to_key("http://h/dav", "http://h/dav/"), "");
}

#[test]
fn webdav_url_for_encodes_segments_and_rejects_dotdot() {
    let storage = WebDavStorage {
        client: reqwest::blocking::Client::new(),
        auth: None,
        base_url: "http://h/dav".into(),
        list_timeout: Duration::from_secs(10),
        io_timeout: Duration::from_secs(10),
    };
    assert_eq!(
        storage.url_for("a b/c.kdbx").unwrap(),
        "http://h/dav/a%20b/c.kdbx"
    );
    assert_eq!(storage.url_for("a.kdbx").unwrap(), "http://h/dav/a.kdbx");
    // ".." segments must not escape the endpoint.
    assert_eq!(storage.url_for("../x.kdbx").unwrap(), "http://h/dav/x.kdbx");
}
