//! Remote transport tests: in-memory fake, S3 and WebDAV local TCP mocks
//! (extracted from remote/mod.rs).

use super::local::{profile_storage_parts, sanitize_dir_name};
use super::s3::S3Storage;
use super::shared_blocking_client;
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
fn memory_conditional_put_is_compare_and_swap() {
    let storage = MemoryStorage::default();
    storage.seed("vaults/a.kdbx", b"original".to_vec());

    let (data, etag) = storage.get_with_etag("vaults/a.kdbx").unwrap();
    assert_eq!(data, b"original");
    assert!(etag.is_some(), "the fake always exposes a content identity");

    // A concurrent writer lands between our read and our write.
    storage.put("vaults/a.kdbx", b"interloper").unwrap();

    let error = storage
        .put_if_match("vaults/a.kdbx", b"stale-write", etag.as_deref())
        .unwrap_err();
    assert!(error.starts_with(REMOTE_CONFLICT_MARKER));
    assert_eq!(
        storage.get("vaults/a.kdbx").unwrap(),
        b"interloper".to_vec(),
        "the stale conditional write must not clobber the newer object"
    );

    // A fresh observation precondition succeeds.
    let (_, etag) = storage.get_with_etag("vaults/a.kdbx").unwrap();
    storage
        .put_if_match("vaults/a.kdbx", b"merged", etag.as_deref())
        .unwrap();
    assert_eq!(storage.get("vaults/a.kdbx").unwrap(), b"merged".to_vec());

    // No precondition degrades to a plain put.
    storage
        .put_if_match("vaults/a.kdbx", b"forced", None)
        .unwrap();
    assert_eq!(storage.get("vaults/a.kdbx").unwrap(), b"forced".to_vec());
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

/// S3 transports must share one process-wide reqwest client: a fresh client
/// per command would spin up (and tear down) a full connection pool on each
/// list/open/save. Both calls must resolve to the exact same instance.
#[test]
fn s3_uses_a_single_shared_client() {
    let Ok(a) = shared_blocking_client() else {
        panic!("shared client must initialize in tests");
    };
    let a: *const reqwest::blocking::Client = a;
    let Ok(b) = shared_blocking_client() else {
        panic!("shared client must stay initialized");
    };
    let b: *const reqwest::blocking::Client = b;
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

fn configure_s3_page_stream(stream: &std::net::TcpStream) {
    // Windows accepted sockets inherit the listener's nonblocking mode.
    // read_timeout alone does not change it: header reads can immediately
    // return WouldBlock before reqwest sends any bytes.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
}

/// Finite scripted HTTP peer: captures the actual encoded request target and
/// exits even when a regression stops requesting pages early.
fn spawn_s3_pages(
    pages: Vec<(Duration, String)>,
) -> (std::net::SocketAddr, std::sync::mpsc::Receiver<String>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // The full Rust suite runs CPU-heavy KDF tests concurrently. Give the
        // finite 100-page script longer than the client's 30 s total deadline,
        // so mock shutdown cannot masquerade as a transport/pagination failure.
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        for (delay, body) in pages {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return,
                }
            };
            configure_s3_page_stream(&stream);
            let mut header = Vec::new();
            let mut byte = [0];
            while header.len() < 16 * 1024 && !header.ends_with(b"\r\n\r\n") {
                if stream.read_exact(&mut byte).is_err() {
                    return;
                }
                header.push(byte[0]);
            }
            let request = String::from_utf8(header).unwrap();
            let _ = tx.send(request.lines().next().unwrap().to_owned());
            std::thread::sleep(delay);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (addr, rx)
}

#[test]
fn s3_page_fixture_waits_for_delayed_request_bytes() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut sender = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    configure_s3_page_stream(&stream);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut byte = [0];
        ready_tx.send(()).unwrap();
        let result = stream.read_exact(&mut byte);
        (result, byte)
    });
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    sender.write_all(b"A").unwrap();
    let (result, byte) = reader.join().unwrap();
    assert!(
        result.is_ok(),
        "accepted fixture stream must wait for request bytes: {result:?}"
    );
    assert_eq!(byte, [b'A']);
}

