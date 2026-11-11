use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn document() -> Zeroizing<Vec<u8>> {
    let mut database = Database::new();
    let id = {
        let mut root = database.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", "合成 gzip\r\ncontrol");
        entry.set_unprotected("Empty", "");
        entry.add_attachment("empty.bin", Value::protected(Vec::new()));
        entry.add_attachment("small.bin", Value::unprotected(vec![0, 1, 255]));
        entry.as_ref().id()
    };
    Zeroizing::new(build_entries_document(&database, &[id]).unwrap())
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let framed = Zeroizing::new(encode_payload(bytes, false).unwrap());
    framed[4..].to_vec()
}

fn members(parts: &[&[u8]]) -> Zeroizing<Vec<u8>> {
    let mut payload = Zeroizing::new(vec![0, 0, 0, 0]);
    for part in parts {
        payload.extend_from_slice(&gzip(part));
    }
    payload
}

fn partitions(document: &[u8]) -> Vec<Vec<&[u8]>> {
    // Split within UTF-8 as well as XML markup: gzip members contain bytes,
    // and only their combined output is the complete document.
    let inside_utf8 = document
        .windows(3)
        .position(|bytes| bytes == "合".as_bytes())
        .unwrap()
        + 1;
    vec![
        vec![document],
        vec![b"", document],
        vec![document, b""],
        vec![&document[..inside_utf8], &document[inside_utf8..]],
        vec![
            &document[..1],
            &document[1..inside_utf8],
            &document[inside_utf8..],
        ],
    ]
}

fn suffixes(document: &[u8]) -> Vec<Vec<u8>> {
    let empty = gzip(b"");
    let mut bad_crc = empty.clone();
    let mut bad_size = empty.clone();
    let mut truncated = empty.clone();
    let length = empty.len();
    bad_crc[length - 8] ^= 1;
    bad_size[length - 4] ^= 1;
    truncated.pop();
    vec![
        b"!".to_vec(),
        vec![0x1f],
        gzip(b"!"),
        gzip(document),
        bad_crc,
        bad_size,
        truncated,
    ]
}

#[test]
fn exchange_gzip_complete_members_keep_the_combined_document() {
    let original = document();
    let mut incomplete = Vec::new();
    for (index, parts) in partitions(&original).into_iter().enumerate() {
        let payload = members(&parts);
        match decode_payload(&payload) {
            Ok(decoded) if decoded == *original => {
                let entries = parse_entries_document(&decoded).unwrap();
                assert_eq!(entries.len(), 1);
                assert!(entries[0]
                    .fields
                    .iter()
                    .any(|(name, value, _)| name == "Title" && value == "合成 gzip\r\ncontrol"));
                assert_eq!(entries[0].attachments.len(), 2);
            }
            Ok(decoded) => incomplete.push((index, decoded.len())),
            Err(_) => incomplete.push((index, 0)),
        }
    }
    assert!(
        incomplete.is_empty(),
        "incomplete member cases: {incomplete:?}"
    );
}

#[test]
fn exchange_gzip_suffix_damage_never_publishes_a_document() {
    let original = document();
    let first = gzip(&original);
    let mut published = Vec::new();
    for (index, suffix) in suffixes(&original).into_iter().enumerate() {
        let mut payload = Zeroizing::new(vec![0, 0, 0, 0]);
        payload.extend_from_slice(&first);
        payload.extend_from_slice(&suffix);
        match decode_payload(&payload).and_then(|decoded| parse_entries_document(&decoded)) {
            Ok(entries) => published.push((index, entries.len())),
            Err(error) => assert!(
                error == "剪贴板数据已损坏" || error == "剪贴板文档格式无效",
                "suffix {index}: error contract"
            ),
        }
    }
    assert!(
        published.is_empty(),
        "published suffix cases: {published:?}"
    );
}

#[test]
fn exchange_gzip_output_budget_counts_all_members() {
    let mut accepted = Vec::new();
    for limit in [0, 1, 32, 4096] {
        let first = Zeroizing::new(vec![b'x'; limit]);
        let payload = members(&[&first, b"x"]);
        match gunzip_limited(&payload[4..], limit) {
            Err(error) => assert!(error == "剪贴板数据过大", "member size error contract"),
            Ok(decoded) => accepted.push((limit, decoded.len())),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted cumulative limits: {accepted:?}"
    );
    for limit in [6, usize::MAX] {
        let payload = members(&[b"abc", b"def"]);
        assert!(
            gunzip_limited(&payload[4..], limit).unwrap() == b"abcdef",
            "combined exact/unlimited bytes"
        );
    }
}

#[test]
fn exchange_gzip_exact_total_limit_checks_later_trailers() {
    let first = gzip(b"abc");
    let second = gzip(b"def");
    let mut bad_crc = second.clone();
    let mut bad_size = second.clone();
    let mut truncated = second.clone();
    let length = second.len();
    bad_crc[length - 8] ^= 1;
    bad_size[length - 4] ^= 1;
    truncated.pop();
    let mut accepted = Vec::new();
    for (index, suffix) in [bad_crc, bad_size, truncated].into_iter().enumerate() {
        let mut payload = Zeroizing::new(vec![0, 0, 0, 0]);
        payload.extend_from_slice(&first);
        payload.extend_from_slice(&suffix);
        match gunzip_limited(&payload[4..], 6) {
            Err(error) => assert!(error == "剪贴板数据已损坏", "later trailer error contract"),
            Ok(_) => accepted.push(index),
        }
    }
    assert!(accepted.is_empty(), "accepted later trailers: {accepted:?}");
}

#[test]
fn exchange_gzip_single_and_multimember_values_survive_native_reopen() {
    let original = document();
    let dir = TempDir::new().unwrap();
    for (index, parts) in partitions(&original).into_iter().enumerate() {
        let payload = members(&parts);
        let decoded = Zeroizing::new(decode_payload(&payload).unwrap());
        let entries = parse_entries_document(&decoded).unwrap();
        let path = dir
            .path()
            .join(format!("synthetic-gzip-members-{index}.kdbx"));
        let mut target = VaultSession::default();
        target
            .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
            .unwrap();
        let uuid = insert_entry(
            &mut target.require_db_mut().unwrap().root_mut(),
            &entries[0],
        )
        .unwrap();
        target.mark_dirty();
        target.save().unwrap();
        target.close();
        target.open(&path, "synthetic-master", None).unwrap();
        let entry = target
            .require_db()
            .unwrap()
            .entry(EntryId::from_uuid(uuid))
            .unwrap();
        assert_eq!(entry.get_title(), Some("合成 gzip\r\ncontrol"));
        assert_eq!(entry.get("Empty"), Some(""));
        assert_eq!(entry.attachments_named().count(), 2);
        assert!(entry
            .attachments_named()
            .any(|(name, binary)| name == "empty.bin"
                && binary.data.get().is_empty()
                && binary.data.is_protected()));
        assert!(entry
            .attachments_named()
            .any(|(name, binary)| name == "small.bin"
                && binary.data.get().as_slice() == [0, 1, 255]
                && !binary.data.is_protected()));
    }
}
