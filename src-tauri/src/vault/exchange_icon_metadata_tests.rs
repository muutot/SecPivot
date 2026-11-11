use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn time(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(seconds, 0)
        .unwrap()
        .naive_utc()
}

fn seed(database: &mut Database, name: Option<&str>, seconds: Option<i64>) -> EntryId {
    let mut root = database.root_mut();
    let mut entry = root.add_entry();
    entry.set_unprotected("Title", "Synthetic icon metadata");
    {
        let mut icon = entry.set_icon_custom_new(vec![0, 1, 255]);
        icon.name = name.map(str::to_owned);
        icon.last_modification_time = seconds.map(time);
    }
    entry.as_ref().id()
}

fn parsed(document: &[u8]) -> Result<Vec<PastedEntry>, String> {
    let frame = Zeroizing::new(encode_payload(document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&frame).unwrap());
    assert!(*decoded == document, "complete icon metadata framing");
    parse_entries_document(&decoded)
}

fn copy(source: &Database, id: EntryId, target: &mut Database) -> EntryId {
    let bytes = Zeroizing::new(build_entries_document(source, &[id]).unwrap());
    let entries = parsed(&bytes).unwrap();
    EntryId::from_uuid(insert_entry(&mut target.root_mut(), &entries[0]).unwrap())
}

fn assert_icon(source: &Database, id: EntryId, target: &Database, copied: EntryId) {
    let source_entry = source.entry(id).unwrap();
    let target_entry = target.entry(copied).unwrap();
    assert_eq!(target_entry.get_title(), source_entry.get_title());
    assert_ne!(id, copied);
    let expected = source_entry.custom_icon().unwrap();
    let actual = target_entry.custom_icon().unwrap();
    assert_ne!(
        expected.id(),
        actual.id(),
        "pasted icon remains a new identity"
    );
    assert_eq!(actual.data, expected.data);
    assert_eq!(actual.name, expected.name, "native icon name retained");
    assert_eq!(
        actual.last_modification_time, expected.last_modification_time,
        "native icon timestamp retained"
    );
}

#[test]
fn exchange_icon_metadata_keeps_native_names_times_and_source() {
    for (name, seconds) in [
        (Some("Synthetic icon.png"), Some(1_600_000_000)),
        (Some(""), Some(0)),
        (Some(" \t\n"), Some(-1)),
        (Some("\u{a0}\u{3000}"), None),
        (Some("Synthetic\r\nname\r"), None),
        (None, Some(1_600_000_000)),
        (None, None),
    ] {
        let mut source = Database::new();
        let id = seed(&mut source, name, seconds);
        let before = source.entry(id).unwrap().custom_icon().unwrap().clone();
        let mut target = Database::new();
        let copied = copy(&source, id, &mut target);
        assert_icon(&source, id, &target, copied);
        assert_eq!(*source.entry(id).unwrap().custom_icon().unwrap(), before);
    }
}

#[test]
fn exchange_icon_metadata_keeps_reopened_shared_source_after_target_reopen() {
    let dir = TempDir::new().unwrap();
    let source_path = dir.path().join("synthetic-icon-metadata-source.kdbx");
    let target_path = dir.path().join("synthetic-icon-metadata-target.kdbx");
    let mut source = VaultSession::default();
    source
        .create(
            &source_path,
            "synthetic-master",
            "Aes",
            "Aes256",
            "None",
            None,
        )
        .unwrap();
    let first = seed(
        source.require_db_mut().unwrap(),
        Some("Synthetic icon.png"),
        Some(1_600_000_000),
    );
    let shared = source
        .require_db()
        .unwrap()
        .entry(first)
        .unwrap()
        .custom_icon()
        .unwrap()
        .id();
    let second = {
        let mut root = source.require_db_mut().unwrap().root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", "Synthetic shared icon metadata");
        entry.set_icon_custom(shared).unwrap();
        entry.as_ref().id()
    };
    source.mark_dirty();
    source.save().unwrap();
    source.close();
    source.open(&source_path, "synthetic-master", None).unwrap();
    assert_eq!(
        source
            .require_db()
            .unwrap()
            .entry(first)
            .unwrap()
            .custom_icon()
            .unwrap()
            .name
            .as_deref(),
        Some("Synthetic icon.png")
    );
    assert_eq!(
        source
            .require_db()
            .unwrap()
            .entry(first)
            .unwrap()
            .custom_icon()
            .unwrap()
            .last_modification_time,
        Some(time(1_600_000_000))
    );
    let bytes = Zeroizing::new(
        build_entries_document(source.require_db().unwrap(), &[first, second]).unwrap(),
    );
    let model: KeePassFile = quick_xml::de::from_reader(&bytes[..]).unwrap();
    assert_eq!(model.meta.custom_icons.unwrap().icons.len(), 1);
    let entries = parsed(&bytes).unwrap();
    let mut target = VaultSession::default();
    target
        .create(
            &target_path,
            "synthetic-master",
            "Aes",
            "Aes256",
            "None",
            None,
        )
        .unwrap();
    let copied: Vec<_> = entries
        .iter()
        .map(|entry| {
            EntryId::from_uuid(
                insert_entry(&mut target.require_db_mut().unwrap().root_mut(), entry).unwrap(),
            )
        })
        .collect();
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&target_path, "synthetic-master", None).unwrap();
    for (id, copy) in [first, second].into_iter().zip(copied) {
        assert_icon(
            source.require_db().unwrap(),
            id,
            target.require_db().unwrap(),
            copy,
        );
    }
}