#[test]
fn s3_list_follows_pages_and_preserves_encoded_token_and_keys() {
    let first = r#"<ListBucketResult><IsTruncated>true</IsTruncated>
        <NextContinuationToken> +/=&amp;雪 </NextContinuationToken>
        <Contents><Key>vaults/a.kdbx</Key><Size>3</Size></Contents></ListBucketResult>"#;
    let second = r#"<ListBucketResult><IsTruncated>false</IsTruncated>
        <Contents><Key>vaults/ a&amp;b&#x20;<![CDATA[雪.kdbx ]]></Key><Size>4</Size></Contents></ListBucketResult>"#;
    let (addr, requests) = spawn_s3_pages(vec![
        (Duration::ZERO, first.into()),
        (Duration::ZERO, second.into()),
    ]);
    let storage = S3Storage::with_timeouts(
        &mock_config(addr),
        Duration::from_secs(3),
        Duration::from_secs(3),
    )
    .unwrap();
    let objects = storage.list("vaults/").unwrap();
    assert_eq!(
        objects.len(),
        2,
        "a truncated first page is not the whole list"
    );
    assert_eq!(objects[1].key, "vaults/ a&b 雪.kdbx ");
    assert_eq!(objects[1].size, 4);
    assert_eq!(
        requests.recv().unwrap(),
        "GET /test-bucket?list-type=2&prefix=vaults%2F HTTP/1.1"
    );
    assert_eq!(requests.recv().unwrap(), "GET /test-bucket?continuation-token=%20%2B%2F%3D%26%E9%9B%AA%20&list-type=2&prefix=vaults%2F HTTP/1.1");
}

#[test]
fn s3_list_rejects_missing_and_cyclic_continuation_tokens() {
    let missing = "<ListBucketResult><IsTruncated>true</IsTruncated></ListBucketResult>";
    let (addr, _) = spawn_s3_pages(vec![(Duration::ZERO, missing.into())]);
    let storage = S3Storage::new(&mock_config(addr)).unwrap();
    assert!(storage.list("").unwrap_err().contains("缺少后续分页标记"));

    let page = |token| {
        format!("<ListBucketResult><IsTruncated>true</IsTruncated><NextContinuationToken>{token}</NextContinuationToken></ListBucketResult>")
    };
    let (addr, requests) = spawn_s3_pages(vec![
        (Duration::ZERO, page("A")),
        (Duration::ZERO, page("B")),
        (Duration::ZERO, page("A")),
    ]);
    let storage = S3Storage::new(&mock_config(addr)).unwrap();
    assert!(storage.list("").unwrap_err().contains("重复分页标记"));
    assert_eq!(requests.try_iter().count(), 3);
}

#[test]
fn s3_list_later_invalid_page_never_returns_a_partial_list() {
    let first = "<ListBucketResult><IsTruncated>true</IsTruncated><NextContinuationToken>A</NextContinuationToken><Contents><Key>first.kdbx</Key></Contents></ListBucketResult>";
    let (addr, requests) = spawn_s3_pages(vec![
        (Duration::ZERO, first.into()),
        (Duration::ZERO, "<html>Service unavailable</html>".into()),
    ]);
    let storage = S3Storage::new(&mock_config(addr)).unwrap();
    assert!(storage.list("").unwrap_err().contains("ListBucketResult"));
    assert_eq!(requests.try_iter().count(), 2);
}

