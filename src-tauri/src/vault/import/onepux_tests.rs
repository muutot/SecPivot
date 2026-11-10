use super::*;
use serde_json::json;
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn item() -> Json {
    json!({
        "uuid":"same-source-id", "categoryUuid":"001", "state":"active", "favIndex":1,
        "createdAt":1700000000i64, "updatedAt":1700000010i64,
        "overview":{"title":"Synthetic Login", "url":"https://first.example", "urls":[{"label":"second","url":"https://second.example"}], "tags":[" a/b ","测试"]},
        "details":{"notesPlain":"first\r\nsecond", "loginFields":[
            {"designation":"username", "value":" demo ", "fieldType":"T"},
            {"designation":"password", "value":" synthetic-password ", "fieldType":"P"}
        ], "sections":[{"title":"Extra", "fields":[
            {"title":"Password", "guarded":false, "value":{"concealed":"synthetic-pin"}},
            {"title":"Password", "value":{"string":"synthetic-other"}},
            {"title":"OTP", "value":{"totp":"JBSWY3DPEHPK3PXP"}},
            {"title":"Unknown", "value":{"newType":{"value":"synthetic-unknown"}}},
            {"title":"Document", "value":{"file":{"documentId":"doc","fileName":"note.bin"}}}
        ]}], "documentAttributes":{"documentId":"doc","fileName":"note.bin","decryptedSize":3},
        "passwordHistory":[{"value":"prior-synthetic","time":1700000000i64}]}
    })
}

fn data() -> Json {
    let mut archived = item();
    archived["state"] = json!("archived");
    archived["overview"]["title"] = json!("Archived Synthetic");
    archived["categoryUuid"] = json!("002");
    json!({"accounts":[{"attrs":{"uuid":"account-a","name":" Same / Account "},"vaults":[
        {"attrs":{"uuid":"vault-a","name":" Same / Vault "},"items":[item()]},
        {"attrs":{"uuid":"vault-b","name":" Same / Vault "},"items":[archived]}
    ]}], "futureMetadata":{"concealed":"synthetic-parent-secret"}})
}

