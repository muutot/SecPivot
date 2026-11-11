use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn database() -> (Database, EntryId) {
    let mut db = Database::new();
    let id = {
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", "Synthetic empty attachments");
        entry.set_protected("Password", "synthetic-password");
        entry.add_attachment("zero-protected.bin", Value::protected(Vec::new()));
        entry.add_attachment("zero-visible.bin", Value::unprotected(Vec::new()));
        entry.add_attachment("nonempty.bin", Value::protected(vec![0, 1, 2, 255]));
        entry.as_ref().id()
    };
    (db, id)
}

fn framed_document(db: &Database, id: EntryId) -> Zeroizing<Vec<u8>> {
    let document = Zeroizing::new(build_entries_document(db, &[id]).unwrap());
    let framed = Zeroizing::new(encode_payload(&document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&framed).unwrap());
    assert_eq!(&*document, &*decoded);
    decoded
}

#[test]
fn exchange_empty_binaries_survive_actual_document_framing_and_parse() {
    let (db, id) = database();
    let document = framed_document(&db, id);
    let parsed = parse_entries_document(&document);
    assert!(
        parsed.is_ok(),
        "internally generated empty binaries rejected"
    );
    let parsed = parsed.unwrap().remove(0);
    assert_eq!(parsed.attachments.len(), 3);
    for (name, data, protected) in [
        ("zero-protected.bin", Vec::new(), true),
        ("zero-visible.bin", Vec::new(), false),
        ("nonempty.bin", vec![0, 1, 2, 255], true),
    ] {
        assert!(parsed
            .attachments
            .iter()
            .any(|(actual, bytes, flag)| actual == name && bytes == &data && *flag == protected));
    }
}

fn document(binary: &str) -> String {
    format!("<KeePassFile><Root><Group><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><Entry><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><String><Key>BinDes-0</Key><Value Protected=\"True\">empty.bin</Value></String></Entry></Group></Root><Binaries>{binary}</Binaries></KeePassFile>")
}

#[test]
fn exchange_empty_binaries_accept_equivalent_empty_xml_forms() {
    for binary in [
        "<Binary ID=\"0\"/>",
        "<Binary ID=\"0\"></Binary>",
        "<Binary ID=\"0\"> \r\n\t </Binary>",
    ] {
        let parsed = parse_entries_document(document(binary).as_bytes());
        assert!(parsed.is_ok(), "valid empty Base64 form rejected");
        assert_eq!(
            parsed.unwrap()[0].attachments,
            vec![("empty.bin".to_owned(), Vec::new(), true)]
        );
    }
    let parsed =
        parse_entries_document(document("<Binary ID=\"0\"> AAEC/w== </Binary>").as_bytes())
            .unwrap();
    assert_eq!(
        parsed[0].attachments,
        vec![("empty.bin".to_owned(), vec![0, 1, 2, 255], true)]
    );
}

#[test]
fn exchange_empty_binaries_preserve_empty_and_nonempty_attachments_after_native_reopen() {
    let (db, id) = database();
    let document = framed_document(&db, id);
    let parsed = parse_entries_document(&document);
    assert!(
        parsed.is_ok(),
        "cannot paste internally generated empty attachment"
    );
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-empty-exchange.kdbx");
    let mut target = VaultSession::default();
    target
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let new_id = insert_entry(
        &mut target.require_db_mut().unwrap().root_mut(),
        &parsed.unwrap()[0],
    )
    .unwrap();
    assert_ne!(id.uuid(), new_id);
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&path, "synthetic-master", None).unwrap();
    let entry = target
        .require_db()
        .unwrap()
        .entry(EntryId::from_uuid(new_id))
        .unwrap();
    let attachments: Vec<_> = entry
        .attachments_named()
        .map(|(name, data)| {
            (
                name.to_owned(),
                data.data.get().to_vec(),
                data.data.is_protected(),
            )
        })
        .collect();
    assert_eq!(attachments.len(), 3);
    for expected in [
        ("zero-protected.bin", Vec::new(), true),
        ("zero-visible.bin", Vec::new(), false),
        ("nonempty.bin", vec![0, 1, 2, 255], true),
    ] {
        assert!(attachments
            .iter()
            .any(|(name, data, protected)| name == expected.0
                && data == &expected.1
                && *protected == expected.2));
    }
    let output = framed_document(target.require_db().unwrap(), EntryId::from_uuid(new_id));
    assert_eq!(
        parse_entries_document(&output).unwrap()[0]
            .attachments
            .len(),
        3
    );
}

#[test]
fn exchange_empty_binary_controls_reject_corrupt_text_structure_and_references() {
    for binary in [
        "<Binary ID=\"0\">%%%synthetic-invalid%%%</Binary>",
        "<Binary/>",
        "<Binary ID=\"0\"><Data>AAEC</Data></Binary>",
        "<Binary ID=\"0\" Compressed=\"True\">AAEC</Binary>",
    ] {
        assert!(parse_entries_document(document(binary).as_bytes()).is_err());
    }
    let missing = document("");
    assert!(parse_entries_document(missing.as_bytes()).is_err());
}