#[test]
fn s3_list_uses_one_deadline_across_pages() {
    let first = "<ListBucketResult><IsTruncated>true</IsTruncated><NextContinuationToken>A</NextContinuationToken></ListBucketResult>";
    let (addr, requests) = spawn_s3_pages(vec![
        (Duration::from_millis(180), first.into()),
        (Duration::from_millis(300), LIST_V2_XML.into()),
    ]);
    let storage = S3Storage::with_timeouts(
        &mock_config(addr),
        Duration::from_millis(400),
        Duration::from_secs(3),
    )
    .unwrap();
    let started = std::time::Instant::now();
    assert!(
        storage.list("").is_err(),
        "the timeout must not reset at each page"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(requests.try_iter().count(), 2);
}

#[test]
fn s3_list_bounds_object_count_and_page_bytes() {
    let body = format!(
        "<ListBucketResult><IsTruncated>false</IsTruncated>{}</ListBucketResult>",
        "<Contents><Key>a.kdbx</Key></Contents>".repeat(10_001)
    );
    let (addr, _) = spawn_s3_pages(vec![(Duration::ZERO, body)]);
    let storage = S3Storage::new(&mock_config(addr)).unwrap();
    assert!(storage.list("").unwrap_err().contains("对象数量上限"));

    let (addr, _) = spawn_s3_pages(vec![(Duration::ZERO, " ".repeat(8 * 1024 * 1024 + 1))]);
    let storage = S3Storage::new(&mock_config(addr)).unwrap();
    assert!(storage.list("").unwrap_err().contains("8 MiB 上限"));
}

#[test]
fn s3_list_bounds_pages_even_when_empty_tokens_keep_changing() {
    let pages = (0..100).map(|i| (Duration::ZERO, format!(
        "<ListBucketResult><IsTruncated>true</IsTruncated><NextContinuationToken>t{i}</NextContinuationToken></ListBucketResult>"
    ))).collect();
    let (addr, requests) = spawn_s3_pages(pages);
    let storage = S3Storage::new(&mock_config(addr)).unwrap();
    let error = storage.list("").unwrap_err();
    assert!(error.contains("分页上限"), "unexpected error: {error}");
    assert_eq!(requests.try_iter().count(), 100);
}

#[test]
fn s3_list_rejects_malformed_xml_instead_of_showing_an_empty_bucket() {
    for body in [
        "<ListBucketResult><IsTruncated>false</IsTruncated>",
        "<ListBucketResult><IsTruncated>maybe</IsTruncated></ListBucketResult>",
        "<ListBucketResult></ListBucketResult>",
        "<ListBucketResult><IsTruncated>false</IsTruncated><Contents><Key>a&unknown;b</Key></Contents></ListBucketResult>",
    ] {
        let (addr, _) = spawn_s3_pages(vec![(Duration::ZERO, body.into())]);
        let storage = S3Storage::new(&mock_config(addr)).unwrap();
        assert!(storage.list("").is_err(), "invalid response must not succeed: {body}");
    }
}

#[test]
fn unknown_storage_kind_is_rejected() {
    // A typo'd kind must fail fast instead of silently building an S3 client
    // (which would surface as a confusing network error later).
    let mut cfg = mock_config("127.0.0.1:9000".parse().expect("addr"));
    cfg.kind = "WebDav".into();
    match make_storage(&cfg) {
        Err(err) => assert!(
            err.contains("未知的远程存储类型"),
            "unexpected error: {err}"
        ),
        Ok(_) => panic!("unknown kind must be rejected"),
    }

    cfg.kind = "s3".into();
    assert!(make_storage(&cfg).is_ok(), "canonical s3 must build");
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

#[test]
fn s3_rejects_scheme_less_or_non_http_endpoints() {
    let base = |endpoint: &str| RemoteSettings {
        kind: "s3".into(),
        endpoint: endpoint.to_owned(),
        region: "us-east-1".to_owned(),
        bucket: "test-bucket".to_owned(),
        access_key: "AK".to_owned(),
        secret_key: "SK".to_owned(),
        ..Default::default()
    };
    // A bare host (no scheme) used to panic inside `host_header`.
    for bad in [
        "127.0.0.1:9000",
        "localhost:9000",
        "ftp://example.com",
        "://nope",
    ] {
        match S3Storage::with_timeouts(&base(bad), Duration::from_secs(10), Duration::from_secs(10))
        {
            Err(err) => assert!(
                err.contains("S3 服务地址无效"),
                "unexpected error for {bad:?}: {err}"
            ),
            Ok(_) => panic!("misconfigured endpoint must be rejected: {bad:?}"),
        }
    }
    // A valid http(s) endpoint still builds and yields the expected host header.
    let storage = S3Storage::with_timeouts(
        &base("http://127.0.0.1:9000"),
        Duration::from_secs(10),
        Duration::from_secs(10),
    )
    .expect("valid endpoint");
    assert_eq!(storage.host_header().unwrap(), "127.0.0.1:9000");
    let storage = S3Storage::with_timeouts(
        &base("https://minio.example.com"),
        Duration::from_secs(10),
        Duration::from_secs(10),
    )
    .expect("valid endpoint");
    assert_eq!(storage.host_header().unwrap(), "minio.example.com");
}

/// Live end-to-end S3 round trip against a real local S3-compatible server
/// (MinIO at `B:\Program\s3\minio`, `http://127.0.0.1:9000`,
/// `rustfsadmin`/`rustfsadmin`, path-style). It exercises the same
/// `S3Storage` code path the desktop app uses: bucket create, put/list/get,
/// then object + bucket delete. The test skips itself when the server is not
/// running so the suite stays green offline; run it with the server up for
/// real-provider evidence.
///
/// Note: the rustfs 1.0.0-beta.12 binary at `B:\Program\s3\rustfs` cannot
/// serve as this server on Windows — its storage layer never reaches
/// readiness (format load hits a Windows sharing violation, `os error 32`,
/// retries 10× and stays on `storage_quorum`); MinIO is the working local
/// substitute and is what this test was verified against (see PITFALLS.md).
#[test]
fn s3_transport_round_trips_against_live_s3_server() {
    const ENDPOINT: &str = "http://127.0.0.1:9000";
    // Opt in explicitly. The previous guard only checked that *something*
    // accepts TCP on the port, so an unrelated service (or a MinIO instance
    // with different credentials) made the test run and fail on an HTTP 403
    // that has nothing to do with this codebase. A test that needs an external
    // service must require the operator to say so.
    if std::env::var("SECPIVOT_S3_LIVE").as_deref() != Ok("1") {
        eprintln!("skip: set SECPIVOT_S3_LIVE=1 to run the live S3 round-trip");
        return;
    }
    let addr: std::net::SocketAddr = "127.0.0.1:9000".parse().expect("loopback addr");
    if std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(800)).is_err() {
        eprintln!("skip: local S3-compatible server not running at {ENDPOINT}");
        return;
    }

    let bucket_name = format!("secpivot-live-{}", std::process::id());
    let cfg = RemoteSettings {
        kind: "s3".into(),
        endpoint: ENDPOINT.to_owned(),
        region: "us-east-1".to_owned(),
        bucket: bucket_name.clone(),
        access_key: "rustfsadmin".to_owned(),
        secret_key: "rustfsadmin".to_owned(),
        ..Default::default()
    };
    let storage = S3Storage::with_timeouts(&cfg, Duration::from_secs(10), Duration::from_secs(10))
        .expect("storage");
    storage.create_bucket().expect("create bucket");

    let key = "vaults/live.kdbx";
    storage.put(key, &[1, 2, 3]).expect("put");
    let objects = storage.list("vaults/").expect("list");
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].key, key);
    assert_eq!(objects[0].size, 3);
    assert_eq!(storage.get(key).expect("get"), vec![1, 2, 3]);

    // Best-effort cleanup so repeated runs on the local server stay tidy.
    let _ = storage.delete_object(key);
    let _ = storage.delete_bucket();
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
        href_to_key("http://h/dav", "http://h/dav/vaults/a.kdbx").unwrap(),
        "vaults/a.kdbx"
    );
    assert_eq!(
        href_to_key("http://h/dav", "/dav/vaults/a.kdbx").unwrap(),
        "vaults/a.kdbx"
    );
    assert_eq!(
        href_to_key("http://h/dav", "vaults/a.kdbx").unwrap(),
        "vaults/a.kdbx"
    );
    assert_eq!(
        href_to_key("http://h", "/vaults/a.kdbx").unwrap(),
        "vaults/a.kdbx"
    );
    assert_eq!(href_to_key("http://h/dav", "http://h/dav/").unwrap(), "");
    for (href, key) in [
        ("/dav/vaults/a%20b.kdbx", "vaults/a b.kdbx"),
        ("http://h/dav/%E9%9B%AA.kdbx", "雪.kdbx"),
        ("//h/dav/rate%25.kdbx", "rate%.kdbx"),
        ("a%2520b.kdbx", "a%20b.kdbx"),
        ("a+b.kdbx", "a+b.kdbx"),
        ("/dav/a%23%3F.kdbx?ignored#fragment", "a#?.kdbx"),
    ] {
        assert_eq!(href_to_key("http://h/dav", href).unwrap(), key);
    }
    assert_eq!(
        href_to_key("http://h/dav%20root", "/dav%20root/a%20b.kdbx").unwrap(),
        "a b.kdbx"
    );
}

