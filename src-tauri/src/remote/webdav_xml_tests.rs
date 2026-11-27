//! Whole-document syntax controls, including otherwise ignored extensions.
use super::*;

#[test]
fn webdav_full_xml_checks_syntax_even_in_ignored_extension_content() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/webdav-full-xml.json")).unwrap();
    let mut mismatches = Vec::new();
    for case in cases.as_array().unwrap() {
        let result = parse_multistatus(
            case["xml"].as_str().unwrap(),
            "http://h/dav",
            "http://h/dav/",
        );
        if result.is_err() != case["reject"].as_bool().unwrap() {
            mismatches.push(case["name"].as_str().unwrap());
        }
    }
    assert!(mismatches.is_empty(), "XML case mismatches: {mismatches:?}");
}

#[test]
fn webdav_full_xml_limits_apply_to_ignored_subtrees_and_output_objects() {
    let wrap = |content: &str| {
        format!(
            "<d:multistatus xmlns:d=\"DAV:\" xmlns:x=\"urn:synthetic\">{content}</d:multistatus>"
        )
    };
    let deep = format!("{}{}", "<x:e>".repeat(64), "</x:e>".repeat(64));
    assert!(parse_multistatus(&wrap(&deep), "http://h/dav", "http://h/dav/").is_err());
    let attributes = (0..129).map(|i| format!(" a{i}=\"v\"")).collect::<String>();
    assert!(parse_multistatus(
        &wrap(&format!("<x:e{attributes}/>")),
        "http://h/dav",
        "http://h/dav/"
    )
    .is_err());
    let responses = (0..10_001)
        .map(|i| format!("<d:response><d:href>/dav/{i}.kdbx</d:href></d:response>"))
        .collect::<String>();
    assert!(parse_multistatus(&wrap(&responses), "http://h/dav", "http://h/dav/").is_err());
}

#[test]
fn webdav_full_xml_event_attribute_and_byte_limits_have_valid_boundary_controls() {
    let wrap = |content: &str| format!("<d:multistatus xmlns:d=\"DAV:\">{content}</d:multistatus>");
    let valid_depth = format!("{}{}", "<e>".repeat(63), "</e>".repeat(63));
    assert!(parse_multistatus(&wrap(&valid_depth), "http://h/dav", "http://h/dav/").is_ok());
    let attributes = (0..128).map(|i| format!(" a{i}=\"v\"")).collect::<String>();
    let node = format!("<e{attributes}/>");
    assert!(parse_multistatus(&wrap(&node), "http://h/dav", "http://h/dav/").is_ok());
    assert_eq!(
        xml::check_limits(&wrap(&node.repeat(2049))).unwrap_err(),
        xml::limit_error()
    );
    assert_eq!(
        xml::check_limits(&wrap(&"<!--x-->".repeat(262_144))).unwrap_err(),
        xml::limit_error()
    );
    let mut boundary = wrap("");
    boundary.push_str(&" ".repeat(xml::MAX_LIST_BYTES - boundary.len()));
    assert!(parse_multistatus(&boundary, "http://h/dav", "http://h/dav/").is_ok());
    boundary.push(' ');
    assert_eq!(
        parse_multistatus(&boundary, "http://h/dav", "http://h/dav/").unwrap_err(),
        xml::limit_error()
    );
}

