//! Command-layer unit tests (extracted from commands.rs).

use super::*;
use std::io::Write;
use tempfile::TempDir;

fn write_file(dir: &TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn read_text_file_accepts_csv_and_xml_and_rejects_others() {
    let dir = TempDir::new().unwrap();
    let csv = write_file(&dir, "import.csv", "title,username,password\n");
    assert_eq!(read_text_file(csv).unwrap(), "title,username,password\n");

    let xml = write_file(&dir, "vault.kdbx.xml", "<KeePassFile/>");
    assert_eq!(read_text_file(xml).unwrap(), "<KeePassFile/>");

    let json = write_file(&dir, "bitwarden.json", "{\"items\":[]}");
    assert_eq!(read_text_file(json).unwrap(), "{\"items\":[]}");

    let pif = write_file(&dir, "export.1pif", "***Top of File***\n");
    assert_eq!(read_text_file(pif).unwrap(), "***Top of File***\n");

    let txt = write_file(&dir, "notes.txt", "secret local text");
    let err = read_text_file(txt).unwrap_err();
    assert!(err.contains(".csv"), "unexpected error: {err}");

    let no_ext = write_file(&dir, "config", "{}");
    assert!(read_text_file(no_ext).unwrap_err().contains(".csv"));
}

#[test]
fn read_text_file_rejects_missing_path() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("nope.csv").to_string_lossy().into_owned();
    assert!(read_text_file(missing).unwrap_err().contains("失败"));
}

#[test]
fn parse_proxy_server_handles_wininet_forms() {
    assert_eq!(
        parse_proxy_server("127.0.0.1:51400").as_deref(),
        Some("127.0.0.1:51400")
    );
    assert_eq!(
        parse_proxy_server("host:8080;secure=10.0.0.1:8443").as_deref(),
        Some("10.0.0.1:8443")
    );
    assert_eq!(
        parse_proxy_server("http=127.0.0.1:7890;https=127.0.0.1:7891").as_deref(),
        Some("127.0.0.1:7891")
    );
    assert_eq!(
        parse_proxy_server("https=proxy.local:3128").as_deref(),
        Some("proxy.local:3128")
    );
    assert_eq!(
        parse_proxy_server("ftp=ftp.local:21;http=127.0.0.1:8080").as_deref(),
        Some("127.0.0.1:8080")
    );
    assert_eq!(
        parse_proxy_server("http://127.0.0.1:51400").as_deref(),
        Some("127.0.0.1:51400")
    );
    assert_eq!(parse_proxy_server("").as_deref(), None);
    assert_eq!(parse_proxy_server("ftp=ftp.local:21").as_deref(), None);
}

#[test]
fn looks_like_image_sniffs_common_formats() {
    assert!(looks_like_image(&[0x00, 0x00, 0x01, 0x00])); // ICO
    assert!(looks_like_image(&[0x00, 0x00, 0x02, 0x00])); // CUR
    assert!(looks_like_image(&[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A
    ])); // PNG
    assert!(looks_like_image(&[0xFF, 0xD8, 0xFF, 0xE0])); // JPEG
    assert!(looks_like_image(b"GIF89a"));
    assert!(looks_like_image(b"BM\x36\x00\x00\x00")); // BMP
    assert!(looks_like_image(b"RIFF\x00\x00\x00\x00WEBPVP8 ")); // WebP
    assert!(looks_like_image(
        b"<svg xmlns=\"http://www.w3.org/2000/svg\">"
    ));
    assert!(looks_like_image(b"\xEF\xBB\xBF<svg"));
    assert!(looks_like_image(b"<?xml version=\"1.0\"?><svg/>"));
    // A 200-with-HTML soft-404 must not be accepted as an icon.
    assert!(!looks_like_image(
        b"<!DOCTYPE html><html><head><title>404</title>"
    ));
    assert!(!looks_like_image(b"not an image at all"));
}

#[test]
fn favicon_link_urls_resolves_and_orders_link_tags() {
    let html = r#"<!DOCTYPE html>
<html><head>
<link rel="shortcut icon" href="/static/favicon.ico">
<link REL="ICON" HREF='https://cdn.example.com/favicon-32.png'>
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="icon" href="favicon.svg">
<link rel="icon" href="data:image/x-icon;base64,AAAA">
<link rel="stylesheet" href="/style.css">
<link href="/no-rel.png">
</head></html>"#;
    let urls = favicon_link_urls(html, "https://example.com/");
    assert_eq!(
        urls,
        vec![
            "https://example.com/static/favicon.ico",
            "https://cdn.example.com/favicon-32.png",
            "https://example.com/favicon.svg",
            "https://example.com/apple-touch-icon.png",
        ]
    );
}