#[test]
fn webdav_list_rejects_ambiguous_encoding_before_returning_keys() {
    for href in [
        "/dav/a%.kdbx",
        "/dav/a%2.kdbx",
        "/dav/a%GG.kdbx",
        "/dav/%FF.kdbx",
        "/dav/a%2Fb.kdbx",
        "/dav/%2e/a.kdbx",
        "/dav/%2E%2e/a.kdbx",
        "http://h/dav/../a.kdbx",
        "/dav/a%00.kdbx",
        "/dav/a%0A.kdbx",
        "/dav//a.kdbx",
        "/dav2/a.kdbx",
        "/other/a.kdbx",
    ] {
        let body = format!("<d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/dav/valid.kdbx</d:href></d:response><d:response><d:href>{href}</d:href></d:response></d:multistatus>");
        let error = parse_multistatus(&body, "http://h/dav", "http://h/dav/").unwrap_err();
        assert!(error.starts_with("WebDAV "));
        assert!(
            !error.contains(href),
            "invalid paths must not enter diagnostics"
        );
    }
    // A percent-looking logical filename remains data after the one decode.
    assert_eq!(
        href_to_key("http://h/dav", "/dav/a%252Fb.kdbx").unwrap(),
        "a%2Fb.kdbx"
    );
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
    // ".."/"." segments are rejected outright: silently skipping them would
    // alias distinct keys (e.g. `a/../a` collapsing onto `a`).
    assert!(storage.url_for("../x.kdbx").is_err());
    assert!(storage.url_for("a/../a.kdbx").is_err());
    assert!(storage.url_for("./a.kdbx").is_err());
}

