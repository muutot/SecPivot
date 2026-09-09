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
