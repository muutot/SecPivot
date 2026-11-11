use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn fixtures() -> Vec<(&'static str, Option<CustomDataValue>)> {
    vec![
        (
            "Text",
            Some(CustomDataValue::String("Synthetic text".into())),
        ),
        ("EmptyText", Some(CustomDataValue::String(String::new()))),
        ("AsciiBlank", Some(CustomDataValue::String(" \t\n".into()))),
        (
            "UnicodeBlank",
            Some(CustomDataValue::String("\u{a0}\u{3000}".into())),
        ),
        (
            "Carriage",
            Some(CustomDataValue::String("Synthetic\r\nline\r".into())),
        ),
        (
            "Base64Text",
            Some(CustomDataValue::String("bWV0YQ==".into())),
        ),
        (
            "LiteralEntity",
            Some(CustomDataValue::String("Synthetic &#13;".into())),
        ),
        ("EmptyBinary", Some(CustomDataValue::Binary(Vec::new()))),
        (
            "SmallBinary",
            Some(CustomDataValue::Binary(vec![0, 1, 255])),
        ),
        (
            "TextBinary",
            Some(CustomDataValue::Binary(b"meta".to_vec())),
        ),
        ("Absent", None),
    ]
}

fn seed(database: &mut Database, rows: &[(&str, Option<CustomDataValue>)]) -> EntryId {
    let mut root = database.root_mut();
    let mut entry = root.add_entry();
    entry.set_unprotected("Title", "Synthetic metadata values");
    for (key, value) in rows {
        entry.custom_data.insert(
            (*key).to_owned(),
            CustomDataItem {
                value: value.clone(),
                last_modification_time: None,
            },
        );
    }
    entry.as_ref().id()
}

fn parse(document: &[u8]) -> Vec<PastedEntry> {
    let frame = Zeroizing::new(encode_payload(document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&frame).unwrap());
    assert!(*decoded == document, "custom value framing bytes");
    parse_entries_document(&decoded).unwrap()
}

fn transfer(database: &Database, id: EntryId, target: &mut Database) -> EntryId {
    let bytes = Zeroizing::new(build_entries_document(database, &[id]).unwrap());
    let parsed = parse(&bytes);
    assert_eq!(parsed.len(), 1);
    EntryId::from_uuid(insert_entry(&mut target.root_mut(), &parsed[0]).unwrap())
}

fn assert_values(source: &Database, source_id: EntryId, target: &Database, target_id: EntryId) {
    let expected = source.entry(source_id).unwrap();
    let actual = target.entry(target_id).unwrap();
    assert_eq!(actual.custom_data.len(), expected.custom_data.len());
    for (key, item) in &expected.custom_data {
        assert!(
            actual.custom_data.get(key).map(|item| &item.value) == Some(&item.value),
            "custom metadata value/kind"
        );
    }
}

#[test]
fn exchange_custom_values_keep_all_native_value_kinds_in_memory() {
    let mut source = Database::new();
    let id = seed(&mut source, &fixtures());
    let mut target = Database::new();
    let copied = transfer(&source, id, &mut target);
    assert_values(&source, id, &target, copied);
}

#[test]
fn exchange_custom_values_keep_reopened_source_metadata_after_target_reopen() {
    let dir = TempDir::new().unwrap();
    let source_path = dir.path().join("synthetic-metadata-source.kdbx");
    let target_path = dir.path().join("synthetic-metadata-target.kdbx");
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
    let rows: Vec<_> = fixtures()
        .into_iter()
        .filter(|(key, _)| !matches!(*key, "AsciiBlank" | "UnicodeBlank"))
        .collect();
    let id = seed(source.require_db_mut().unwrap(), &rows);
    source.mark_dirty();
    source.save().unwrap();
    source.close();
    source.open(&source_path, "synthetic-master", None).unwrap();
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
    let copied = transfer(
        source.require_db().unwrap(),
        id,
        target.require_db_mut().unwrap(),
    );
    assert_values(
        source.require_db().unwrap(),
        id,
        target.require_db().unwrap(),
        copied,
    );
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&target_path, "synthetic-master", None).unwrap();
    assert_values(
        source.require_db().unwrap(),
        id,
        target.require_db().unwrap(),
        copied,
    );
}

#[test]
fn exchange_custom_values_keep_legacy_plain_text_without_type_guessing() {
    let rows: Vec<_> = fixtures()
        .into_iter()
        .filter(|(_, value)| matches!(value, Some(CustomDataValue::String(_))))
        .collect();
    let mut source = Database::new();
    let id = seed(&mut source, &rows);
    let mut target = Database::new();
    let copied = transfer(&source, id, &mut target);
    assert_values(&source, id, &target, copied);
}

fn marked_document(kind: &str, value: Option<&str>, attachment: bool) -> Zeroizing<Vec<u8>> {
    let mut database = Database::new();
    let id = seed(
        &mut database,
        &[(
            "TypedBinary",
            Some(CustomDataValue::String("Synthetic placeholder".into())),
        )],
    );
    if attachment {
        database
            .entry_mut(id)
            .unwrap()
            .add_attachment("small.bin", Value::unprotected(vec![0, 1, 255]));
    }
    let text = String::from_utf8(build_entries_document(&database, &[id]).unwrap()).unwrap();
    let original = "<Item><Key>TypedBinary</Key><Value>Synthetic placeholder</Value></Item>";
    assert_eq!(text.matches(original).count(), 1);
    let replacement = format!(
        "<Item SecPivotType=\"{kind}\"><Key>TypedBinary</Key>{}</Item>",
        value
            .map(|value| format!("<Value>{value}</Value>"))
            .unwrap_or_default()
    );
    Zeroizing::new(text.replace(original, &replacement).into_bytes())
}

#[test]
fn exchange_custom_values_reject_invalid_types_and_share_binary_budget() {
    for (kind, value) in [
        ("Unknown", Some("AAH/")),
        ("binary", Some("AAH/")),
        ("Binary", None),
        ("Binary", Some("invalid!")),
        ("None", Some("")),
        ("None", Some("Synthetic value")),
    ] {
        assert!(
            parse_entries_document(&marked_document(kind, value, false)).is_err(),
            "invalid typed metadata must refuse whole document"
        );
    }
    for (encoded, exact) in [("AQ==", 1), ("AQI=", 2), ("AQIDBA==", 4)] {
        let padded = marked_document("Binary", Some(encoded), false);
        assert!(
            parse_entries_document_limited(&padded, exact).is_ok(),
            "padded exact metadata budget"
        );
        assert!(
            parse_entries_document_limited(&padded, exact - 1).is_err(),
            "padded overflow metadata budget"
        );
    }
    let document = marked_document("Binary", Some("BQb/"), false);
    assert!(
        parse_entries_document_limited(&document, 2).is_err(),
        "metadata byte budget"
    );
    assert!(
        parse_entries_document_limited(&document, 3).is_ok(),
        "exact metadata budget"
    );
    let combined = marked_document("Binary", Some("BQb/"), true);
    assert!(
        parse_entries_document_limited(&combined, 5).is_err(),
        "combined metadata/attachment budget"
    );
    assert!(
        parse_entries_document_limited(&combined, 6).is_ok(),
        "exact combined budget"
    );
}
