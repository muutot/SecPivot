use super::*;
use keepass::{
    db::{CustomDataValue, EntryId, Icon},
    DatabaseKey,
};

const ROOT: &str = "AQAAAAAAAAAAAAAAAAAAAA==";
const ENTRY: &str = "AgAAAAAAAAAAAAAAAAAAAA==";
const ICON: &str = "AwAAAAAAAAAAAAAAAAAAAA==";

fn xml(meta: &str, content: &str) -> Vec<u8> {
    format!("<KeePassFile><Meta>{meta}</Meta><Root><Group><UUID>{ROOT}</UUID><Name> Root / 源 </Name>{content}</Group></Root></KeePassFile>").into_bytes()
}

fn fixture() -> Vec<u8> {
    xml(
        &format!(
            r#"<Generator>KeePass</Generator><DatabaseName>Source</DatabaseName><MasterKeyChangeForceOnce>True</MasterKeyChangeForceOnce>
      <CustomIcons><Icon><UUID>{ICON}</UUID><Data>AAH/</Data><Name>synthetic-icon</Name></Icon></CustomIcons>
      <Binaries><Binary ID="41">AAH/</Binary><Binary ID="7"></Binary><Binary ID="99">b3JwaGFu</Binary></Binaries>"#
        ),
        &format!(
            r#"<Group><UUID>BAAAAAAAAAAAAAAAAAAAAA==</UUID><Name> same / name </Name></Group>
      <Group><UUID>BQAAAAAAAAAAAAAAAAAAAA==</UUID><Name> same / name </Name></Group>
      <Entry><UUID>{ENTRY}</UUID><CustomIconUUID>{ICON}</CustomIconUUID><QualityCheck>False</QualityCheck>
      <Times><CreationTime>2024-01-02T03:04:05+02:00</CreationTime><LastModificationTime>2024-01-03T00:00:00Z</LastModificationTime></Times>
      <String><Key>Title</Key><Value>synthetic entry</Value></String>
      <String><Key>Password</Key><Value ProtectInMemory="True"> synthetic&#xD;secret </Value></String>
      <String><Key>Empty</Key><Value ProtectInMemory="True"/></String>
      <String><Key>Notes</Key><Value><![CDATA[literal<&>]]>&amp;&#xD;&#xA; tail </Value></String>
      <String><Key>Reference</Key><Value ProtectInMemory="True">{{REF:P@I:02000000000000000000000000000000}}</Value></String>
      <String><Key>SecPivot.OriginalGroup</Key><Value>01000000-0000-0000-0000-000000000000</Value></String>
      <Binary><Key>current.bin</Key><Value Ref="41"/></Binary><Binary><Key>empty.bin</Key><Value Ref="7"/></Binary>
      <Binary><Key>inline.bin</Key><Value>YWJj</Value></Binary>
      <CustomData><Item><Key>plain</Key><Value>YWJj</Value><LastModificationTime>2024-01-02T00:00:00Z</LastModificationTime></Item></CustomData>
      <AutoType><Enabled>True</Enabled><DefaultSequence>{{REF:P@I:02000000000000000000000000000000}}</DefaultSequence><Association><Window>synthetic</Window><KeystrokeSequence>{{PASSWORD}}</KeystrokeSequence></Association></AutoType>
      <History><Entry><UUID>{ENTRY}</UUID><CustomIconUUID>{ICON}</CustomIconUUID><String><Key>Password</Key><Value ProtectInMemory="True">prior synthetic</Value></String><Binary><Key>old.bin</Key><Value Ref="41"/></Binary></Entry></History>
      </Entry>"#
        ),
    )
}

fn session() -> (tempfile::TempDir, VaultSession, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    (dir, session, path)
}

