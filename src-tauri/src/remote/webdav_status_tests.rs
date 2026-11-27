//! RFC 4918 sections 9.1/13/14.28: status applies to one propstat or response.
//! All file names/metadata and HTTP peers below are disposable synthetic data.
use super::*;

fn document(response: &str) -> String {
    format!("<d:multistatus xmlns:d=\"DAV:\" xmlns:x=\"urn:synthetic\">{response}</d:multistatus>")
}

fn props(status: &str, properties: &str, status_first: bool) -> String {
    let status = format!("<d:status>{status}</d:status>");
    let properties = format!("<d:prop>{properties}</d:prop>");
    format!(
        "<d:propstat>{}{}</d:propstat>",
        if status_first { &status } else { &properties },
        if status_first { &properties } else { &status }
    )
}

fn parse(response: &str) -> Result<Vec<RemoteObject>, String> {
    parse_multistatus(&document(response), "http://h/dav", "http://h/dav/")
}

#[test]
fn webdav_status_failed_properties_never_replace_successful_metadata() {
    for status_first in [false, true] {
        for failed_first in [false, true] {
            let good = props("HTTP/1.1 200 OK", "<d:getcontentlength>7</d:getcontentlength><d:getlastmodified>synthetic-current</d:getlastmodified>", status_first);
            let bad = props("HTTP/1.1 404 Not Found", "<d:getcontentlength>999</d:getcontentlength><d:getlastmodified>synthetic-failed</d:getlastmodified>", status_first);
            let blocks = if failed_first {
                format!("{bad}{good}")
            } else {
                format!("{good}{bad}")
            };
            let objects = parse(&format!(
                "<d:response><d:href>/dav/a.kdbx</d:href>{blocks}</d:response>"
            ))
            .unwrap();
            assert_eq!(objects.len(), 1);
            assert_eq!(objects[0].size, 7);
            assert_eq!(objects[0].modified.as_deref(), Some("synthetic-current"));
        }
    }
    for code in [100, 301, 403, 404, 500, 599] {
        let properties = props(&format!("HTTP/1.1 {code} Synthetic"), "<d:getcontentlength>99</d:getcontentlength><d:getlastmodified>synthetic-failed</d:getlastmodified>", false);
        let objects = parse(&format!(
            "<d:response><d:href>/dav/a.kdbx</d:href>{properties}</d:response>"
        ))
        .unwrap();
        assert_eq!(objects[0].size, 0);
        assert!(objects[0].modified.is_none());
    }
}

#[test]
fn webdav_status_only_successful_collection_properties_hide_files() {
    let collection = "<d:resourcetype><d:collection/></d:resourcetype>";
    for status_first in [false, true] {
        let failed = props("HTTP/1.1 404 Not Found", collection, status_first);
        let good = props(
            "HTTP/1.1 200 OK",
            "<d:getcontentlength>7</d:getcontentlength>",
            !status_first,
        );
        let objects = parse(&format!(
            "<d:response><d:href>/dav/backup</d:href>{failed}{good}</d:response>"
        ))
        .unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].key, "backup");
        assert_eq!(objects[0].size, 7);
        let successful = props("HTTP/1.1 200 OK", collection, status_first);
        assert!(parse(&format!(
            "<d:response><d:href>/dav/sub</d:href>{successful}{good}</d:response>"
        ))
        .unwrap()
        .is_empty());
    }
}

#[test]
fn webdav_status_response_failures_are_distinct_from_unavailable_properties() {
    for code in [200, 204, 206, 301, 403, 404, 500] {
        for status_first in [false, true] {
            let status = format!("<d:status>HTTP/1.1 {code} Synthetic</d:status>");
            let href = "<d:href>/dav/a.kdbx</d:href>";
            let response = if status_first {
                format!("{status}{href}")
            } else {
                format!("{href}{status}")
            };
            let objects = parse(&format!("<d:response>{response}</d:response>")).unwrap();
            assert_eq!(objects.len(), usize::from((200..300).contains(&code)));
        }
    }
    let compatibility = "<d:response><d:href>/dav/a.kdbx</d:href><d:propstat><d:prop><d:getcontentlength>8</d:getcontentlength></d:prop></d:propstat></d:response>";
    assert_eq!(parse(compatibility).unwrap()[0].size, 8);
}

