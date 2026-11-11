use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn document() -> (Database, EntryId, Zeroizing<Vec<u8>>) {
    let mut database = Database::new();
    let id = {
        let mut root = database.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", "Synthetic envelope\r\ncontrol");
        entry.set_unprotected("Empty", "");
        entry.add_attachment("empty.bin", Value::protected(Vec::new()));
        entry.add_attachment("small.bin", Value::unprotected(vec![0, 1, 255]));
        entry.as_ref().id()
    };
    let bytes = Zeroizing::new(build_entries_document(&database, &[id]).unwrap());
    (database, id, bytes)
}

fn framed_parse(document: &[u8]) -> Result<Vec<PastedEntry>, String> {
    let framed = Zeroizing::new(encode_payload(document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&framed).unwrap());
    assert_eq!(&*decoded, document);
    parse_entries_document(&decoded)
}

#[test]
fn exchange_document_rejects_unconsumed_suffixes() {
    let (_, _, original) = document();
    let mut accepted = Vec::new();
    for (index, suffix) in [
        b"!".as_slice(),
        original.as_slice(),
        b"<Other/>",
        b"<",
        b"<!--",
        b"<?unfinished",
        b"&#32;",
        b"<![CDATA[ ]]>",
        b"\xff",
        b"\xc2\xa0",
        b"<?xml version=\"1.0\"?>",
        b"<!DOCTYPE KeePassFile>",
        b"</KeePassFile>",
    ]
    .into_iter()
    .enumerate()
    {
        let mut bytes = Zeroizing::new(original.to_vec());
        bytes.extend_from_slice(suffix);
        match framed_parse(&bytes) {
            Ok(entries) => accepted.push((index, entries.len())),
            Err(error) => assert!(
                error == "剪贴板文档格式无效",
                "suffix {index}: error contract"
            ),
        }
    }
    assert!(accepted.is_empty(), "accepted suffix cases: {accepted:?}");
}

#[test]
fn exchange_document_requires_one_complete_named_root() {
    let (_, _, original) = document();
    let text = std::str::from_utf8(&original).unwrap();
    let mut accepted = Vec::new();
    let variants = [
        text.replace("KeePassFile", "OtherFormat"),
        text.trim_end_matches("</KeePassFile>").to_owned(),
        format!("outside{text}"),
        format!("<![CDATA[ ]]>{text}"),
    ];
    for (index, text) in variants.into_iter().enumerate() {
        match framed_parse(text.as_bytes()) {
            Ok(entries) => accepted.push((index, entries.len())),
            Err(error) => assert!(
                error == "剪贴板文档格式无效",
                "root {index}: error contract"
            ),
        }
    }
    assert!(accepted.is_empty(), "accepted root cases: {accepted:?}");
}

#[test]
fn exchange_document_allows_legal_surrounding_xml_misc() {
    let (_, _, original) = document();
    for (prefix, suffix) in [
        ("", ""),
        (" \r\n\t", "\n\t "),
        ("<!--before-->", "<!--after-->"),
        ("<?before synthetic?>", "<?after synthetic?>"),
        ("<!DOCTYPE KeePassFile>", ""),
        (
            "<?xml version=\"1.0\"?><!--before-->\n",
            "\n<!--after--><?after synthetic?>",
        ),
    ] {
        let mut bytes = Zeroizing::new(prefix.as_bytes().to_vec());
        bytes.extend_from_slice(&original);
        bytes.extend_from_slice(suffix.as_bytes());
        let entries = framed_parse(&bytes).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0]
            .fields
            .iter()
            .any(|(key, value, _)| key == "Title" && value == "Synthetic envelope\r\ncontrol"));
        assert_eq!(entries[0].attachments.len(), 2);
    }
}

#[test]
fn exchange_document_complete_input_keeps_native_saved_values() {
    let (_, source_id, original) = document();
    let entries = framed_parse(&original).unwrap();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-document-envelope.kdbx");
    let mut target = VaultSession::default();
    target
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let uuid = insert_entry(
        &mut target.require_db_mut().unwrap().root_mut(),
        &entries[0],
    )
    .unwrap();
    assert_ne!(uuid, source_id.uuid());
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&path, "synthetic-master", None).unwrap();
    let entry = target
        .require_db()
        .unwrap()
        .entry(EntryId::from_uuid(uuid))
        .unwrap();
    assert_eq!(entry.get_title(), Some("Synthetic envelope\r\ncontrol"));
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
