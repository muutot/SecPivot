use super::*;
use crate::vault::VaultSession;
use keepass::db::DataTransferObfuscation;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn configurations() -> Vec<AutoType> {
    [false, true]
        .into_iter()
        .flat_map(|enabled| {
            [
                DataTransferObfuscation::None,
                DataTransferObfuscation::UseClipboard,
            ]
            .into_iter()
            .map(move |data_transfer_obfuscation| AutoType {
                enabled,
                default_sequence: Some("{USERNAME}{TAB}{PASSWORD}{ENTER}".into()),
                data_transfer_obfuscation,
                associations: vec![
                    AutoTypeAssociation {
                        window: "Synthetic editor *".into(),
                        sequence: "{USERNAME}".into(),
                    },
                    AutoTypeAssociation {
                        window: "Synthetic browser *".into(),
                        sequence: "{PASSWORD}".into(),
                    },
                ],
            })
        })
        .collect()
}

fn seed(database: &mut Database, configs: &[AutoType]) -> Vec<EntryId> {
    let mut root = database.root_mut();
    configs
        .iter()
        .enumerate()
        .map(|(index, config)| {
            let mut entry = root.add_entry();
            entry.set_unprotected("Title", format!("Synthetic auto-type {index}"));
            entry.autotype = Some(config.clone());
            entry.as_ref().id()
        })
        .collect()
}

fn flag(auto_type: &AutoType) -> u8 {
    match auto_type.data_transfer_obfuscation {
        DataTransferObfuscation::None => 0,
        DataTransferObfuscation::UseClipboard => 1,
    }
}

fn decoded(document: &[u8]) -> Zeroizing<Vec<u8>> {
    let frame = Zeroizing::new(encode_payload(document, false).unwrap());
    let bytes = Zeroizing::new(decode_payload(&frame).unwrap());
    assert_eq!(&*bytes, document);
    bytes
}

fn insert(database: &mut Database, parsed: &[PastedEntry]) -> Vec<EntryId> {
    parsed
        .iter()
        .map(|entry| EntryId::from_uuid(insert_entry(&mut database.root_mut(), entry).unwrap()))
        .collect()
}

#[test]
fn exchange_autotype_keeps_complete_native_config_in_memory() {
    let configs = configurations();
    let mut source = Database::new();
    let source_ids = seed(&mut source, &configs);
    let bytes = Zeroizing::new(build_entries_document(&source, &source_ids).unwrap());
    let parsed = parse_entries_document(&decoded(&bytes)).unwrap();
    let mut target = Database::new();
    let target_ids = insert(&mut target, &parsed);

    let mut lost = Vec::new();
    for (index, id) in target_ids.iter().enumerate() {
        if target.entry(*id).unwrap().autotype.as_ref() != Some(&configs[index]) {
            lost.push(index);
        }
    }
    assert!(
        lost.is_empty(),
        "changed native auto-type configurations: {lost:?}"
    );
}

#[test]
fn exchange_autotype_keeps_reopened_source_config_after_target_reopen() {
    let dir = TempDir::new().unwrap();
    let source_path = dir.path().join("synthetic-auto-type-source.kdbx");
    let target_path = dir.path().join("synthetic-auto-type-target.kdbx");
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
    let configs = configurations();
    let source_ids = seed(source.require_db_mut().unwrap(), &configs);
    source.mark_dirty();
    source.save().unwrap();
    source.close();
    source.open(&source_path, "synthetic-master", None).unwrap();
    for (index, id) in source_ids.iter().enumerate() {
        assert_eq!(
            source
                .require_db()
                .unwrap()
                .entry(*id)
                .unwrap()
                .autotype
                .as_ref(),
            Some(&configs[index])
        );
    }
    let bytes =
        Zeroizing::new(build_entries_document(source.require_db().unwrap(), &source_ids).unwrap());
    let parsed = parse_entries_document(&decoded(&bytes)).unwrap();
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
    let target_ids = insert(target.require_db_mut().unwrap(), &parsed);
    target.mark_dirty();
    target.save().unwrap();
    target.close();
    target.open(&target_path, "synthetic-master", None).unwrap();

    let mut lost = Vec::new();
    for (index, id) in target_ids.iter().enumerate() {
        if target
            .require_db()
            .unwrap()
            .entry(*id)
            .unwrap()
            .autotype
            .as_ref()
            != Some(&configs[index])
        {
            lost.push(index);
        }
    }
    assert!(
        lost.is_empty(),
        "changed reopened auto-type configurations: {lost:?}"
    );
}

fn document(auto_type: bool, code: Option<&str>) -> Zeroizing<Vec<u8>> {
    let config = if auto_type {
        format!("<AutoType><Enabled>True</Enabled>{}<DefaultSequence>{{USERNAME}}</DefaultSequence></AutoType>",code.map(|value| format!("<DataTransferObfuscation>{value}</DataTransferObfuscation>")).unwrap_or_default())
    } else {
        String::new()
    };
    Zeroizing::new(format!("<KeePassFile><Root><Group><UUID>{}</UUID><Entry><UUID>{}</UUID><String><Key>Title</Key><Value>Synthetic auto-type control</Value></String>{config}</Entry></Group></Root></KeePassFile>",encode_uuid(Uuid::from_u128(1)),encode_uuid(Uuid::from_u128(2))).into_bytes())
}

#[test]
fn exchange_autotype_keeps_absent_and_zero_legacy_controls() {
    for (has_config, code) in [
        (false, None),
        (true, None),
        (true, Some("0")),
        (true, Some(" 0 ")),
        (true, Some("00")),
        (true, Some("+0")),
    ]
    .into_iter()
    {
        let bytes = document(has_config, code);
        let parsed = parse_entries_document(&decoded(&bytes)).unwrap();
        let mut target = Database::new();
        let ids = insert(&mut target, &parsed);
        let actual = target.entry(ids[0]).unwrap();
        if has_config {
            assert_eq!(flag(actual.autotype.as_ref().unwrap()), 0);
        } else {
            assert!(actual.autotype.is_none());
        }
    }
}

#[test]
fn exchange_autotype_resolves_known_one_and_rejects_invalid_codes() {
    let cases = [
        ("1", Some(1)),
        (" 1 ", Some(1)),
        ("01", Some(1)),
        ("+1", Some(1)),
        ("\u{a0}1\u{3000}", Some(1)),
        ("2", None),
        ("-1", None),
        ("word", None),
        ("", None),
        ("4294967296", None),
        ("0.0", None),
    ];
    let mut wrong = Vec::new();
    for (index, (code, expected)) in cases.iter().enumerate() {
        let bytes = document(true, Some(code));
        let parsed = parse_entries_document(&decoded(&bytes));
        let actual = match &parsed {
            Ok(entries) => {
                let mut target = Database::new();
                let ids = insert(&mut target, entries);
                Some(flag(
                    target.entry(ids[0]).unwrap().autotype.as_ref().unwrap(),
                ))
            }
            Err(error) => {
                assert_eq!(error, "条目自动输入配置无效");
                None
            }
        };
        if actual != *expected {
            wrong.push(index);
        }
    }

    assert!(
        wrong.is_empty(),
        "incorrect auto-type code decisions: {wrong:?}"
    );
}