#[test]
fn native_xml_import_preserves_tree_history_binaries_icons_references_and_destination_settings() {
    let (_dir, mut session, path) = session();
    let original = session.require_db().unwrap().clone();
    let before = session.state().unwrap().unwrap();
    let disk = std::fs::read(&path).unwrap();
    let plan = prepare_keepass_xml_import(&fixture()).unwrap();
    assert_eq!(
        (
            plan.counts.groups,
            plan.counts.items,
            plan.counts.history,
            plan.counts.binaries
        ),
        (3, 1, 1, 4)
    );
    assert_eq!(
        plan.database.iter_all_entries().next().unwrap().custom_data["plain"].value,
        Some(CustomDataValue::String("YWJj".into()))
    );
    let lifetime = session.xml_import_lifetime("root").unwrap();
    let state = session.import_keepass_xml(plan, "root", lifetime).unwrap();
    assert_eq!(state.revision, before.revision + 1);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        disk,
        "import must never save automatically"
    );
    let db = session.require_db().unwrap();
    assert_eq!(db.config, original.config);
    assert_eq!(db.meta, original.meta);
    assert_eq!(db.deleted_objects, original.deleted_objects);
    let destination_root = db.root();
    let root = destination_root
        .groups()
        .find(|g| g.name == " Root / 源 ")
        .unwrap();
    assert_ne!(root.id().uuid(), id(ROOT).unwrap());
    assert!(root.is_expanded);
    assert!(root.groups().all(|g| g.is_expanded));
    assert_eq!(
        root.groups().map(|g| g.name.clone()).collect::<Vec<_>>(),
        vec![" same / name ", " same / name "]
    );
    let entry = root
        .entries()
        .find(|e| {
            e.fields
                .get("Title")
                .is_some_and(|v| v.get() == "synthetic entry")
        })
        .unwrap();
    let imported_id = entry.id();
    assert_ne!(imported_id, EntryId::from_uuid(id(ENTRY).unwrap()));
    assert_eq!(entry.fields["Password"].get(), " synthetic\rsecret ");
    assert!(entry.fields["Password"].is_protected());
    assert!(entry.fields["Empty"].is_protected());
    assert_eq!(entry.fields["Notes"].get(), "literal<&>&\r\n tail ");
    assert_eq!(
        entry.fields["Reference"].get(),
        &format!(
            "{{REF:P@I:{}}}",
            imported_id.uuid().simple().to_string().to_uppercase()
        )
    );
    assert_eq!(
        entry
            .autotype
            .as_ref()
            .unwrap()
            .default_sequence
            .as_ref()
            .unwrap(),
        entry.fields["Reference"].get()
    );
    assert_eq!(
        entry.fields["SecPivot.OriginalGroup"].get(),
        &root.id().uuid().to_string()
    );
    assert_eq!(
        entry.times.creation.unwrap().to_string(),
        "2024-01-02 01:04:05"
    );
    assert_eq!(
        entry
            .attachment_by_name("current.bin")
            .unwrap()
            .get()
            .as_slice(),
        &[0, 1, 255]
    );
    assert!(entry
        .attachment_by_name("empty.bin")
        .unwrap()
        .get()
        .is_empty());
    assert_eq!(
        entry
            .attachment_by_name("inline.bin")
            .unwrap()
            .get()
            .as_slice(),
        b"abc"
    );
    let old = entry.historical(0).unwrap();
    assert_eq!(old.id(), imported_id);
    assert_eq!(old.fields["Password"].get(), "prior synthetic");
    assert_eq!(
        old.attachment_by_name("old.bin").unwrap().id(),
        entry.attachment_by_name("current.bin").unwrap().id()
    );
    assert!(matches!(entry.icon(), Some(Icon::Custom(_))));
    assert_eq!(entry.icon(), old.icon());
    let icon = entry.custom_icon().unwrap();
    assert_eq!(icon.entries(true).count(), 2);
    assert!(db
        .iter_all_attachments()
        .any(|a| a.get().as_slice() == b"orphan"));
    let wire = serde_json::to_string(&state).unwrap();
    for secret in ["synthetic\\rsecret", "prior synthetic", "KeePassFile"] {
        assert!(!wire.contains(secret));
    }
    let wire_entry = &state
        .root
        .children
        .iter()
        .find(|g| g.name == " Root / 源 ")
        .unwrap()
        .entries
        .iter()
        .find(|e| e.title == "synthetic entry")
        .unwrap();
    assert!(wire_entry
        .custom_fields
        .iter()
        .filter(|f| f.protected)
        .all(|f| f.value.is_empty()));
    assert!(state
        .custom_icons
        .as_ref()
        .unwrap()
        .contains_key(&icon.id().uuid().to_string()));
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    let reopened = session.require_db().unwrap().entry(imported_id).unwrap();
    assert_eq!(reopened.fields["Password"].get(), " synthetic\rsecret ");
    assert_eq!(
        reopened
            .historical(0)
            .unwrap()
            .attachment_by_name("old.bin")
            .unwrap()
            .get()
            .as_slice(),
        &[0, 1, 255]
    );
}