#[test]
fn webdav_xml_property_text_preserves_full_values() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/webdav-xml-text.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let objects = parse_multistatus(
            case["xml"].as_str().unwrap(),
            "http://h/dav",
            "http://h/dav/vaults/",
        )
        .unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].key, case["key"].as_str().unwrap());
        assert_eq!(objects[0].size, case["size"].as_u64().unwrap() as usize);
        assert_eq!(objects[0].modified.as_deref(), case["modified"].as_str());
    }
    let pi = "<d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/dav/vaults/a<?secpivot fixture?>b.kdbx</d:href></d:response></d:multistatus>";
    assert_eq!(
        parse_multistatus(pi, "http://h/dav", "http://h/dav/").unwrap()[0].key,
        "vaults/ab.kdbx"
    );
}

#[test]
fn webdav_xml_invalid_property_content_rejects_the_entire_list() {
    let valid = "<d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/dav/valid.kdbx</d:href></d:response><d:response>";
    for property in ["href", "getcontentlength", "getlastmodified"] {
        for value in ["a&missing;b", "a&#x110000;b", "a<part/>b", "a&amp"] {
            let body =
                format!("{valid}<{property}>{value}</{property}></d:response></d:multistatus>");
            let error = parse_multistatus(&body, "http://h/dav", "http://h/dav/").unwrap_err();
            assert_eq!(error, "WebDAV 列表字段包含无效 XML 内容");
            assert!(!error.contains(value));
        }
        let truncated = format!("{valid}<{property}>unfinished");
        assert_eq!(
            parse_multistatus(&truncated, "http://h/dav", "http://h/dav/").unwrap_err(),
            "WebDAV 列表字段包含无效 XML 内容"
        );
    }
    // Missing or empty hints remain optional, as before.
    let empty = "<d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/dav/a.kdbx</d:href><d:propstat><d:prop><d:getcontentlength></d:getcontentlength><d:getlastmodified></d:getlastmodified></d:prop></d:propstat></d:response></d:multistatus>";
    let objects = parse_multistatus(empty, "http://h/dav", "http://h/dav/").unwrap();
    assert_eq!(objects[0].size, 0);
    assert!(objects[0].modified.is_none());
}