#[test]
fn webdav_full_xml_strict_decoding_preserves_names_across_utf_and_legacy_charsets() {
    let body = "<d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/dav/合成.kdbx</d:href></d:response></d:multistatus>";
    let mut utf8_bom = vec![0xef, 0xbb, 0xbf];
    utf8_bom.extend_from_slice(body.as_bytes());
    assert_eq!(
        xml::decode(&utf8_bom, Some("text/xml; charset=windows-1252")).unwrap(),
        body
    );
    for little in [false, true] {
        for bom in [false, true] {
            let mut bytes = if bom {
                if little {
                    vec![0xff, 0xfe]
                } else {
                    vec![0xfe, 0xff]
                }
            } else {
                Vec::new()
            };
            for unit in body.encode_utf16() {
                bytes.extend_from_slice(&if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            assert_eq!(xml::decode(&bytes, None).unwrap(), body);
        }
    }
    let legacy = "<?xml version=\"1.0\" encoding=\"iso-8859-1\"?><d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/dav/café.kdbx</d:href></d:response></d:multistatus>";
    let (bytes, _, errors) = encoding_rs::WINDOWS_1252.encode(legacy);
    assert!(!errors);
    assert_eq!(xml::decode(&bytes, None).unwrap(), legacy);
    assert_eq!(
        xml::decode(&bytes, Some("application/xml; charset=\"windows-1252\"")).unwrap(),
        legacy
    );
    for bytes in [
        &[0xff][..],
        &[0xff, 0xfe, 0x3c][..],
        &[0xff, 0xfe, 0x00, 0xd8][..],
    ] {
        assert!(xml::decode(bytes, None).is_err());
    }
    assert!(xml::decode(body.as_bytes(), Some("text/xml; charset=synthetic-unknown")).is_err());
    let oversized_decoded = vec![0xe9; xml::MAX_LIST_BYTES / 2 + 1];
    assert_eq!(
        xml::decode(&oversized_decoded, Some("text/xml; charset=windows-1252")).unwrap_err(),
        xml::limit_error()
    );
}

#[test]
fn webdav_full_xml_http_listing_decodes_strictly_and_caps_declared_or_chunked_bytes() {
    use crate::remote::RemoteStorage;
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    let body = "<?xml version=\"1.0\"?><d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/dav/café.kdbx</d:href></d:response></d:multistatus>";
    let utf16 = |little: bool| {
        let mut bytes = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in body.encode_utf16() {
            bytes.extend_from_slice(&if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        bytes
    };
    let (legacy, _, errors) = encoding_rs::WINDOWS_1252.encode(body);
    assert!(!errors);
    let cases = vec![
        (
            body.as_bytes().to_vec(),
            "application/xml; charset=utf-8",
            false,
            false,
        ),
        (utf16(true), "application/xml; charset=utf-16", false, false),
        (utf16(false), "application/xml", false, false),
        (
            legacy.into_owned(),
            "text/xml; charset=windows-1252",
            false,
            false,
        ),
        (vec![0xff], "application/xml; charset=utf-8", false, false),
        (
            b"<d:multistatus xmlns:d=\"DAV:\"><e a=\"1\" a=\"2\"/></d:multistatus>".to_vec(),
            "application/xml",
            false,
            false,
        ),
        (Vec::new(), "application/xml", true, false),
        (
            vec![b' '; xml::MAX_LIST_BYTES + 1],
            "application/xml",
            false,
            true,
        ),
    ];
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        for (bytes, content_type, oversized, chunked) in cases {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
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
            let length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            assert!(length <= 1024);
            stream.read_exact(&mut vec![0; length]).unwrap();
            let framing = if chunked {
                "Transfer-Encoding: chunked".to_owned()
            } else {
                format!(
                    "Content-Length: {}",
                    if oversized {
                        xml::MAX_LIST_BYTES + 1
                    } else {
                        bytes.len()
                    }
                )
            };
            stream.write_all(format!("HTTP/1.1 207 Multi-Status\r\nContent-Type: {content_type}\r\n{framing}\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
            if chunked {
                stream
                    .write_all(format!("{:X}\r\n", bytes.len()).as_bytes())
                    .unwrap();
                stream.write_all(&bytes).unwrap();
                let _ = stream.write_all(b"\r\n0\r\n\r\n");
            } else if !oversized {
                stream.write_all(&bytes).unwrap();
            }
        }
    });
    let storage = WebDavStorage {
        client: crate::remote::shared_blocking_client().unwrap().clone(),
        auth: None,
        base_url: format!("http://{addr}/dav"),
        list_timeout: Duration::from_secs(3),
        io_timeout: Duration::from_secs(3),
    };
    for _ in 0..4 {
        assert_eq!(storage.list("").unwrap()[0].key, "café.kdbx");
    }
    assert_eq!(
        storage.list("").unwrap_err(),
        "WebDAV 列表响应编码无效或不受支持"
    );
    assert_eq!(storage.list("").unwrap_err(), "WebDAV 列表 XML 文档无效");
    assert!(storage.list("").unwrap_err().contains("超过 8 MiB 上限"));
    assert!(storage.list("").unwrap_err().contains("超过 8 MiB 上限"));
    peer.join().unwrap();
}