#[test]
fn official_library_fixture_export_is_accepted_without_flattening() {
    let bytes = include_bytes!("../../../../tests/fixtures/xml-export-official.kdbx");
    let database = Database::open(
        &mut std::io::Cursor::new(bytes),
        DatabaseKey::new().with_password("synthetic-interop"),
    )
    .unwrap();
    let xml = database.export_plain_xml().unwrap();
    let plan = prepare_keepass_xml_import(&xml).unwrap();
    assert_eq!(plan.database.num_entries(), database.num_entries());
    assert_eq!(plan.database.num_groups(), database.num_groups());
    assert_eq!(plan.database.num_attachments(), database.num_attachments());
    for source in database.iter_all_entries() {
        let imported = plan.database.entry(source.id()).unwrap();
        assert_eq!(imported.fields, source.fields);
        assert_eq!(imported.autotype, source.autotype);
        assert_eq!(imported.times, source.times);
        assert_eq!(imported.history, source.history);
    }
}

#[test]
fn official_resave_of_native_xml_import_reopens_history_and_resolved_references() {
    let bytes = include_bytes!("../../../../tests/fixtures/xml-import-official.kdbx");
    let database = Database::open(
        &mut std::io::Cursor::new(bytes),
        DatabaseKey::new().with_password("synthetic-master"),
    )
    .unwrap();
    let entry = database
        .iter_all_entries()
        .find(|e| {
            e.fields
                .get("Title")
                .is_some_and(|v| v.get() == "synthetic entry")
        })
        .unwrap();
    assert_eq!(entry.fields["Password"].get(), " synthetic\rsecret ");
    assert_eq!(entry.fields["Notes"].get(), "literal<&>&\n tail ");
    assert_eq!(
        entry.fields["Reference"].get(),
        &format!(
            "{{REF:P@I:{}}}",
            entry.id().uuid().simple().to_string().to_uppercase()
        )
    );
    assert_eq!(
        entry
            .attachment_by_name("current.bin")
            .unwrap()
            .get()
            .as_slice(),
        &[0, 1, 255]
    );
    assert_eq!(
        entry.historical(0).unwrap().fields["Password"].get(),
        "prior synthetic"
    );
    assert_eq!(
        entry
            .historical(0)
            .unwrap()
            .attachment_by_name("old.bin")
            .unwrap()
            .get()
            .as_slice(),
        &[0, 1, 255]
    );
    assert_eq!(entry.custom_icon().unwrap().data, vec![0, 1, 255]);
    assert!(database.iter_all_entries().any(|e| e
        .fields
        .get("KeePass Source XML")
        .is_some_and(|v| v.is_protected())));
}

#[test]
fn legacy_plain_protected_values_and_base64_dates_remain_supported() {
    let seconds = 63_871_000_000_i64;
    let time = STANDARD.encode(seconds.to_le_bytes());
    let bytes = xml(
        "<Generator>SecPivot</Generator>",
        &format!(
            r#"<Entry><String><Key>Password</Key><Value Protected="True">IHN5bnRoZXRpYyA=</Value></String><Times><CreationTime>{time}</CreationTime></Times></Entry>"#
        ),
    );
    let plan = prepare_keepass_xml_import(&bytes).unwrap();
    let entry = plan.database.iter_all_entries().next().unwrap();
    assert_eq!(entry.fields["Password"].get(), " synthetic ");
    assert!(entry.fields["Password"].is_protected());
    assert!(entry.times.creation.is_some());
    let utf16: Vec<_> = [0xff, 0xfe]
        .into_iter()
        .chain(
            String::from_utf8(bytes)
                .unwrap()
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        )
        .collect();
    assert!(prepare_keepass_xml_import(&utf16).is_ok());
}