#[test]
fn webdav_listed_filenames_get_put_and_read_back_the_same_resource() {
    use std::collections::BTreeMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let cases = [
        ("plain.kdbx", "plain.kdbx", "plain.kdbx"),
        ("a%20b.kdbx", "a%20b.kdbx", "a b.kdbx"),
        ("%E9%9B%AA.kdbx", "%E9%9B%AA.kdbx", "雪.kdbx"),
        ("rate%25.kdbx", "rate%25.kdbx", "rate%.kdbx"),
        ("a%2520b.kdbx", "a%2520b.kdbx", "a%20b.kdbx"),
        ("a+b.kdbx", "a+b.kdbx", "a+b.kdbx"),
        ("a%23%3F.kdbx", "a%23%3F.kdbx", "a#?.kdbx"),
        ("named&amp;x.kdbx", "named&x.kdbx", "named&x.kdbx"),
        ("decimal&#38;x.kdbx", "decimal&x.kdbx", "decimal&x.kdbx"),
        ("cdata<![CDATA[x.kdbx]]>", "cdatax.kdbx", "cdatax.kdbx"),
        (
            "comment<!--split-->x.kdbx",
            "commentx.kdbx",
            "commentx.kdbx",
        ),
        ("percent&#37;20x.kdbx", "percent%20x.kdbx", "percent x.kdbx"),
        (
            "literal&amp;amp;x.kdbx",
            "literal&amp;x.kdbx",
            "literal&amp;x.kdbx",
        ),
    ];
    let root = "/dav%20root/vaults/";
    let listing = format!(
        "<d:multistatus xmlns:d=\"DAV:\">{}</d:multistatus>",
        cases.iter().map(|(xml, _, _)| format!(
            "<d:response><d:href>{root}{xml}</d:href><d:propstat><d:prop><d:getcontentlength>&#56;</d:getcontentlength><d:getlastmodified>Mon, <![CDATA[01 Jan 2024]]> 00:00:00 GMT</d:getlastmodified></d:prop></d:propstat></d:response>"
        )).collect::<String>()
    );
    let mut initial = cases
        .iter()
        .map(|(_, uri, _)| (format!("{root}{uri}"), b"original".to_vec()))
        .collect::<BTreeMap<_, _>>();
    // A sibling whose literal percent-looking name would receive a doubly
    // encoded request must remain untouched.
    initial.insert(format!("{root}a%252520b.kdbx"), b"sentinel".to_vec());
    let mut expected = initial.clone();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut requests = Vec::new();
        let mut files = initial;
        for _ in 0..1 + cases.len() * 3 {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            return (requests, files);
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("synthetic WebDAV peer accept failed: {e}"),
                }
            };
            configure_s3_page_stream(&stream);
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut header = Vec::new();
            let mut byte = [0];
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16 * 1024, "bounded fixture header");
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            let mut request = header.lines().next().unwrap().split_whitespace();
            let method = request.next().unwrap();
            let target = request.next().unwrap();
            let length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            assert!(length <= 1024, "bounded synthetic payload");
            let mut data = vec![0; length];
            stream.read_exact(&mut data).unwrap();
            requests.push((method.to_owned(), target.to_owned()));
            let (status, body) = match method {
                "PROPFIND" if target == root => ("207 Multi-Status", listing.as_bytes().to_vec()),
                "GET" if files.contains_key(target) => ("200 OK", files[target].clone()),
                "PUT" if files.contains_key(target) => {
                    files.insert(target.to_owned(), data);
                    ("204 No Content", Vec::new())
                }
                _ => ("404 Not Found", Vec::new()),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        }
        (requests, files)
    });
    let storage = WebDavStorage {
        client: reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .unwrap(),
        auth: None,
        base_url: format!("http://{addr}/dav%20root"),
        list_timeout: Duration::from_secs(3),
        io_timeout: Duration::from_secs(3),
    };
    let objects = storage.list("vaults").unwrap();
    assert_eq!(objects.len(), cases.len());
    for (i, (object, (_, uri, logical))) in objects.iter().zip(cases).enumerate() {
        assert_eq!(object.key, format!("vaults/{logical}"));
        assert_eq!(object.size, 8);
        assert_eq!(
            object.modified.as_deref(),
            Some("Mon, 01 Jan 2024 00:00:00 GMT")
        );
        assert_eq!(storage.get(&object.key).unwrap(), b"original");
        let data = format!("updated-{i}").into_bytes();
        storage.put(&object.key, &data).unwrap();
        assert_eq!(storage.get(&object.key).unwrap(), data);
        expected.insert(format!("{root}{uri}"), data);
    }
    let (requests, files) = peer.join().unwrap();
    assert_eq!(requests.len(), 1 + cases.len() * 3);
    assert_eq!(
        files, expected,
        "only the listed resources may be overwritten"
    );
    for (requests, (_, uri, _)) in requests[1..].chunks_exact(3).zip(cases) {
        for ((method, target), expected_method) in requests.iter().zip(["GET", "PUT", "GET"]) {
            assert_eq!(method, expected_method);
            assert_eq!(target, &format!("{root}{uri}"));
        }
    }
}
