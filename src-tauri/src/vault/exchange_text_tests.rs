use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn fields_db(fields: &[(&str, &str, bool)]) -> (Database, EntryId) {
    let mut db = Database::new();
    let id = {
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", "Synthetic exchange");
        for (name, value, protected) in fields {
            entry.set(
                *name,
                if *protected {
                    Value::protected((*value).to_owned())
                } else {
                    Value::unprotected((*value).to_owned())
                },
            );
        }
        entry.as_ref().id()
    };
    (db, id)
}

fn recovered(db: &Database, id: EntryId) -> PastedEntry {
    let document = Zeroizing::new(build_entries_document(db, &[id]).unwrap());
    let payload = Zeroizing::new(encode_payload(&document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&payload).unwrap());
    assert_eq!(&*decoded, &*document);
    parse_entries_document(&decoded)
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
}

#[test]
fn exchange_text_whitespace_values_survive_the_real_document_pipeline() {
    let values = [
        "", " ", "\t", "\n", "\r", "\r\n", " \t\r\n ", "\u{85}", "\u{a0}", "\u{2028}", "\u{3000}",
    ];
    let mut failures = Vec::new();
    for (index, value) in values.iter().enumerate() {
        for protected in [false, true] {
            let fields = [("Password", *value, protected), ("PIN", *value, protected)];
            let (db, id) = fields_db(&fields);
            let parsed = recovered(&db, id);
            for (key, expected, protect) in fields {
                if !parsed.fields.iter().any(|(name, value, protected)| {
                    name == key && value == expected && *protected == protect
                }) {
                    failures.push(format!("case{index}/{key}/{protected}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "field text/protection mismatches: {failures:?}"
    );
}

#[test]
fn exchange_text_carriage_returns_survive_the_real_document_pipeline() {
    let values = [
        "synthetic\rvalue",
        "synthetic\r\nvalue",
        " \r\t实体😀\r\n<&> ",
        "&#13; &amp;",
    ];
    let mut failures = Vec::new();
    for (index, value) in values.iter().enumerate() {
        for protected in [false, true] {
            let fields = [("Password", *value, protected), ("PIN", *value, protected)];
            let (db, id) = fields_db(&fields);
            let parsed = recovered(&db, id);
            if !fields.iter().all(|(name, expected, protect)| {
                parsed.fields.iter().any(|(key, value, protected)| {
                    key == name && value == expected && protected == protect
                })
            }) {
                failures.push(format!("case{index}/{protected}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "field carriage mismatches: {failures:?}"
    );
}

#[test]
fn exchange_text_pasted_fields_keep_identity_and_protection_after_native_reopen() {
    let fields = [
        ("Password", " \r\t ", true),
        ("PIN", " \t\n ", true),
        ("Visible", "\r\n", false),
        (" Key ", "synthetic\r\n实体<&>😀", false),
        ("\t", "literal &#13;", true),
        ("SecPivot.WebsiteRules", "{\"version\":1,\"rules\":[{\"url\":\"https://synthetic.example:8443/A?Case=%2f#B\",\"mode\":\"exact\"}]}", true),
    ];
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-exchange.kdbx");
    let mut target = VaultSession::default();
    target
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let (db, id) = fields_db(&fields);
    let parsed = recovered(&db, id);
    let new_id = insert_entry(&mut target.require_db_mut().unwrap().root_mut(), &parsed).unwrap();
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
    let mut failures = Vec::new();
    for (name, expected, protected) in fields {
        if !entry
            .fields
            .get(name)
            .is_some_and(|field| field.get() == expected && field.is_protected() == protected)
        {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "saved/reopened field mismatches: {failures:?}"
    );
    let policy = target
        .get_entry_website_rules(&new_id.to_string())
        .unwrap()
        .unwrap();
    assert_eq!(policy.rules.len(), 1);
    assert!(target.state().unwrap().unwrap().root.entries[0].has_password);
}

#[test]
fn exchange_text_reference_controls_keep_empty_and_reject_missing_values() {
    let header = "<KeePassFile><Root><Group><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><Entry><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID>";
    let footer = "</Entry></Group></Root></KeePassFile>";
    for (value, expected) in [
        ("", ""),
        (" &#13;&#9;&#10; ", " \r\t\n "),
        (" a&#13;\nb &amp;#13; ", " a\r\nb &#13; "),
        (" a\r\nb\rc ", " a\nb\nc "),
        ("<![CDATA[ \t ]]>", " \t "),
    ] {
        let xml = format!("{header}<String><Key>PIN</Key><Value Protected=\"True\">{value}</Value></String>{footer}");
        let entry = parse_entries_document(xml.as_bytes()).unwrap().remove(0);
        assert_eq!(entry.fields, vec![("PIN".into(), expected.into(), true)]);
    }
    let missing = format!("{header}<String><Key>PIN</Key></String>{footer}");
    assert!(parse_entries_document(missing.as_bytes()).is_err());
}

#[test]
fn exchange_text_attachment_names_keep_whitespace_carriage_and_protection() {
    let (mut db, id) = fields_db(&[("PIN", "synthetic", true)]);
    let names = ["", " \t ", "synthetic\r\n实体<&>😀.txt"];
    {
        let mut entry = db.entry_mut(id).unwrap();
        for (index, name) in names.iter().enumerate() {
            entry.add_attachment(
                *name,
                if index % 2 == 0 {
                    Value::protected(vec![index as u8, 1, 2])
                } else {
                    Value::unprotected(vec![index as u8, 1, 2])
                },
            );
        }
    }
    let entry = recovered(&db, id);
    assert_eq!(entry.attachments.len(), names.len());
    for (index, name) in names.iter().enumerate() {
        assert!(entry.attachments.iter().any(|(actual, bytes, protected)| {
            actual == name && bytes == &[index as u8, 1, 2] && *protected == (index % 2 == 0)
        }));
    }
}
