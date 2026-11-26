use super::*;

fn fixture() -> Vec<u8> {
    include_bytes!("../../../../tests/fixtures/bitwarden-native.json").to_vec()
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
fn native_bitwarden_preserves_empty_exact_folders_memberships_fields_and_protected_source() {
    let (_dir, mut session, path) = session();
    let disk = std::fs::read(&path).unwrap();
    let original = session.require_db().unwrap().clone();
    let revision = session.revision;
    let plan = prepare_bitwarden_import(&fixture()).unwrap();
    assert_eq!(
        (
            plan.counts.folders,
            plan.counts.collections,
            plan.counts.items,
            plan.counts.unresolved_memberships
        ),
        (4, 3, 2, 1)
    );
    let lifetime = session.native_import_lifetime("root").unwrap();
    let state = session.import_bitwarden(plan, "root", lifetime).unwrap();
    assert_eq!(state.revision, revision + 1);
    assert!(state.dirty);
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    let db = session.require_db().unwrap();
    assert_eq!(db.config, original.config);
    assert_eq!(db.meta, original.meta);
    let destination = db.root();
    let root = destination
        .groups()
        .find(|g| g.name == "Bitwarden")
        .unwrap();
    assert_eq!(
        root.groups().map(|g| g.name.clone()).collect::<Vec<_>>(),
        ["Empty", " Same / Name ", " Same / Name ", ""]
    );
    let groups = root.groups().collect::<Vec<_>>();
    assert_eq!(groups[0].entries().count(), 0);
    assert_eq!(groups[1].entries().count(), 1);
    assert_eq!(groups[2].entries().count(), 1);
    let entry = groups[1].entries().next().unwrap();
    assert_eq!(
        entry.fields["Password"].get(),
        " synthetic-sensitive-password "
    );
    assert!(entry.fields["Password"].is_protected());
    assert!(entry.fields["Bitwarden Empty Secret"].is_protected());
    assert_eq!(entry.fields["Bitwarden Empty Secret"].get(), "");
    assert_eq!(
        entry.fields["Bitwarden Linked Password"].get(),
        entry.fields["Password"].get()
    );
    assert_eq!(entry.fields[SOURCE_ITEM].get(), "synthetic-source-label");
    assert_eq!(
        entry.fields["Bitwarden Source Item (2)"].get(),
        "synthetic-second-label"
    );
    let item: Json = serde_json::from_str(entry.fields["Bitwarden Source Item (3)"].get()).unwrap();
    assert_eq!(item["collectionIds"][2], "outside-managed-scope");
    assert_eq!(item["attachments"][0]["id"], "metadata-only");
    assert_eq!(
        item["unknownExtension"]["secret"],
        "synthetic-sensitive-extension"
    );
    assert!(entry.fields["Bitwarden Source Item (3)"].is_protected());
    assert_eq!(entry.tags, ["Team / Shared", "Team / Other"]);
    assert!(entry.quality_check);
    assert_eq!(entry.get(FIELD_FAVORITE), Some(FIELD_FAVORITE_TRUE));
    assert_eq!(entry.fields[FIELD_OTP].get(), "JBSWY3DPEHPK3PXP");
    assert_eq!(
        entry.times.creation.unwrap().to_string(),
        "2024-01-02 01:04:05"
    );
    assert_eq!(
        entry.times.last_modification.unwrap().to_string(),
        "2024-03-04 05:06:07"
    );
    assert!(entry
        .history
        .as_ref()
        .is_none_or(|h| h.get_entries().is_empty()));
    assert_eq!(entry.attachments().count(), 0);
    assert!(!groups[2].entries().next().unwrap().quality_check);
    let archive = root.entries().next().unwrap();
    assert!(archive.fields["Bitwarden Source Export"].is_protected());
    assert_eq!(
        archive.fields["Bitwarden Source Export"].get().as_bytes(),
        fixture()
    );
    let masked = serde_json::to_string(&state).unwrap();
    for secret in [
        "synthetic-sensitive-password",
        "synthetic-sensitive-history",
        "synthetic-sensitive-extension",
        "synthetic-source-label",
        "JBSWY3DPEHPK3PXP",
    ] {
        assert!(!masked.contains(secret));
    }
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    let db = session.require_db().unwrap();
    let destination = db.root();
    let imported = destination
        .groups()
        .find(|g| g.name == "Bitwarden")
        .unwrap();
    assert_eq!(imported.groups().count(), 4);
    let entry = db
        .iter_all_entries()
        .find(|e| e.get("Title") == Some("Synthetic Login"))
        .unwrap();
    assert_eq!(entry.tags, ["Team / Shared", "Team / Other"]);
    assert_eq!(
        entry.fields["Password"].get(),
        " synthetic-sensitive-password "
    );
    assert_eq!(
        entry.fields["Bitwarden Source Item (3)"].get(),
        serde_json::to_string(&item).unwrap().as_str()
    );
}

#[test]
fn exact_literal_source_values_are_not_internal_keepass_uuid_references() {
    let (_dir, mut session, _) = session();
    let mut plan = prepare_bitwarden_import(&fixture()).unwrap();
    let id = plan.database.iter_all_entries().next().unwrap().id();
    let literal = format!("{{REF:P@I:{}}}", id.uuid().simple());
    plan.database
        .entry_mut(id)
        .unwrap()
        .fields
        .insert("Literal".into(), Value::protected(literal.clone()));
    let lifetime = session.native_import_lifetime("root").unwrap();
    session.import_bitwarden(plan, "root", lifetime).unwrap();
    let entry = session
        .require_db()
        .unwrap()
        .iter_all_entries()
        .find(|e| e.fields.contains_key("Literal"))
        .unwrap();
    assert_ne!(entry.id(), id);
    assert_eq!(entry.fields["Literal"].get(), &literal);
}

#[test]
fn only_folders_or_only_collections_and_legacy_idless_items_are_supported() {
    for document in [
        r#"{"encrypted":false,"folders":[{"id":"a","name":"Empty"}],"items":[]}"#,
        r#"{"encrypted":false,"collections":[{"id":"a","name":"Empty collection"}],"items":[]}"#,
        r#"{"items":[{"type":1,"login":{"password":"synthetic"}}]}"#,
    ] {
        assert!(prepare_bitwarden_import(document.as_bytes()).is_ok());
    }
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend(fixture());
    assert!(prepare_bitwarden_import(&bom).is_ok());
}

#[test]
fn repeated_identity_keys_missing_folders_and_malformed_metadata_reject_whole_document() {
    for document in [
        r#"{"items":[],"items":[]}"#,
        r#"{"items":[{"type":1,"name":"a","name":"b"}]}"#,
        r#"{"folders":[{"id":"a"},{"id":"a"}],"items":[]}"#,
        r#"{"collections":[{"id":"a"},{"id":"a"}],"items":[]}"#,
        r#"{"items":[{"id":"a","type":1},{"id":"a","type":2}]}"#,
        r#"{"items":[{"type":1,"folderId":"missing"}]}"#,
        r#"{"items":[{"type":1,"collectionIds":["a","a"]}]}"#,
        r#"{"items":[{"type":1,"collectionIds":[1]}]}"#,
        r#"{"collections":[{"id":"a","name":"A","organizationId":"org-a"}],"items":[{"type":1,"organizationId":"org-b","collectionIds":["a"]}]}"#,
        r#"{"items":[{"type":1,"creationDate":"bad synthetic date"}]}"#,
        r#"{"items":[{"type":1,"revisionDate":123}]}"#,
        r#"{"items":[{"type":1,"name":"synthetic\u0000"}]}"#,
        r#"{"items":[{"type":1,"fields":[{"name":"synthetic\u0000","type":1,"value":"safe"}]}]}"#,
        r#"{"items":[{"type":1,"login":{"uris":[{"uri":"https://a","match":0}]}}]}"#,
        r#"{"items":[{"type":1,"login":{"uris":[{"uri":"https://a"},{"uri":"https://b"}]}}]}"#,
        r#"{"items":[{"type":1}],"encrypted":true}"#,
        r#"{"items":[{"type":1}],"passwordProtected":true}"#,
        r#"{"items":[]} trailing synthetic secret"#,
        r#"[1,2,3]"#,
    ] {
        let error = prepare_bitwarden_import(document.as_bytes()).err().unwrap();
        assert!(!error.contains("safe"));
        assert!(!error.contains("synthetic"));
    }
}

#[test]
fn resource_bounds_are_checked_before_mapping_and_mutation() {
    assert!(preflight(&vec![b' '; MAX_BYTES + 1]).is_err());
    assert!(preflight("[".repeat(64).as_bytes()).is_err());
    assert!(preflight(",".repeat(262_145).as_bytes()).is_err());
    let folders = (0..8193)
        .map(|n| serde_json::json!({"id":n.to_string(),"name":""}))
        .collect::<Vec<_>>();
    let source = serde_json::json!({"folders":folders,"items":[]}).to_string();
    assert_eq!(
        prepare_bitwarden_import(source.as_bytes()).err().unwrap(),
        LIMIT
    );
}

#[test]
fn collection_display_normalization_and_typed_items_remain_stable_after_save_reopen() {
    let (_dir, mut session, path) = session();
    let mut items = Vec::new();
    for kind in 1..=8 {
        let mut item = serde_json::json!({"id":format!("synthetic-{kind}"),"type":kind,"name":format!("Type {kind}"),"collectionIds":["mixed"]});
        if let Some((section, _, _)) = super::super::typed_section(kind) {
            item[section] = serde_json::json!({"unknownSecret":"synthetic-typed-value"});
        }
        items.push(item);
    }
    let source =
        serde_json::json!({"collections":[{"id":"mixed","name":" A/B; C,D\t E "}],"items":items})
            .to_string();
    let target = session.db.as_mut().unwrap().root_mut().add_group().id();
    let uuid = target.uuid().to_string();
    let lifetime = session.native_import_lifetime(&uuid).unwrap();
    session
        .import_bitwarden(
            prepare_bitwarden_import(source.as_bytes()).unwrap(),
            &uuid,
            lifetime,
        )
        .unwrap();
    assert_eq!(
        session
            .require_db()
            .unwrap()
            .group(target)
            .unwrap()
            .groups()
            .count(),
        1
    );
    for save_reopen in [false, true] {
        if save_reopen {
            session.save().unwrap();
            session.close();
            session.open(&path, "synthetic-master", None).unwrap();
        }
        let db = session.require_db().unwrap();
        let entries = db
            .iter_all_entries()
            .filter(|e| e.get("Title").is_some_and(|s| s.starts_with("Type ")))
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 8);
        for entry in entries {
            assert_eq!(entry.tags, ["A / B", "C", "D", "E"]);
            assert_eq!(entry.quality_check, entry.get("Title") == Some("Type 1"));
            assert!(entry.fields[SOURCE_ITEM].is_protected());
            for field in entry
                .fields
                .values()
                .filter(|v| v.get() == "synthetic-typed-value")
            {
                assert!(field.is_protected());
            }
        }
    }
}

