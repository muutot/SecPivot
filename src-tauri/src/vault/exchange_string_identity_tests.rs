use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn source() -> Zeroizing<Vec<u8>> {
    let mut database = Database::new();
    let mut ids = Vec::new();
    for index in 0..2 {
        let id = {
            let mut root = database.root_mut();
            let mut entry = root.add_entry();
            entry.set_unprotected("Title", format!("Synthetic entry {index}"));
            entry.set_protected("Custom", "Synthetic original");
            entry.set_unprotected(" 合 ", "Synthetic whitespace name");
            entry.add_attachment("empty.bin", Value::protected(Vec::new()));
            entry.add_attachment("small.bin", Value::unprotected(vec![0, 1, 255]));
            entry.as_ref().id()
        };
        ids.push(id);
    }
    Zeroizing::new(build_entries_document(&database, &ids).unwrap())
}

fn model() -> KeePassFile {
    quick_xml::de::from_reader(source().as_slice()).unwrap()
}

fn document(file: &KeePassFile) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(
        to_string_with_root("KeePassFile", file)
            .unwrap()
            .replace('\r', "&#13;")
            .into_bytes(),
    )
}

fn field(key: &str, value: &str, protected: bool) -> XmlStringField {
    XmlStringField {
        key: key.to_owned(),
        value: XmlStringValue {
            value: value.to_owned(),
            protected: bool_xml(protected),
        },
    }
}

fn duplicate_documents() -> Vec<Zeroizing<Vec<u8>>> {
    let mut documents = Vec::new();
    for key in ["Title", "Custom", " 合 "] {
        for first_protected in [false, true] {
            for last_protected in [false, true] {
                let mut file = model();
                let entry = &mut file.root.group.entries[1];
                let original = entry
                    .string_fields
                    .iter_mut()
                    .find(|field| field.key == key)
                    .unwrap();
                *original = field(key, "Synthetic original", first_protected);
                entry
                    .string_fields
                    .push(field(key, "Synthetic duplicate", last_protected));
                documents.push(document(&file));
            }
        }
    }
    for first_protected in [false, true] {
        for last_protected in [false, true] {
            let mut file = model();
            let entry = &mut file.root.group.entries[1];
            *entry
                .string_fields
                .iter_mut()
                .find(|field| field.key == "Custom")
                .unwrap() = field("Custom", "Synthetic original", first_protected);
            entry
                .string_fields
                .push(field("Custom", "Synthetic original", last_protected));
            documents.push(document(&file));
        }
    }
    let mut file = model();
    file.root.group.entries[1]
        .string_fields
        .push(field("Custom", "Synthetic duplicate", false));
    let text = String::from_utf8(document(&file).to_vec()).unwrap();
    let start = text.rfind("<Key>Custom</Key>").unwrap();
    let mut alias = text.clone();
    alias.replace_range(
        start..start + "<Key>Custom</Key>".len(),
        "<Key>C&#117;stom</Key>",
    );
    documents.push(Zeroizing::new(alias.into_bytes()));
    let entry = &mut file.root.group.entries[1];
    let first = entry
        .string_fields
        .iter()
        .position(|field| field.key == "Custom")
        .unwrap();
    let last = entry.string_fields.len() - 1;
    entry.string_fields.swap(first, last);
    documents.push(document(&file));
    documents
}

fn distinct_model() -> KeePassFile {
    let mut file = model();
    for entry in &mut file.root.group.entries {
        for (index, key) in [
            "custom",
            "Custom ",
            " Custom",
            "é",
            "e\u{301}",
            "Cust&#111;m",
        ]
        .into_iter()
        .enumerate()
        {
            entry.string_fields.push(field(
                key,
                &format!("Synthetic distinct {index}"),
                index % 2 == 0,
            ));
        }
    }
    file
}

fn repeated_binary_model() -> KeePassFile {
    let mut file = model();
    let entry = &mut file.root.group.entries[0];
    let key = entry
        .string_fields
        .iter()
        .find(|field| {
            field.key.starts_with(BINARY_FIELD_PREFIX) && field.value.value == "empty.bin"
        })
        .unwrap()
        .key
        .clone();
    entry.string_fields.push(field(&key, "repeated.bin", false));
    file
}

fn framed_parse(document: &[u8]) -> Result<Vec<PastedEntry>, String> {
    let payload = Zeroizing::new(encode_payload(document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&payload).unwrap());
    assert!(*decoded == document, "framing bytes");
    parse_entries_document(&decoded)
}

fn assert_saved_model(file: &KeePassFile) {
    let parsed = framed_parse(&document(file)).unwrap();
    assert_eq!(parsed.len(), 2);
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-exchange-string-identity.kdbx");
    let mut target = VaultSession::default();
    target
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let ids: Vec<_> = parsed
        .iter()
        .map(|entry| insert_entry(&mut target.require_db_mut().unwrap().root_mut(), entry).unwrap())
        .collect();
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&path, "synthetic-master", None).unwrap();
    for ((id, expected), parsed) in ids.iter().zip(&file.root.group.entries).zip(&parsed) {
        let entry = target
            .require_db()
            .unwrap()
            .entry(EntryId::from_uuid(*id))
            .unwrap();
        let ordinary: Vec<_> = expected
            .string_fields
            .iter()
            .filter(|field| !field.key.starts_with(BINARY_FIELD_PREFIX))
            .collect();
        assert_eq!(entry.fields.len(), ordinary.len());
        for field in ordinary {
            assert!(
                entry.get(&field.key) == Some(field.value.value.as_str()),
                "saved field value"
            );
            assert!(
                entry.fields.get(&field.key).unwrap().is_protected()
                    == (field.value.protected == "True"),
                "saved field protection"
            );
        }
        assert_eq!(entry.attachments_named().count(), parsed.attachments.len());
        for (name, bytes, protected) in &parsed.attachments {
            assert!(
                entry
                    .attachments_named()
                    .any(|(saved_name, binary)| saved_name == name.as_str()
                        && binary.data.get() == bytes
                        && binary.data.is_protected() == *protected),
                "saved attachment identity"
            );
        }
    }
}

#[test]
fn exchange_string_identity_rejects_duplicate_ordinary_fields() {
    let mut accepted = Vec::new();
    for (index, document) in duplicate_documents().into_iter().enumerate() {
        match framed_parse(&document) {
            Ok(entries) => accepted.push((index, entries.len())),
            Err(error) => assert!(error == "条目字段重复", "field {index}: error contract"),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted ordinary duplicate fields: {accepted:?}"
    );
}

#[test]
fn exchange_string_identity_keeps_distinct_names_and_entry_scopes() {
    assert_saved_model(&model());
    assert_saved_model(&distinct_model());
}

#[test]
fn exchange_string_identity_keeps_repeated_binary_references() {
    let file = repeated_binary_model();
    let entries = framed_parse(&document(&file)).unwrap();
    assert_eq!(entries[0].attachments.len(), 3);
    assert_eq!(entries[1].attachments.len(), 2);
    assert!(entries[0]
        .attachments
        .iter()
        .any(|(name, bytes, protected)| name == "repeated.bin" && bytes.is_empty() && !protected));
    assert_saved_model(&file);
}