fn bytes(data: &Json, files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in [
        ("export.attributes", br#"{"version":3}"#.as_slice()),
        ("export.data", serde_json::to_vec(data).unwrap().as_slice()),
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(content).unwrap();
    }
    for (name, content) in files {
        writer.start_file(*name, options).unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn fixture(data: &Json) -> Vec<u8> {
    bytes(
        data,
        &[
            ("files/doc__note.bin", b"\0\xffx"),
            ("files/empty___empty.bin", b""),
            ("files/unrelated.bin", b"synthetic-orphan"),
        ],
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
fn native_migration_masks_secrets_and_preserves_identity_names_and_reopened_binaries() {
    let (_dir, mut session, path) = session();
    let plan = prepare_1pux_import(&fixture(&data())).unwrap();
    let counts = plan.counts();
    assert_eq!(
        (
            counts.accounts,
            counts.vaults,
            counts.items,
            counts.archived,
            counts.attachments,
            counts.unreferenced_files
        ),
        (1, 2, 2, 1, 2, 2)
    );
    let before = session.state().unwrap().unwrap();
    let state = session.import_1pux(plan, "root").unwrap();
    assert_eq!(state.revision, before.revision + 1);
    assert!(state.dirty);
    let wire = serde_json::to_string(&state).unwrap();
    for marker in [
        "synthetic-password",
        "synthetic-pin",
        "synthetic-other",
        "JBSWY3DPEHPK3PXP",
        "prior-synthetic",
        "synthetic-parent-secret",
        "synthetic-orphan",
    ] {
        assert!(!wire.contains(marker));
    }
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    let db = session.require_db().unwrap();
    let db_root = db.root();
    let root = db_root.groups().find(|g| g.name == "1Password").unwrap();
    let account = root.groups().next().unwrap();
    assert_eq!(account.name, " Same / Account ");
    let vaults: Vec<_> = account.groups().collect();
    assert_eq!(vaults.len(), 2);
    assert_eq!(vaults[0].name, " Same / Vault ");
    assert_eq!(vaults[1].name, " Same / Vault ");
    let live = vaults[0].entries().next().unwrap();
    let archived = vaults[1].groups().next().unwrap();
    assert_eq!(archived.name, "归档");
    let archived = archived.entries().next().unwrap();
    assert_ne!(live.id(), archived.id());
    assert_eq!(live.get("UserName"), Some(" demo "));
    assert_eq!(live.get("Password"), Some(" synthetic-password "));
    assert!(live.fields["Password"].is_protected());
    assert!(live.fields["otp"].is_protected());
    assert_eq!(live.get("Notes"), Some("first\r\nsecond"));
    assert_eq!(
        live.times.creation.unwrap().and_utc().timestamp(),
        1700000000
    );
    assert_eq!(
        live.times.last_modification.unwrap().and_utc().timestamp(),
        1700000010
    );
    assert!(live.quality_check && !archived.quality_check);
    assert_eq!(live.get("SecPivot.Favorite"), Some("true"));
    // Native KeePass tag reading trims edges; source JSON preserves originals.
    assert_eq!(live.tags, ["a/b", "测试"]);
    assert_eq!(
        live.history
            .as_ref()
            .map_or(0, |history| history.get_entries().len()),
        0
    );
    assert!(live.fields["1Password Source Item"].is_protected());
    let source: Json = serde_json::from_str(live.get("1Password Source Item").unwrap()).unwrap();
    assert_eq!(
        source["details"]["passwordHistory"][0]["value"],
        "prior-synthetic"
    );
    assert_eq!(
        source["overview"]["urls"][0]["url"],
        "https://second.example"
    );
    assert!(live.fields["1Password Extra Password"].is_protected());
    assert_eq!(source["overview"]["tags"][0], " a/b ");
    assert!(live.fields["1Password Extra Password (2)"].is_protected());
    let attached = live
        .attachments_named()
        .find(|(name, _)| *name == "note.bin")
        .unwrap()
        .1;
    assert!(attached.data.is_protected());
    assert_eq!(attached.data.get(), b"\0\xffx");
    let metadata = root.entries().next().unwrap();
    assert!(metadata.fields["1Password Export Data"].is_protected());
    let unused: Vec<_> = metadata.attachments_named().collect();
    assert_eq!(unused.len(), 2);
    assert!(unused.iter().all(|(_, a)| a.data.is_protected()));
    assert!(unused
        .iter()
        .any(|(name, a)| *name == "files/empty___empty.bin" && a.data.get().is_empty()));
}

#[test]
fn exact_double_triple_separator_and_empty_documents_are_supported() {
    let mut data = data();
    for vault in data["accounts"][0]["vaults"].as_array_mut().unwrap() {
        let item = &mut vault["items"][0];
        item["details"]["documentAttributes"] =
            json!({"documentId":"empty","fileName":"empty.bin","decryptedSize":0});
        item["details"]["sections"] = json!([]);
    }
    let (_dir, mut session, _) = session();
    let plan = prepare_1pux_import(&fixture(&data)).unwrap();
    assert_eq!(plan.counts().attachments, 2);
    session.import_1pux(plan, "root").unwrap();
    fn count_empty(group: &keepass::db::GroupRef<'_>) -> usize {
        let count = group
            .entries()
            .map(|entry| {
                entry
                    .attachments_named()
                    .filter(|(name, a)| {
                        if *name != "empty.bin" {
                            return false;
                        }
                        assert!(a.data.get().is_empty());
                        true
                    })
                    .count()
            })
            .sum::<usize>();
        count
            + group
                .groups()
                .map(|group| count_empty(&group))
                .sum::<usize>()
    }
    let count = count_empty(&session.require_db().unwrap().root());
    assert_eq!(count, 2);
}

#[test]
fn malformed_identity_fields_and_documents_reject_before_mutation() {
    let original = data();
    for (pointer, value) in [
        ("/accounts/0/attrs/uuid", json!("")),
        ("/accounts/0/vaults/1/attrs/uuid", json!("vault-a")),
        (
            "/accounts/0/vaults/0/items/0/details/loginFields/1/designation",
            json!("username"),
        ),
        (
            "/accounts/0/vaults/0/items/0/details/loginFields/0/fieldType",
            json!("P"),
        ),
        (
            "/accounts/0/vaults/0/items/0/details/loginFields/1/value",
            json!(false),
        ),
        (
            "/accounts/0/vaults/0/items/0/details/documentAttributes/decryptedSize",
            json!(4),
        ),
        ("/accounts/0/vaults/0/items/0/state", json!("deleted")),
        ("/accounts/0/vaults/0/items/0/overview/tags", json!([false])),
        ("/accounts/0/vaults/0/items/0/createdAt", json!(i64::MAX)),
        (
            "/accounts/0/vaults/0/items/0/details/sections/0/fields/0/value",
            json!("raw"),
        ),
    ] {
        let mut data = original.clone();
        *data.pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            prepare_1pux_import(&fixture(&data)).err().as_deref(),
            Some(INVALID)
        );
    }
    let mut data = original.clone();
    let duplicated = data["accounts"][0]["vaults"][0]["items"][0].clone();
    data["accounts"][0]["vaults"][0]["items"]
        .as_array_mut()
        .unwrap()
        .push(duplicated);
    assert!(prepare_1pux_import(&fixture(&data)).is_err());
    assert!(prepare_1pux_import(&bytes(&original, &[])).is_err());
    assert!(prepare_1pux_import(&bytes(
        &original,
        &[
            ("files/doc__note.bin", b"\0\xffx"),
            ("files/doc___note.bin", b"\0\xffx")
        ]
    ))
    .is_err());
    assert!(prepare_1pux_import(&bytes(
        &original,
        &[("files/unrelated/doc__note.bin", b"\0\xffx")]
    ))
    .is_err());
}

#[test]
fn bad_target_read_only_closed_and_snapshot_failure_leave_exact_previous_database() {
    let fixture = fixture(&data());
    let (_dir, mut session, _) = session();
    for target in ["bad-id", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"] {
        let before = session.require_db().unwrap().clone();
        let state = session.state().unwrap().unwrap();
        assert!(session
            .import_1pux(prepare_1pux_import(&fixture).unwrap(), target)
            .is_err());
        assert_eq!(session.require_db().unwrap(), &before);
        assert_eq!(session.state().unwrap().unwrap().revision, state.revision);
    }
    let before = session.require_db().unwrap().clone();
    let before_revision = session.revision;
    session.path = None; // Deterministic final-snapshot failure seam.
    assert!(session
        .import_1pux(prepare_1pux_import(&fixture).unwrap(), "root")
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    assert_eq!(session.revision, before_revision);
    session.save_failures = u32::MAX;
    assert!(session
        .import_1pux(prepare_1pux_import(&fixture).unwrap(), "root")
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    session.close();
    assert!(session
        .import_1pux(prepare_1pux_import(&fixture).unwrap(), "root")
        .is_err());
    assert!(session.require_db().is_err());
}

#[test]
fn tiny_budget_prevents_repeated_binary_copies_and_field_plan_growth() {
    let mut budget = Budget {
        binary: 256 * 1024 * 1024 - 1,
        ..Default::default()
    };
    assert_eq!(budget.binary(b"ab").err().as_deref(), Some(LIMIT));
    let mut budget = Budget {
        fields: 65_536,
        ..Default::default()
    };
    assert_eq!(budget.text("a", true).err().as_deref(), Some(LIMIT));
    let mut budget = Budget {
        groups: 8192,
        ..Default::default()
    };
    assert_eq!(budget.group("a").err().as_deref(), Some(LIMIT));
}

#[test]
fn guarded_username_and_type_aliases_cannot_expose_a_secret() {
    let mut data = data();
    let field = &mut data["accounts"][0]["vaults"][0]["items"][0]["details"]["loginFields"][0];
    field["guarded"] = json!(true);
    field["value"] = json!("synthetic-hidden-username");
    let (_dir, mut session, _) = session();
    let state = session
        .import_1pux(prepare_1pux_import(&fixture(&data)).unwrap(), "root")
        .unwrap();
    assert!(!serde_json::to_string(&state)
        .unwrap()
        .contains("synthetic-hidden-username"));
    let field = &mut data["accounts"][0]["vaults"][0]["items"][0]["details"]["loginFields"][0];
    field["type"] = json!("P");
    assert!(prepare_1pux_import(&fixture(&data)).is_err());
}

#[test]
fn existing_target_group_receives_the_whole_import_without_merging_equal_names() {
    let (_dir, mut session, _) = session();
    let target = {
        let db = session.require_db_mut().unwrap();
        let mut root = db.root_mut();
        let mut target = root.add_group();
        target.name = "Target / Group".into();
        target.id().uuid().to_string()
    };
    session
        .import_1pux(prepare_1pux_import(&fixture(&data())).unwrap(), &target)
        .unwrap();
    let db = session.require_db().unwrap();
    let root = db.root();
    assert_eq!(root.groups().filter(|g| g.name == "1Password").count(), 0);
    let target = db.group(parse_group_id(&target).unwrap()).unwrap();
    assert_eq!(target.groups().filter(|g| g.name == "1Password").count(), 1);
}