#[test]
fn snapshot_failure_restores_a_staged_external_reload_and_its_token() {
    use crate::vault::file_reload::{check_file, reload_file};
    let (_dir, mut session, path) = session();
    let mut external = VaultSession::default();
    external.open(&path, "synthetic-master", None).unwrap();
    external.db.as_mut().unwrap().root_mut().name = "Synthetic External".into();
    external.mark_dirty();
    external.save().unwrap();
    let ticket = session.prepare_file_check().unwrap();
    let changed = check_file(&ticket).unwrap().unwrap();
    let job = session.prepare_file_reload(&ticket, changed).unwrap();
    let result = reload_file(job).unwrap();
    let token = session
        .stage_file_reload(ticket, result)
        .unwrap()
        .token
        .unwrap();
    let lifetime = session.native_import_lifetime("root").unwrap();
    session.path = None;
    assert!(session
        .import_bitwarden(
            prepare_bitwarden_import(&fixture()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert!(session.pending_file_reload.is_some());
    session.path = Some(path.to_string_lossy().into_owned());
    session.apply_file_reload(&token).unwrap();
    assert_eq!(
        session.require_db().unwrap().root().name,
        "Synthetic External"
    );
    assert_eq!(session.require_db().unwrap().root().groups().count(), 0);
}

#[test]
fn bitwarden_publication_rejects_stale_readonly_and_missing_target_without_partial_mutation() {
    let (_dir, mut session, path) = session();
    let lifetime = session.native_import_lifetime("root").unwrap();
    let before = session.require_db().unwrap().clone();
    let meta = (session.dirty, session.revision, session.modified_at.clone());
    let cache = session
        .cached_snapshot
        .as_ref()
        .map(|(r, s)| (*r, serde_json::to_value(s).unwrap()));
    session.path = None;
    assert!(session
        .import_bitwarden(
            prepare_bitwarden_import(&fixture()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    assert_eq!(
        (session.dirty, session.revision, session.modified_at.clone()),
        meta
    );
    assert_eq!(
        session
            .cached_snapshot
            .as_ref()
            .map(|(r, s)| (*r, serde_json::to_value(s).unwrap())),
        cache
    );
    session.path = Some(path.to_string_lossy().into_owned());
    session.save_failures = u32::MAX;
    assert!(session
        .import_bitwarden(
            prepare_bitwarden_import(&fixture()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    session.save_failures = 0;
    assert!(session
        .import_bitwarden(
            prepare_bitwarden_import(&fixture()).unwrap(),
            &Uuid::new_v4().to_string(),
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    let reopened = session.require_db().unwrap().clone();
    assert!(session
        .import_bitwarden(
            prepare_bitwarden_import(&fixture()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &reopened);
}