#[test]
fn favicon_link_urls_resolves_scheme_relative_hrefs_against_base() {
    let html = r#"<html><head>
<link rel="icon" href="//cdn.example.org/i.png">
<link rel="icon" href="icon.svg">
<link rel="icon" href="../up.png">
<link rel="icon" href="javascript:void(0)">
</head></html>"#;
    let urls = favicon_link_urls(html, "https://example.com/sub/");
    assert_eq!(
        urls,
        vec![
            "https://cdn.example.org/i.png",
            "https://example.com/sub/icon.svg",
            "https://example.com/up.png",
        ]
    );
    // Over http the scheme-relative href inherits http.
    let urls = favicon_link_urls(html, "http://example.com/");
    assert_eq!(
        urls,
        vec![
            "http://cdn.example.org/i.png",
            "http://example.com/icon.svg",
            "http://example.com/up.png",
        ]
    );
}

#[test]
fn favicon_link_urls_reads_rel_after_href_and_unquoted_attrs() {
    let html = "<html><head><link href=/a.png rel=icon><LINK href=/b.ico REL='shortcut icon'></head></html>";
    assert_eq!(
        favicon_link_urls(html, "http://ex.com/"),
        vec!["http://ex.com/a.png", "http://ex.com/b.ico"]
    );
}

#[test]
fn favicon_link_urls_rejects_binary_body() {
    let body = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01, 0x02, 0x03,
    ];
    let text = String::from_utf8_lossy(&body);
    assert!(favicon_link_urls(&text, "https://e.com/").is_empty());
}

#[test]
fn hibp_superseded_run_stops_even_after_reset_clears_flag() {
    let cancel = HibpCancel::default();
    let first = cancel.reset();
    assert!(!cancel.should_stop(first));
    cancel.cancel();
    assert!(cancel.should_stop(first));
    // A second run bumps the epoch and clears the flag: the stale first run
    // must still stop instead of finishing late work.
    let second = cancel.reset();
    assert_ne!(first, second);
    assert!(cancel.should_stop(first));
    assert!(!cancel.should_stop(second));
    cancel.cancel();
    assert!(cancel.should_stop(second));
}

#[test]
fn favicon_superseded_run_stops_even_after_reset_clears_flag() {
    let cancel = FaviconCancel::default();
    let first = cancel.reset();
    assert!(!cancel.should_stop(first));
    cancel.cancel();
    assert!(cancel.should_stop(first));
    let second = cancel.reset();
    assert_ne!(first, second);
    assert!(cancel.should_stop(first));
    assert!(!cancel.should_stop(second));
    cancel.cancel();
    assert!(cancel.should_stop(second));
}

#[tokio::test]
async fn hibp_cancel_wakes_a_parked_waiter() {
    let cancel = HibpCancel::default();
    let notify = cancel.notify.clone();
    let waiter = tokio::spawn(async move {
        notify.notified().await;
    });
    // Give the waiter a chance to park before firing cancel.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    cancel.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
        .await
        .expect("cancel must wake parked waiter")
        .unwrap();
}

#[tokio::test]
async fn favicon_cancel_wakes_a_parked_waiter() {
    let cancel = FaviconCancel::default();
    let notify = cancel.notify.clone();
    let waiter = tokio::spawn(async move {
        notify.notified().await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    cancel.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
        .await
        .expect("cancel must wake parked waiter")
        .unwrap();
}

/// Minimal HIBP mock: serves one request, optionally asserting the path
/// carries only the 5-char prefix, then (after `delay`) answers `body`.
fn spawn_hibp_mock(
    delay: std::time::Duration,
    body: &'static str,
    expected_path: Option<&'static str>,
) -> (String, std::thread::JoinHandle<()>) {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = String::new();
        let mut buf = [0u8; 1];
        while !head.ends_with("\r\n\r\n") {
            if stream.read(&mut buf).unwrap_or(0) == 0 {
                break;
            }
            head.push(buf[0] as char);
        }
        if let Some(expected) = expected_path {
            let path = head
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("");
            assert_eq!(path, expected, "full hash must never leave the client");
        }
        std::thread::sleep(delay);
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
    });
    (format!("http://{addr}/range/"), handle)
}

fn hibp_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap()
}

fn hibp_test_entries() -> Vec<(String, String, String, String)> {
    // SHA-1("password") = 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8
    vec![(
        "uuid-1".to_owned(),
        "GitHub".to_owned(),
        "octocat".to_owned(),
        "password".to_owned(),
    )]
}

