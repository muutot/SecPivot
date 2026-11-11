use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

#[test]
fn exchange_reserved_custom_names_reject_before_document_emission() {
    let mut emitted = 0;
    let mut parse_rejections = 0;
    let mut reclassified = 0;
    for name in ["BinDes-", "BinDes-0", "BinDes-00", "BinDes-custom"] {
        for protected in [false, true] {
            for with_attachment in [false, true] {
                let mut db = Database::new();
                let (safe, conflict) = {
                    let mut root = db.root_mut();
                    let safe = root.add_entry().as_ref().id();
                    let mut entry = root.add_entry();
                    entry.set(
                        name,
                        if protected {
                            Value::protected("synthetic-custom-value".to_owned())
                        } else {
                            Value::unprotected("synthetic-custom-value".to_owned())
                        },
                    );
                    if with_attachment {
                        entry.add_attachment("synthetic.bin", Value::protected(vec![1, 2, 3]));
                    }
                    (safe, entry.as_ref().id())
                };
                // The conflict can appear after another selected entry; no
                // complete document may be returned for clipboard publication.
                let result = build_entries_document(&db, &[safe, conflict]);
                if let Err(error) = &result {
                    assert_eq!(error, "条目包含与附件交换格式冲突的字段名");
                }
                if let Ok(document) = result {
                    emitted += 1;
                    let framed = Zeroizing::new(encode_payload(&document, false).unwrap());
                    let decoded = Zeroizing::new(decode_payload(&framed).unwrap());
                    match parse_entries_document(&decoded) {
                        Err(_) => parse_rejections += 1,
                        Ok(entries) => {
                            if !entries[1].fields.iter().any(|(key, _, _)| key == name) {
                                reclassified += 1;
                            }
                        }
                    }
                }
                let source = db.entry(conflict).unwrap();
                assert_eq!(source.get(name), Some("synthetic-custom-value"));
                assert_eq!(source.fields[name].is_protected(), protected);
                assert_eq!(
                    source.attachments_named().count(),
                    usize::from(with_attachment)
                );
            }
        }
    }
    assert_eq!(
        emitted, 0,
        "emitted={emitted}, parse_rejections={parse_rejections}, reclassified={reclassified}"
    );
}

#[test]
fn exchange_near_prefix_custom_fields_and_attachments_survive_native_reopen() {
    let fields = [
        ("bindes-0", "synthetic-case", true),
        ("BinDes", "synthetic-no-dash", false),
        ("xBinDes-0", "synthetic-middle", true),
        (" BinDes-0", "synthetic-leading", false),
    ];
    let mut db = Database::new();
    let id = {
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        for (name, value, protected) in fields {
            entry.set(
                name,
                if protected {
                    Value::protected(value.to_owned())
                } else {
                    Value::unprotected(value.to_owned())
                },
            );
        }
        entry.add_attachment("empty.bin", Value::unprotected(Vec::new()));
        entry.add_attachment("nonempty.bin", Value::protected(vec![1, 2, 3]));
        entry.as_ref().id()
    };
    let document = Zeroizing::new(build_entries_document(&db, &[id]).unwrap());
    let payload = Zeroizing::new(encode_payload(&document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&payload).unwrap());
    let parsed = parse_entries_document(&decoded).unwrap();
    assert_eq!(parsed.len(), 1);
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-field-identity.kdbx");
    let mut target = VaultSession::default();
    target
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let new_id =
        insert_entry(&mut target.require_db_mut().unwrap().root_mut(), &parsed[0]).unwrap();
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&path, "synthetic-master", None).unwrap();
    let entry = target
        .require_db()
        .unwrap()
        .entry(EntryId::from_uuid(new_id))
        .unwrap();
    for (name, value, protected) in fields {
        assert_eq!(entry.get(name), Some(value));
        assert_eq!(entry.fields[name].is_protected(), protected);
    }
    assert_eq!(entry.attachments_named().count(), 2);
    for (name, bytes, protected) in [
        ("empty.bin", Vec::new(), false),
        ("nonempty.bin", vec![1, 2, 3], true),
    ] {
        assert!(entry
            .attachments_named()
            .any(|(actual, attachment)| actual == name
                && attachment.data.get() == &bytes
                && attachment.data.is_protected() == protected));
    }
}
