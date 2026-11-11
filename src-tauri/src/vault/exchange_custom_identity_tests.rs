use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn item(key: &str, value: Option<&str>) -> XmlCustomDataItem {
    XmlCustomDataItem {
        key: key.to_owned(),
        value: value.map(str::to_owned),
        value_type: None,
        last_modification_time: None,
    }
}

fn model() -> KeePassFile {
    let mut database = Database::new();
    let mut ids = Vec::new();
    for index in 0..2 {
        let mut root = database.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", format!("Synthetic custom entry {index}"));
        entry.custom_data.insert(
            "Plugin".to_owned(),
            CustomDataItem {
                value: Some(CustomDataValue::String("Synthetic original".to_owned())),
                last_modification_time: None,
            },
        );
        ids.push(entry.as_ref().id());
    }
    let bytes = Zeroizing::new(build_entries_document(&database, &ids).unwrap());
    quick_xml::de::from_reader(bytes.as_slice()).unwrap()
}

fn document(file: &KeePassFile) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(
        to_string_with_root("KeePassFile", file)
            .unwrap()
            .replace('\r', "&#13;")
            .into_bytes(),
    )
}

fn parse(document: &[u8]) -> Result<Vec<PastedEntry>, String> {
    let frame = Zeroizing::new(encode_payload(document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&frame).unwrap());
    assert!(*decoded == document, "framed custom data bytes");
    parse_entries_document(&decoded)
}

fn duplicates() -> Vec<Zeroizing<Vec<u8>>> {
    let mut documents = Vec::new();
    for (first, last) in [
        (Some("Synthetic original"), Some("Synthetic duplicate")),
        (Some("Synthetic original"), Some("Synthetic original")),
        (Some(""), Some("Synthetic duplicate")),
        (None, Some("Synthetic duplicate")),
        (Some("Synthetic original"), None),
        (None, None),
    ] {
        let mut file = model();
        file.root.group.entries[1].custom_data = Some(XmlCustomData {
            items: vec![item("Plugin", first), item("Plugin", last)],
        });
        documents.push(document(&file));
    }
    let mut file = model();
    file.root.group.entries[1].custom_data = Some(XmlCustomData {
        items: vec![
            item(" 合 ", Some("Synthetic first")),
            item(" 合 ", Some("Synthetic last")),
        ],
    });
    documents.push(document(&file));
    let mut file = model();
    file.root.group.entries[1].custom_data = Some(XmlCustomData {
        items: vec![
            item("Plugin", Some("Synthetic original")),
            item("Plugin", Some("Synthetic duplicate")),
        ],
    });
    let text = String::from_utf8(document(&file).to_vec()).unwrap();
    let offset = text.rfind("<Key>Plugin</Key>").unwrap();
    let mut alias = text;
    alias.replace_range(
        offset..offset + "<Key>Plugin</Key>".len(),
        "<Key>P&#108;ugin</Key>",
    );
    documents.push(Zeroizing::new(alias.into_bytes()));
    file.root.group.entries[1]
        .custom_data
        .as_mut()
        .unwrap()
        .items
        .reverse();
    documents.push(document(&file));
    documents
}

fn saved(file: &KeePassFile) {
    let entries = parse(&document(file)).unwrap();
    assert_eq!(entries.len(), 2);
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-custom-identity.kdbx");
    let mut target = VaultSession::default();
    target
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let ids: Vec<_> = entries
        .iter()
        .map(|entry| insert_entry(&mut target.require_db_mut().unwrap().root_mut(), entry).unwrap())
        .collect();
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&path, "synthetic-master", None).unwrap();
    for (id, expected) in ids.into_iter().zip(&entries) {
        let entry = target
            .require_db()
            .unwrap()
            .entry(EntryId::from_uuid(id))
            .unwrap();
        assert_eq!(entry.custom_data.len(), expected.custom_data.len());
        for item in &expected.custom_data {
            assert!(
                entry.custom_data[&item.key].value == item.value,
                "saved custom value"
            );
        }
    }
}

#[test]
fn exchange_custom_identity_rejects_duplicate_metadata_keys() {
    let mut accepted = Vec::new();
    for (index, document) in duplicates().into_iter().enumerate() {
        match parse(&document) {
            Ok(entries) => accepted.push((index, entries.len())),
            Err(error) => assert!(error == "条目自定义数据重复", "custom error contract"),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted custom duplicate identities: {accepted:?}"
    );
}

#[test]
fn exchange_custom_identity_keeps_distinct_names_and_entry_scopes() {
    saved(&model());
    let mut file = model();
    for entry in &mut file.root.group.entries {
        entry.custom_data.as_mut().unwrap().items.extend([
            item("plugin", Some("Synthetic case")),
            item("Plugin ", Some("Synthetic trailing")),
            item(" Plugin", Some("Synthetic leading")),
            item("é", Some("Synthetic NFC")),
            item("e\u{301}", Some("Synthetic NFD")),
            item("P&#108;ugin", Some("Synthetic literal entity")),
        ]);
    }
    saved(&file);
}

#[test]
fn exchange_custom_identity_keeps_ordinary_field_namespace_separate() {
    let mut file = model();
    for entry in &mut file.root.group.entries {
        entry
            .custom_data
            .as_mut()
            .unwrap()
            .items
            .push(item("Title", Some("Synthetic independent metadata")));
    }
    saved(&file);
}