fn document(timestamp: Option<&str>) -> Zeroizing<Vec<u8>> {
    let icon_id = encode_uuid(Uuid::from_u128(3));
    Zeroizing::new(format!(
        "<KeePassFile><Meta><CustomIcons><Icon><UUID>{icon_id}</UUID><Data>AAH/</Data>{}</Icon></CustomIcons></Meta><Root><Group><UUID>{}</UUID><Entry><UUID>{}</UUID><CustomIconUUID>{icon_id}</CustomIconUUID></Entry></Group></Root></KeePassFile>",
        timestamp.map(|s| format!("<LastModificationTime>{s}</LastModificationTime>")).unwrap_or_default(), encode_uuid(Uuid::from_u128(1)), encode_uuid(Uuid::from_u128(2))
    ).into_bytes())
}

#[test]
fn exchange_icon_metadata_rejects_out_of_range_time_before_insertion() {
    for stamp in [i64::MIN, i64::MAX] {
        let result = parsed(&document(Some(&stamp.to_string())));
        assert!(
            result.is_err(),
            "out-of-range icon time must refuse whole document"
        );
        assert_eq!(result.err().unwrap(), "自定义图标时间无效");
    }
    assert!(parsed(&document(Some("invalid-time"))).is_err());
}

#[test]
fn exchange_icon_metadata_legacy_missing_name_time_stays_absent() {
    let entries = parsed(&document(None)).unwrap();
    let mut target = Database::new();
    let id = EntryId::from_uuid(insert_entry(&mut target.root_mut(), &entries[0]).unwrap());
    let entry = target.entry(id).unwrap();
    let icon = entry.custom_icon().unwrap();
    assert_eq!(icon.name, None);
    assert_eq!(icon.last_modification_time, None);
    assert_eq!(icon.data, vec![0, 1, 255]);
}

#[test]
fn exchange_icon_metadata_shared_names_charge_full_utf8_copy_budget() {
    for (name, data, per_copy) in [
        ("ABCD", "AAH/", 7),
        ("é", "AAH/", 5),
        ("", "AAH/", 3),
        ("ABCD", "", 4),
    ] {
        let text = String::from_utf8(document(None).to_vec()).unwrap();
        let text = text.replace(
            "<Data>AAH/</Data>",
            &format!("<Data>{data}</Data><Name>{name}</Name>"),
        );
        let text = text.replace(
            "</Group>",
            &format!(
                "<Entry><UUID>{}</UUID><CustomIconUUID>{}</CustomIconUUID></Entry></Group>",
                encode_uuid(Uuid::from_u128(4)),
                encode_uuid(Uuid::from_u128(3))
            ),
        );
        let bytes = Zeroizing::new(text.into_bytes());
        let entries = parse_entries_document_limited(&bytes, per_copy * 2).unwrap();
        assert_eq!(entries.len(), 2);
        for entry in &entries {
            assert_eq!(
                entry.custom_icon.as_ref().unwrap().name.as_deref(),
                Some(name)
            );
        }
        if !name.is_empty() {
            assert_ne!(
                entries[0]
                    .custom_icon
                    .as_ref()
                    .unwrap()
                    .name
                    .as_ref()
                    .unwrap()
                    .as_ptr(),
                entries[1]
                    .custom_icon
                    .as_ref()
                    .unwrap()
                    .name
                    .as_ref()
                    .unwrap()
                    .as_ptr(),
                "nonempty shared names own separate bytes"
            );
        }
        let result = parse_entries_document_limited(&bytes, per_copy * 2 - 1);
        assert!(
            result.is_err(),
            "name plus image copies must respect the same budget"
        );
        assert_eq!(result.err().unwrap(), "剪贴板数据过大");
    }
}
