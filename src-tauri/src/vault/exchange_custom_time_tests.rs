use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn time(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(seconds, 0)
        .unwrap()
        .naive_utc()
}

fn seed(database: &mut Database) -> EntryId {
    let mut root = database.root_mut();
    let mut entry = root.add_entry();
    entry.set_unprotected("Title", "Synthetic custom data times");
    for (key, value, seconds) in [
        (
            "Text",
            Some(CustomDataValue::String("Synthetic text".into())),
            Some(1_600_000_000),
        ),
        (
            "Binary",
            Some(CustomDataValue::Binary(vec![0, 1, 255])),
            Some(-1),
        ),
        ("AbsentValue", None, Some(0)),
        (
            "AbsentTime",
            Some(CustomDataValue::String(String::new())),
            None,
        ),
    ] {
        entry.custom_data.insert(
            key.to_owned(),
            CustomDataItem {
                value,
                last_modification_time: seconds.map(time),
            },
        );
    }
    entry.as_ref().id()
}

fn parsed(document: &[u8]) -> Result<Vec<PastedEntry>, String> {
    let frame = Zeroizing::new(encode_payload(document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&frame).unwrap());
    assert!(*decoded == document, "complete custom-time framing");
    parse_entries_document(&decoded)
}

fn copy(source: &Database, id: EntryId, target: &mut Database) -> EntryId {
    let document = Zeroizing::new(build_entries_document(source, &[id]).unwrap());
    let entries = parsed(&document).unwrap();
    assert_eq!(entries.len(), 1);
    EntryId::from_uuid(insert_entry(&mut target.root_mut(), &entries[0]).unwrap())
}

fn assert_metadata(source: &Database, id: EntryId, target: &Database, copied: EntryId) {
    let expected = source.entry(id).unwrap();
    let actual = target.entry(copied).unwrap();
    assert_ne!(id, copied, "paste keeps fresh entry identity");
    assert_eq!(actual.get_title(), expected.get_title());
    assert_eq!(actual.custom_data.len(), expected.custom_data.len());
    for (key, item) in &expected.custom_data {
        let copied = actual.custom_data.get(key).unwrap();
        assert!(
            copied.value == item.value,
            "native metadata value/kind retained"
        );
        assert_eq!(
            copied.last_modification_time, item.last_modification_time,
            "native metadata timestamp retained"
        );
    }
}

#[test]
fn exchange_custom_times_keep_present_and_absent_native_times() {
    let mut source = Database::new();
    let id = seed(&mut source);
    let before = source.entry(id).unwrap().custom_data.clone();
    let mut target = Database::new();
    let copied = copy(&source, id, &mut target);
    assert_metadata(&source, id, &target, copied);
    assert_eq!(source.entry(id).unwrap().custom_data, before);
}

#[test]
fn exchange_custom_times_keep_reopened_source_times_after_target_reopen() {
    let dir = TempDir::new().unwrap();
    let source_path = dir.path().join("synthetic-time-source.kdbx");
    let target_path = dir.path().join("synthetic-time-target.kdbx");
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
    let id = seed(source.require_db_mut().unwrap());
    source.mark_dirty();
    source.save().unwrap();
    source.close();
    source.open(&source_path, "synthetic-master", None).unwrap();
    assert_eq!(
        source.require_db().unwrap().entry(id).unwrap().custom_data["Text"].last_modification_time,
        Some(time(1_600_000_000))
    );
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
    let copied = copy(
        source.require_db().unwrap(),
        id,
        target.require_db_mut().unwrap(),
    );
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&target_path, "synthetic-master", None).unwrap();
    assert_metadata(
        source.require_db().unwrap(),
        id,
        target.require_db().unwrap(),
        copied,
    );
}

fn document(timestamp: Option<&str>) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(format!(
        "<KeePassFile><Root><Group><UUID>{}</UUID><Entry><UUID>{}</UUID><CustomData><Item><Key>SyntheticTime</Key><Value>Synthetic text</Value>{}</Item></CustomData></Entry></Group></Root></KeePassFile>",
        encode_uuid(Uuid::from_u128(1)), encode_uuid(Uuid::from_u128(2)),
        timestamp.map(|s| format!("<LastModificationTime>{s}</LastModificationTime>")).unwrap_or_default()
    ).into_bytes())
}

#[test]
fn exchange_custom_times_reject_out_of_range_before_returning_entries() {
    for stamp in [i64::MIN, i64::MAX] {
        let result = parsed(&document(Some(&stamp.to_string())));
        assert!(
            result.is_err(),
            "out-of-range timestamp must refuse the whole list"
        );
        assert_eq!(result.err().unwrap(), "自定义数据时间无效");
    }
    assert!(parsed(&document(Some("invalid-time"))).is_err());
}

#[test]
fn exchange_custom_times_legacy_missing_time_remains_absent() {
    let entries = parsed(&document(None)).unwrap();
    let mut target = Database::new();
    let id = EntryId::from_uuid(insert_entry(&mut target.root_mut(), &entries[0]).unwrap());
    assert_eq!(
        target.entry(id).unwrap().custom_data["SyntheticTime"].last_modification_time,
        None
    );
}