#[test]
fn webdav_status_invalid_or_duplicate_status_rejects_the_complete_list() {
    let valid = "<d:response><d:href>/dav/valid.kdbx</d:href></d:response>";
    for status in [
        "",
        "OK",
        "HTTP/1.1 20 OK",
        "HTTP/1.1 2000 OK",
        "HTTP/1.1 099 Bad",
        "HTTP/1.1 600 Bad",
        "HTTP/1.1 synthetic-secret OK",
        "HTTP/1.1 200\nsynthetic-secret",
    ] {
        for nested in [false, true] {
            let status = format!("<d:status>{status}</d:status>");
            let content = if nested {
                format!("<d:propstat><d:prop/>{status}</d:propstat>")
            } else {
                status
            };
            let error = parse(&format!(
                "{valid}<d:response><d:href>/dav/a.kdbx</d:href>{content}</d:response>"
            ))
            .unwrap_err();
            assert_eq!(error, "WebDAV 列表状态码无效");
            assert!(!error.contains("synthetic-secret"));
        }
    }
    for content in ["<d:status/>", "<d:status>HTTP/1.1 200 OK</d:status><d:status>HTTP/1.1 404 Not Found</d:status>", "<d:propstat><d:status>HTTP/1.1 200 OK</d:status><d:status>HTTP/1.1 200 OK</d:status></d:propstat>"] {
        assert_eq!(parse(&format!("{valid}<d:response><d:href>/dav/a.kdbx</d:href>{content}</d:response>")).unwrap_err(), "WebDAV 列表状态码无效");
    }
}

#[test]
fn webdav_status_uses_complete_dav_text_and_ignores_foreign_statuses() {
    let status =
        "<d:status>HTTP/1.1 &#50;<![CDATA[06]]><!--synthetic--> Partial Content</d:status>";
    let response = format!("<d:response><d:href>/dav/a.kdbx</d:href><x:status>HTTP/1.1 404 Not Found</x:status><d:propstat><d:prop><d:getcontentlength>8</d:getcontentlength></d:prop>{status}<x:status>invalid</x:status></d:propstat></d:response>");
    assert_eq!(parse(&response).unwrap()[0].size, 8);
    let ignored = "<d:response><d:href>/dav/a.kdbx</d:href><x:extension><d:status>invalid</d:status></x:extension></d:response>";
    assert_eq!(parse(ignored).unwrap()[0].key, "a.kdbx");
}

#[test]
fn webdav_status_http_list_applies_inner_statuses_and_rejects_ambiguous_lists() {
    use crate::remote::RemoteStorage;
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};

    let good = props("HTTP/1.1 200 OK", "<d:getcontentlength>7</d:getcontentlength><d:getlastmodified>synthetic-current</d:getlastmodified>", false);
    let bad = props("HTTP/1.1 404 Not Found", "<d:getcontentlength>999</d:getcontentlength><d:resourcetype><d:collection/></d:resourcetype>", true);
    let bodies = [
        document(&format!("<d:response><d:href>/dav/backup</d:href>{good}{bad}</d:response>")),
        document(&format!("<d:response><d:href>/dav/a.kdbx</d:href>{bad}{good}</d:response>")),
        document("<d:response><d:href>/dav/absent.kdbx</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>"),
        document("<d:response><d:href>/dav/valid.kdbx</d:href></d:response><d:response><d:href>/dav/ambiguous.kdbx</d:href><d:status>HTTP/1.1 200 OK</d:status><d:status>HTTP/1.1 404 Not Found</d:status></d:response>"),
    ];
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        for body in bodies {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "synthetic accept deadline");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("synthetic accept: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut header = Vec::new();
            let mut byte = [0];
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16 * 1024);
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            assert!(header.starts_with("PROPFIND /dav/ HTTP/1.1\r\n"));
            assert!(!header.to_ascii_lowercase().contains("authorization:"));
            assert!(header.to_ascii_lowercase().contains("depth: 1\r\n"));
            let length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            assert!(length <= 1024);
            let mut request = vec![0; length];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(request, PROPFIND_BODY.as_bytes());
            let response = format!("HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    let storage = WebDavStorage {
        client: crate::remote::shared_blocking_client().unwrap().clone(),
        auth: None,
        base_url: format!("http://{addr}/dav"),
        list_timeout: Duration::from_secs(3),
        io_timeout: Duration::from_secs(3),
    };
    for key in ["backup", "a.kdbx"] {
        let objects = storage.list("").unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].key, key);
        assert_eq!(objects[0].size, 7);
        assert_eq!(objects[0].modified.as_deref(), Some("synthetic-current"));
    }
    assert!(storage.list("").unwrap().is_empty());
    assert_eq!(storage.list("").unwrap_err(), "WebDAV 列表状态码无效");
    peer.join().unwrap();
}