#[tokio::test]
async fn hibp_range_check_matches_locally_and_reports_progress() {
    let (base, handle) = spawn_hibp_mock(
        std::time::Duration::from_millis(0),
        "1E4C9B93F3F0682250B6CF8331B7EE68FD8:42\n",
        Some("/range/5BAA6"),
    );
    let client = hibp_test_client();
    let cancel = HibpCancel::default();
    let epoch = cancel.reset();
    let progress = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let progress_task = progress.clone();
    let findings = check_hibp_entries(
        hibp_test_entries(),
        &client,
        &base,
        &cancel,
        epoch,
        move |done, total| {
            progress_task.lock().unwrap().push((done, total));
        },
    )
    .await
    .unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].uuid, "uuid-1");
    assert_eq!(findings[0].title, "GitHub");
    assert_eq!(findings[0].count, 42);
    assert_eq!(*progress.lock().unwrap(), vec![(1, 1)]);
    handle.join().unwrap();
}

#[tokio::test]
async fn hibp_range_check_aborts_promptly_on_cancel() {
    // The server never answers in time: the run must abort via cancel,
    // not via the client timeout.
    let (base, _server) = spawn_hibp_mock(
        std::time::Duration::from_secs(30),
        "1E4C9B93F3F0682250B6CF8331B7EE68FD8:42\n",
        None,
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    let cancel = HibpCancel::default();
    let epoch = cancel.reset();
    let run = check_hibp_entries(
        hibp_test_entries(),
        &client,
        &base,
        &cancel,
        epoch,
        |_, _| {},
    );
    tokio::pin!(run);
    // Let the request go out, then cancel mid-flight.
    tokio::select! {
        _ = &mut run => panic!("run must not finish before cancel"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(300)) => {
            cancel.cancel();
        }
    }
    let findings = tokio::time::timeout(std::time::Duration::from_secs(10), run)
        .await
        .expect("cancelled run must abort promptly")
        .unwrap();
    assert!(findings.is_empty());
}

/// Minimal byte server: serves one request with `body` after `delay`.
fn spawn_bytes_server(
    delay: std::time::Duration,
    body: &'static [u8],
) -> (String, std::thread::JoinHandle<()>) {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = String::new();
        let mut buf = [0u8; 1];
        while !head.ends_with("\r\n\r\n") {
            if stream.read(&mut buf).unwrap_or(0) == 0 {
                break;
            }
            head.push(buf[0] as char);
        }
        std::thread::sleep(delay);
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len(),
        );
        let _ = std::io::Write::write_all(&mut stream, body);
    });
    (format!("http://{addr}/icon.png"), handle)
}

const TEST_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01, 0x02, 0x03,
];

fn favicon_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap()
}

#[tokio::test]
async fn favicon_fetch_bytes_returns_none_when_precancelled() {
    let cancel = FaviconCancel::default();
    let epoch = cancel.reset();
    cancel.cancel();
    let out = fetch_bytes(
        &favicon_test_client(),
        "http://127.0.0.1:9/unused.png",
        512 * 1024,
        &cancel,
        epoch,
    )
    .await;
    assert!(out.is_none());
}

#[tokio::test]
async fn favicon_fetch_bytes_returns_body_on_fast_response() {
    let (url, handle) = spawn_bytes_server(std::time::Duration::from_millis(0), TEST_PNG);
    let cancel = FaviconCancel::default();
    let epoch = cancel.reset();
    let out = fetch_bytes(&favicon_test_client(), &url, 512 * 1024, &cancel, epoch).await;
    assert_eq!(out.unwrap(), TEST_PNG);
    handle.join().unwrap();
}

#[tokio::test]
async fn favicon_fetch_bytes_aborts_slow_response_on_cancel() {
    // The server never answers in time: the fetch must abort via cancel,
    // not via the client timeout.
    let (url, _server) = spawn_bytes_server(std::time::Duration::from_secs(30), TEST_PNG);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    let cancel = FaviconCancel::default();
    let epoch = cancel.reset();
    let run = fetch_bytes(&client, &url, 512 * 1024, &cancel, epoch);
    tokio::pin!(run);
    // Let the request go out, then cancel mid-flight.
    tokio::select! {
        _ = &mut run => panic!("fetch must not finish before cancel"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(300)) => {
            cancel.cancel();
        }
    }
    let out = tokio::time::timeout(std::time::Duration::from_secs(10), run)
        .await
        .expect("cancelled fetch must abort promptly");
    assert!(out.is_none());
}
