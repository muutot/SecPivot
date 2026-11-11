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

#[derive(Deserialize)]
struct BinaryIdentityFixture {
    duplicates: Vec<String>,
    unique: Vec<String>,
}

fn binary_identity_fixture() -> BinaryIdentityFixture {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/exchange-binary-identity.json"
    ))
    .unwrap()
}

#[test]
fn exchange_duplicate_binary_ids_reject_the_complete_document() {
    let fixture = binary_identity_fixture();
    assert_eq!(fixture.duplicates.len(), 6);
    let mut accepted = Vec::new();
    for (index, binaries) in fixture.duplicates.iter().enumerate() {
        let xml = Zeroizing::new(document(binaries));
        let framed = Zeroizing::new(encode_payload(xml.as_bytes(), false).unwrap());
        let decoded = Zeroizing::new(decode_payload(&framed).unwrap());
        match parse_entries_document(&decoded) {
            Ok(_) => accepted.push(index),
            Err(error) => assert_eq!(error, "附件编号重复"),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted duplicate-ID cases: {accepted:?}"
    );
}

#[test]
fn exchange_unique_binary_ids_keep_sparse_order_and_shared_references_after_reopen() {
    let fixture = binary_identity_fixture();
    assert_eq!(fixture.unique.len(), 3);
    let entry = "<Entry><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><String><Key>BinDes-0</Key><Value Protected=\"True\">nonempty.bin</Value></String><String><Key>BinDes-7</Key><Value>empty.bin</Value></String></Entry>";
    for (index, binaries) in fixture.unique.iter().enumerate() {
        let xml = Zeroizing::new(format!("<KeePassFile><Root><Group><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID>{entry}{entry}</Group></Root><Binaries>{binaries}</Binaries></KeePassFile>"));
        let framed = Zeroizing::new(encode_payload(xml.as_bytes(), false).unwrap());
        let decoded = Zeroizing::new(decode_payload(&framed).unwrap());
        let parsed = parse_entries_document(&decoded).unwrap();
        assert_eq!(parsed.len(), 2);
        let dir = TempDir::new().unwrap();
        let path = dir
            .path()
            .join(format!("synthetic-binary-identity-{index}.kdbx"));
        let mut target = VaultSession::default();
        target
            .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
            .unwrap();
        let ids: Vec<_> = parsed
            .iter()
            .map(|entry| {
                insert_entry(&mut target.require_db_mut().unwrap().root_mut(), entry).unwrap()
            })
            .collect();
        assert_ne!(ids[0], ids[1]);
        target.mark_dirty();
        target.save().unwrap();
        target.close();
        target.open(&path, "synthetic-master", None).unwrap();
        for id in &ids {
            let entry = target
                .require_db()
                .unwrap()
                .entry(EntryId::from_uuid(*id))
                .unwrap();
            assert_eq!(entry.attachments_named().count(), 2);
            for (name, bytes, protected) in [
                ("nonempty.bin", vec![0, 1, 2, 255], true),
                ("empty.bin", Vec::new(), false),
            ] {
                assert!(entry
                    .attachments_named()
                    .any(|(actual, attachment)| actual == name
                        && attachment.data.get() == &bytes
                        && attachment.data.is_protected() == protected));
            }
        }
        let document = Zeroizing::new(
            build_entries_document(
                target.require_db().unwrap(),
                &ids.iter()
                    .map(|id| EntryId::from_uuid(*id))
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        );
        let reparsed = parse_entries_document(&document).unwrap();
        assert_eq!(reparsed.len(), 2);
        assert!(reparsed.iter().all(|entry| entry.attachments.len() == 2));
    }
}
