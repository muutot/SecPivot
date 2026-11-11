use super::*;
use crate::vault::VaultSession;
use tempfile::TempDir;
use zeroize::Zeroizing;

fn icon(uuid: &str, data: Option<&str>) -> String {
    format!(
        "<Icon><UUID>{uuid}</UUID>{}</Icon>",
        data.map(|data| format!("<Data>{data}</Data>"))
            .unwrap_or_default()
    )
}

fn document(icons: &str, references: &[Option<&str>]) -> Zeroizing<Vec<u8>> {
    let entries: String = references
        .iter()
        .enumerate()
        .map(|(index, reference)| {
            format!(
                "<Entry><UUID>{}</UUID><IconID>9</IconID>{}<String><Key>Title</Key><Value>Synthetic icon {index}</Value></String></Entry>",
                encode_uuid(Uuid::from_u128(100 + index as u128)),
                reference.map(|uuid| format!("<CustomIconUUID>{uuid}</CustomIconUUID>"))
                    .unwrap_or_default()
            )
        })
        .collect();
    Zeroizing::new(format!(
        "<KeePassFile><Meta><CustomIcons>{icons}</CustomIcons></Meta><Root><Group><UUID>{}</UUID>{entries}</Group></Root></KeePassFile>",
        encode_uuid(Uuid::from_u128(99))
    ).into_bytes())
}

fn framed(document: &[u8]) -> Zeroizing<Vec<u8>> {
    let frame = Zeroizing::new(encode_payload(document, false).unwrap());
    let decoded = Zeroizing::new(decode_payload(&frame).unwrap());
    assert_eq!(&*decoded, document);
    decoded
}

fn saved_icons(entries: &[PastedEntry]) -> Vec<Option<Vec<u8>>> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-icon-identity.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let mut ids = Vec::new();
    for entry in entries {
        ids.push(EntryId::from_uuid(
            insert_entry(&mut session.require_db_mut().unwrap().root_mut(), entry).unwrap(),
        ));
    }
    session.mark_dirty();
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    ids.iter()
        .zip(entries)
        .map(|(id, expected)| {
            let actual = session.require_db().unwrap().entry(*id).unwrap();
            assert_eq!(actual.get_title(), Some(expected.fields[0].1.as_str()));
            let bytes = actual.custom_icon().map(|icon| icon.data.clone());
            assert_eq!(
                bytes,
                expected.custom_icon.as_ref().map(|icon| icon.data.clone())
            );
            if bytes.is_none() {
                assert_eq!(actual.icon(), Some(&Icon::BuiltIn(9)));
            }
            bytes
        })
        .collect()
}

#[test]
fn exchange_icon_identity_rejects_duplicate_definitions_before_publishing() {
    let uuid = encode_uuid(Uuid::from_u128(1));
    assert!(uuid.starts_with('A'));
    let alias = format!("&#65;{}", &uuid[1..]);
    let cases = [
        ("different-forward", Some("AQID"), Some("BAUG"), true, false),
        ("different-reverse", Some("BAUG"), Some("AQID"), true, false),
        ("identical", Some("AQID"), Some("AQID"), true, false),
        ("empty", Some(""), Some(""), true, false),
        ("missing-last", Some("AQID"), None, true, false),
        ("missing-first", None, Some("AQID"), true, false),
        ("invalid-last", Some("AQID"), Some("invalid!"), true, false),
        ("invalid-first", Some("invalid!"), Some("AQID"), true, false),
        ("unreferenced", Some("AQID"), Some("BAUG"), false, false),
        (
            "character-reference",
            Some("AQID"),
            Some("BAUG"),
            true,
            true,
        ),
    ];
    let mut accepted = Vec::new();
    for (name, first, second, referenced, use_alias) in &cases {
        let icons = icon(&uuid, *first) + &icon(if *use_alias { &alias } else { &uuid }, *second);
        let bytes = document(&icons, &[referenced.then_some(uuid.as_str())]);
        let decoded = framed(&bytes);
        let result = parse_entries_document(&decoded);
        if result.is_ok() {
            accepted.push(*name);
        } else {
            assert_eq!(result.err().unwrap(), "自定义图标编号重复");
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted duplicate icon identities: {accepted:?}"
    );
}

#[test]
fn exchange_icon_identity_keeps_unique_shared_and_unreferenced_controls() {
    let first = encode_uuid(Uuid::from_u128(1));
    let second = encode_uuid(Uuid::from_u128(2));
    let empty = encode_uuid(Uuid::from_u128(3));
    let unused = encode_uuid(Uuid::from_u128(4));
    let icons = icon(&second, Some("BAUG"))
        + &icon(&unused, Some("BwgJ"))
        + &icon(&first, Some("AQID"))
        + &icon(&empty, Some(""));
    let bytes = document(
        &icons,
        &[
            Some(&first),
            Some(&second),
            Some(&first),
            Some(&empty),
            None,
        ],
    );
    let parsed = parse_entries_document(&framed(&bytes)).unwrap();
    let expected = vec![
        Some(vec![1, 2, 3]),
        Some(vec![4, 5, 6]),
        Some(vec![1, 2, 3]),
        Some(vec![]),
        None,
    ];
    assert_eq!(
        parsed
            .iter()
            .map(|entry| entry.custom_icon.as_ref().map(|icon| icon.data.clone()))
            .collect::<Vec<_>>(),
        expected
    );
    assert_ne!(
        parsed[0].custom_icon.as_ref().unwrap().data.as_ptr(),
        parsed[2].custom_icon.as_ref().unwrap().data.as_ptr()
    );
    assert_eq!(saved_icons(&parsed), expected);
}

#[test]
fn exchange_icon_identity_native_writer_emits_one_shared_definition() {
    let mut source = Database::new();
    let mut root = source.root_mut();
    let mut first = root.add_entry();
    first.set_unprotected("Title", "Synthetic shared icon");
    let id = first.set_icon_custom_new(vec![1, 2, 3]).id();
    let first_id = first.as_ref().id();
    let mut second = root.add_entry();
    second.set_unprotected("Title", "Synthetic shared icon");
    second.set_icon_custom(id).unwrap();
    let second_id = second.as_ref().id();
    let bytes = Zeroizing::new(build_entries_document(&source, &[first_id, second_id]).unwrap());
    let model: KeePassFile = quick_xml::de::from_reader(&bytes[..]).unwrap();
    assert_eq!(model.meta.custom_icons.unwrap().icons.len(), 1);
    let parsed = parse_entries_document(&framed(&bytes)).unwrap();
    assert_eq!(
        saved_icons(&parsed),
        vec![Some(vec![1, 2, 3]), Some(vec![1, 2, 3])]
    );
}