#[test]
fn malformed_identity_duplicates_missing_references_and_ciphertext_fail_without_payload_errors() {
    let valid = String::from_utf8(fixture()).unwrap();
    let invalid = [
        valid.replace("<Meta>", "<Meta><1Invalid/>"),
        valid.replace("<Meta>", "<Meta 1Invalid=\"x\">"),
        valid.replace("Ref=\"41\"", "Ref=\"42\""),
        valid.replace("ID=\"7\"", "ID=\"41\""),
        valid.replace("BQAAAAAAAAAAAAAAAAAAAA==", "BAAAAAAAAAAAAAAAAAAAAA=="),
        valid.replace("<Key>Empty</Key>", "<Key>Password</Key>"),
        valid.replace("<History><Entry><UUID>Ag", "<History><Entry><UUID>Bg"),
        valid.replace("ProtectInMemory=\"True\"", "Protected=\"True\""),
        valid.replace("ProtectInMemory=\"True\"", "ProtectInMemory=\"invalid\""),
        valid.replace("<Data>AAH/</Data>", "<Data>invalid!</Data>"),
        valid.replace(
            "AwAAAAAAAAAAAAAAAAAAAA==</CustomIconUUID>",
            "BwAAAAAAAAAAAAAAAAAAAA==</CustomIconUUID>",
        ),
        valid.replace(
            "<LastModificationTime>2024-01-03T00:00:00Z</LastModificationTime>",
            "<LastModificationTime>bad-date</LastModificationTime>",
        ),
        format!("<!DOCTYPE KeePassFile [<!ENTITY x SYSTEM 'file:///synthetic'>]>{valid}"),
        format!("{valid}<Other/>"),
        format!("{valid}synthetic-sensitive-marker"),
        valid.replace("<Value>synthetic entry</Value>", "<Value>&#0;</Value>"),
        valid.replace(
            "<Value>synthetic entry</Value>",
            "<Value>&external;</Value>",
        ),
    ];
    for bytes in invalid {
        let error = prepare_keepass_xml_import(bytes.as_bytes()).err().unwrap();
        assert!(
            [INVALID, LIMIT, ENCRYPTED].contains(&error.as_str()),
            "fixed error category"
        );
    }
}

