//! Tiny synthetic fixtures, including independent Python ZIP output. No user data.
use super::*;
use std::io::Write;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const STORED: &[u8] = include_bytes!("../../../tests/fixtures/onepux/stored.1pux");
const DEFLATED: &[u8] = include_bytes!("../../../tests/fixtures/onepux/deflated.1pux");
const LOCAL64: &[u8] = include_bytes!("../../../tests/fixtures/onepux/local-zip64.1pux");
const DIRECTORY64: &[u8] = include_bytes!("../../../tests/fixtures/onepux/directory-zip64.1pux");
const DUPLICATE: &[u8] = include_bytes!("../../../tests/fixtures/onepux/duplicate.1pux");

fn rejects(bytes: &[u8]) {
    assert!(matches!(decode(bytes, LIMITS), Err(INVALID | LIMIT)));
}

fn make(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in members {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn with_extra(name: &str, content: &[u8]) -> Vec<u8> {
    make(&[
        ("export.attributes", br#"{"version":3}"#),
        ("export.data", br#"{"accounts":[]}"#),
        (name, content),
    ])
}

fn central(bytes: &[u8]) -> usize {
    u32_at(bytes, bytes.len() - 6).unwrap() as usize
}

fn patch16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn patch32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn independent_stored_deflate_and_zip64_preserve_exact_binary_and_empty_files() {
    for fixture in [STORED, DEFLATED, LOCAL64, DIRECTORY64] {
        let archive = parse_1pux_archive(fixture).unwrap();
        assert_eq!(archive.attributes()["version"], 3);
        assert!(archive.data()["accounts"].as_array().unwrap().is_empty());
        assert_eq!(archive.files().len(), 3);
        assert!(archive.files()["files/empty__empty.bin"].is_empty());
        assert_eq!(
            &**archive.files().get("files/blob__note.txt").unwrap(),
            b"synthetic attachment\x00\xff"
        );
        assert_eq!(
            &**archive.files().get("files/nested/笔记.txt").unwrap(),
            "合成附件".as_bytes()
        );
    }
}

#[test]
fn nested_directory_entries_are_accepted_without_extraction() {
    assert!(parse_1pux_archive(&with_extra("files/nested/", b""))
        .unwrap()
        .files()
        .is_empty());
    rejects(&with_extra("files/nested/", b"not empty"));
}

#[test]
fn missing_invalid_or_wrong_version_metadata_rejects_whole_archive() {
    for members in [
        vec![("export.data", &br#"{"accounts":[]}"#[..])],
        vec![("export.attributes", &br#"{"version":3}"#[..])],
        vec![
            ("export.attributes", &br#"{"version":4}"#[..]),
            ("export.data", &br#"{"accounts":[]}"#[..]),
        ],
        vec![
            ("export.attributes", &br#"{"version":"3"}"#[..]),
            ("export.data", &br#"{"accounts":[]}"#[..]),
        ],
        vec![
            ("export.attributes", &b"{"[..]),
            ("export.data", &br#"{"accounts":[]}"#[..]),
        ],
        vec![
            ("export.attributes", &br#"{"version":3}"#[..]),
            ("export.data", &b"[]"[..]),
        ],
        vec![
            ("export.attributes", &br#"{"version":3}"#[..]),
            ("export.data", &br#"{"accounts":{}}"#[..]),
        ],
    ] {
        rejects(&make(&members));
    }
}

#[test]
fn rejects_duplicate_raw_names_before_library_deduplication() {
    assert!(matches!(preflight(DUPLICATE, LIMITS), Err(INVALID)));
    rejects(DUPLICATE);
}

#[test]
fn repeated_json_keys_and_trailing_tokens_cannot_discard_source_data() {
    for data in [
        &br#"{"accounts":[],"accounts":[]}"#[..],
        &br#"{"accounts":[],"other":{"concealed":"first","concealed":"second"}}"#[..],
        &br#"{"accounts":[]} {}"#[..],
    ] {
        rejects(&make(&[
            ("export.attributes", br#"{"version":3}"#),
            ("export.data", data),
        ]));
    }
    rejects(&make(&[
        ("export.attributes", br#"{"version":2,"version":3}"#),
        ("export.data", br#"{"accounts":[]}"#),
    ]));
}

#[test]
fn json_primitive_values_and_escape_sequences_retain_semantics() {
    let bytes = make(&[("export.attributes", br#"{"version":3}"#), ("export.data", br#"{"accounts":[],"values":[null,true,false,-1,18446744073709551615,1.5,"line\n\u4e2d"]}"#)]);
    let archive = parse_1pux_archive(&bytes).unwrap();
    let expected: Value =
        serde_json::from_slice(br#"[null,true,false,-1,18446744073709551615,1.5,"line\n\u4e2d"]"#)
            .unwrap();
    assert_eq!(archive.data()["values"], expected);
}

#[test]
fn rejects_unsafe_and_unexpected_paths_without_echoing_them() {
    for name in [
        "/files/a",
        "files/../a",
        "files/./a",
        "files//a",
        "files/a\\b",
        "files/\na",
        "files/a//",
        "other/a",
        "C:/a",
        "export.data/",
        "",
    ] {
        rejects(&with_extra(name, b"synthetic"));
    }
    rejects(&with_extra(&format!("files/{}", "a".repeat(1020)), b""));
    let mut bytes = STORED.to_vec();
    let directory = central(&bytes);
    bytes[directory + 46] = 0xff;
    rejects(&bytes);
    assert_eq!(
        parse_1pux_archive(&with_extra(
            "files/../synthetic-private-label",
            b"secret-marker"
        ))
        .err()
        .as_deref(),
        Some(INVALID)
    );
}

#[test]
fn rejects_crc_damage_and_declared_size_mismatch() {
    let mut bytes = STORED.to_vec();
    let start = ZipArchive::new(Cursor::new(STORED))
        .unwrap()
        .by_name("files/blob__note.txt")
        .unwrap()
        .data_start()
        .unwrap() as usize;
    bytes[start] ^= 1;
    rejects(&bytes);
    for size in [0, 1, 100] {
        let mut bytes = STORED.to_vec();
        let directory = central(&bytes);
        patch32(&mut bytes, directory + 24, size);
        rejects(&bytes);
    }
}

#[test]
fn rejects_truncation_and_trailing_garbage() {
    for end in 0..STORED.len() {
        rejects(&STORED[..end]);
    }
    let mut bytes = STORED.to_vec();
    bytes.extend_from_slice(b"extra");
    rejects(&bytes);
}

#[test]
fn rejects_encryption_special_files_and_unsupported_compression() {
    for kind in [0o120000u32, 0o010000, 0o020000, 0o060000, 0o140000] {
        let mut bytes = STORED.to_vec();
        let directory = central(&bytes);
        patch32(&mut bytes, directory + 38, (kind | 0o600) << 16);
        rejects(&bytes);
    }
    let mut bytes = STORED.to_vec();
    let directory = central(&bytes);
    patch16(&mut bytes, directory + 8, 1);
    patch16(&mut bytes, 6, 1);
    rejects(&bytes);
    let mut bytes = STORED.to_vec();
    patch16(&mut bytes, directory + 10, 99);
    patch16(&mut bytes, 8, 99);
    rejects(&bytes);
}

#[test]
fn rejects_multidisk_and_inconsistent_central_directory() {
    for field in [4, 6, 8, 10] {
        let mut bytes = STORED.to_vec();
        let end = bytes.len() - 22;
        patch16(&mut bytes, end + field, 1);
        rejects(&bytes);
    }
    for field in [12, 16] {
        let mut bytes = STORED.to_vec();
        let end = bytes.len() - 22;
        patch32(&mut bytes, end + field, u32::MAX - 1);
        rejects(&bytes);
    }
    let mut bytes = STORED.to_vec();
    let directory = central(&bytes);
    patch16(&mut bytes, directory + 34, 1);
    rejects(&bytes);
}

#[test]
fn rejects_malformed_zip64_records_with_tiny_samples() {
    let zip64 = LOCAL64.len() - 22;
    for (offset, replacement) in [
        (zip64, 0),
        (zip64 + 4, 43),
        (zip64 + 16, 1),
        (zip64 + 20, 1),
        (zip64 + 24, 1),
        (zip64 + 40, 0),
        (zip64 + 48, 0),
        (zip64 + 56 + 4, 1),
        (zip64 + 56 + 16, 2),
    ] {
        let mut bytes = DIRECTORY64.to_vec();
        bytes[offset] = replacement;
        rejects(&bytes);
    }
    let mut bytes = DIRECTORY64.to_vec();
    bytes[zip64 + 56 + 8..zip64 + 56 + 16].copy_from_slice(&u64::MAX.to_le_bytes());
    rejects(&bytes);
}

#[test]
fn applies_all_resource_limits_before_or_during_small_reads() {
    for limits in [
        Limits {
            compressed: STORED.len() - 1,
            ..LIMITS
        },
        Limits {
            members: 5,
            ..LIMITS
        },
        Limits { json: 10, ..LIMITS },
        Limits {
            member: 10,
            ..LIMITS
        },
        Limits {
            expanded: 50,
            ..LIMITS
        },
    ] {
        assert!(matches!(decode(STORED, limits), Err(LIMIT)));
    }
    assert!(decode(
        STORED,
        Limits {
            compressed: STORED.len(),
            members: 6,
            json: 47,
            member: 22,
            expanded: 96
        }
    )
    .is_ok());
    let mut bytes = STORED.to_vec();
    let end = bytes.len() - 22;
    patch16(&mut bytes, end + 8, 8193);
    patch16(&mut bytes, end + 10, 8193);
    assert!(matches!(preflight(&bytes, LIMITS), Err(LIMIT)));
}

#[test]
fn permits_zip_comment_and_rejects_overlapping_members() {
    let mut bytes = STORED.to_vec();
    let end = bytes.len() - 22;
    patch16(&mut bytes, end + 20, 7);
    bytes.extend_from_slice(b"comment");
    assert!(parse_1pux_archive(&bytes).is_ok());
    let mut bytes = STORED.to_vec();
    let first = central(&bytes);
    let second = first + 46 + usize::from(u16_at(&bytes, first + 28).unwrap());
    patch32(&mut bytes, second + 42, 0);
    rejects(&bytes);
}