#[test]
fn compression_limits_and_invalid_policies_are_checked_before_import() {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(b"synthetic-compressed").unwrap();
    let compressed = STANDARD.encode(encoder.finish().unwrap());
    let bytes = xml(
        &format!(r#"<Binaries><Binary ID="8" Compressed="True">{compressed}</Binary></Binaries>"#),
        r#"<Entry><String><Key>Password</Key><Value ProtectInMemory="True"/></String><Binary><Key>data</Key><Value Ref="8"/></Binary></Entry>"#,
    );
    let plan = prepare_keepass_xml_import(&bytes).unwrap();
    assert_eq!(
        plan.database
            .iter_all_entries()
            .next()
            .unwrap()
            .attachment_by_name("data")
            .unwrap()
            .get()
            .as_slice(),
        b"synthetic-compressed"
    );
    let defaults =
        prepare_keepass_xml_import(&xml("", "<IsExpanded/><Entry><AutoType/></Entry>")).unwrap();
    assert!(defaults.database.root().is_expanded);
    assert!(
        defaults
            .database
            .iter_all_entries()
            .next()
            .unwrap()
            .autotype
            .as_ref()
            .unwrap()
            .enabled
    );
    let mut prep = Preparation {
        expanded: MAX_BINARY - 1,
        ..Default::default()
    };
    let mut binary = Node::new("Binary");
    *binary.text = compressed;
    binary
        .attrs
        .insert("Compressed".into(), Zeroizing::new("True".into()));
    assert_eq!(prep.binary(&binary).err().unwrap(), LIMIT);
    let depth = format!("{}{}", "<Group>".repeat(70), "</Group>".repeat(70));
    assert_eq!(
        prepare_keepass_xml_import(&xml("", &depth)).err().unwrap(),
        LIMIT
    );
    let policy = r#"<Entry><String><Key>SecPivot.WebsiteRules</Key><Value>{"version":1,"rules":[{"url":"https://synthetic.example","mode":"never"}]}</Value></String></Entry>"#;
    let plan = prepare_keepass_xml_import(&xml("", policy)).unwrap();
    assert!(
        plan.database.iter_all_entries().next().unwrap().fields["SecPivot.WebsiteRules"]
            .is_protected()
    );
    assert!(prepare_keepass_xml_import(&xml("", &policy.replace("never", "invalid"))).is_err());
    assert!(prepare_keepass_xml_import(&xml(
        "",
        &policy.replace(
            "<Entry>",
            "<Entry><OverrideURL>https://other.example</OverrideURL>"
        )
    ))
    .is_err());
}

#[test]
fn copying_into_selected_group_preserves_literal_reference_text_inside_website_rules() {
    let (_dir, mut session, _) = session();
    let target = session.db.as_mut().unwrap().root_mut().add_group().id();
    let target_uuid = target.uuid().to_string();
    let policy = r#"{"version":1,"rules":[{"url":"https://synthetic.example/?literal={REF:P@I:02000000000000000000000000000000}","mode":"exact"}]}"#;
    let source = xml("", &format!("<Entry><UUID>{ENTRY}</UUID><String><Key>SecPivot.WebsiteRules</Key><Value>{policy}</Value></String></Entry>"));
    let plan = prepare_keepass_xml_import(&source).unwrap();
    let lifetime = session.xml_import_lifetime(&target_uuid).unwrap();
    session
        .import_keepass_xml(plan, &target_uuid, lifetime)
        .unwrap();
    let db = session.require_db().unwrap();
    let parent = db.group(target).unwrap();
    let copied = parent.groups().next().unwrap();
    let entry = copied
        .entries()
        .find(|e| e.fields.contains_key("SecPivot.WebsiteRules"))
        .unwrap();
    assert_eq!(entry.fields["SecPivot.WebsiteRules"].get(), policy);
    assert_ne!(entry.id().uuid(), id(ENTRY).unwrap());
    assert_eq!(db.root().groups().count(), 1);
}

#[test]
fn target_readonly_snapshot_and_reopened_lifetime_failures_restore_exact_session() {
    let (_dir, mut session, path) = session();
    let lifetime = session.xml_import_lifetime("root").unwrap();
    let before = session.require_db().unwrap().clone();
    let meta = (session.dirty, session.revision, session.modified_at.clone());
    let cache = session
        .cached_snapshot
        .as_ref()
        .map(|(revision, state)| (*revision, serde_json::to_value(state).unwrap()));
    session.path = None;
    assert!(session
        .import_keepass_xml(
            prepare_keepass_xml_import(&fixture()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    assert_eq!(
        session
            .cached_snapshot
            .as_ref()
            .map(|(revision, state)| (*revision, serde_json::to_value(state).unwrap())),
        cache
    );
    assert_eq!(
        (session.dirty, session.revision, session.modified_at.clone()),
        meta
    );
    session.path = Some(path.to_string_lossy().into_owned());
    session.save_failures = u32::MAX;
    assert!(session
        .import_keepass_xml(
            prepare_keepass_xml_import(&fixture()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    session.save_failures = 0;
    assert!(session
        .import_keepass_xml(
            prepare_keepass_xml_import(&fixture()).unwrap(),
            &Uuid::new_v4().to_string(),
            lifetime
        )
        .is_err());
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    let reopened = session.require_db().unwrap().clone();
    assert!(session
        .import_keepass_xml(
            prepare_keepass_xml_import(&fixture()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &reopened);
}
