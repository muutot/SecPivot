use super::*;
fn uri_settings() -> crate::vault::BitwardenUriSettings {
    crate::vault::BitwardenUriSettings {
        default_match: 0,
        equivalent_domains: vec![],
    }
}

fn fixture() -> Vec<u8> {
    include_bytes!("../../../../tests/fixtures/bitwarden-native.json").to_vec()
}

#[test]
fn six_uri_modes_defaults_raw_source_and_profiles_survive_native_import_kdbx_history_xml_and_hosts()
{
    use crate::{
        bridge::BridgeHost,
        rpc::RpcHost,
        vault::{HistoryRuleSource, WebsiteMode},
    };
    let settings = crate::vault::BitwardenUriSettings {
        default_match: 1,
        equivalent_domains: vec![vec!["example.com".into(), "example.net".into()]],
    };
    let uris = serde_json::json!([
        {"uri":"https://example.net/private-source","match":0},
        {"uri":"\u{feff}https://host.example:443/Original\u{feff}","match":1},
        {"uri":"https://prefix.example/Case","match":2},
        {"uri":"https://exact.example/Case?x=%2f#B","match":3},
        {"uri":"^https://regex\\.example/(?<=example/)login$","match":4},
        {"uri":"https://never.example","match":5},
        {"uri":"http://inherited.example:80/a"},
        {"uri":"https://null.example:443/a","match":null},
        {"uri":"[","match":4},
        {"uri":"","match":4},
        {"uri":null,"match":0}
    ]);
    let source=serde_json::json!({"items":[{"type":1,"name":"Synthetic URI migration","login":{"password":"synthetic-private","uris":uris}}]}).to_string();
    let (_dir, mut session, path) = session();
    let before = std::fs::read(&path).unwrap();
    let plan = prepare_bitwarden_import(source.as_bytes(), &settings).unwrap();
    let lifetime = session.native_import_lifetime("root").unwrap();
    let state = session.import_bitwarden(plan, "root", lifetime).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let masked = serde_json::to_string(&state).unwrap();
    assert!(
        !masked.contains("synthetic-private")
            && !masked.contains("equivalentDomains")
            && !masked.contains("^https://regex")
    );
    let id = session
        .require_db()
        .unwrap()
        .iter_all_entries()
        .find(|e| e.get_title() == Some("Synthetic URI migration"))
        .unwrap()
        .id()
        .uuid()
        .to_string();
    let policy = session.get_entry_website_rules(&id).unwrap().unwrap();
    assert!(policy.bitwarden.as_ref() == Some(&settings));
    assert_eq!(policy.rules.len(), 11);
    assert!(
        policy.rules.iter().map(|r| r.mode).collect::<Vec<_>>()
            == [
                WebsiteMode::Site,
                WebsiteMode::Host,
                WebsiteMode::Prefix,
                WebsiteMode::Exact,
                WebsiteMode::Regex,
                WebsiteMode::Never,
                WebsiteMode::Host,
                WebsiteMode::Host,
                WebsiteMode::Regex,
                WebsiteMode::Regex,
                WebsiteMode::Site
            ]
    );
    assert_eq!(
        policy.rules[1].url,
        "\u{feff}https://host.example:443/Original\u{feff}"
    );
    for reopened in [false, true] {
        if reopened {
            session.save().unwrap();
            session.close();
            session.open(&path, "synthetic-master", None).unwrap();
        }
        assert!(session.get_entry_website_rules(&id).unwrap() == Some(policy.clone()));
        for (url, expected) in [
            ("https://sub.example.com/a", true),
            ("https://host.example/other", true),
            ("https://host.example:8443/other", false),
            ("https://prefix.example/Case/suffix", true),
            ("https://prefix.example/case", false),
            ("https://exact.example/Case?x=%2f#B", true),
            ("https://exact.example/Case?x=%2F#B", false),
            ("https://regex.example/LOGIN", true),
            ("https://never.example", false),
            ("http://inherited.example/other", true),
            ("https://null.example/other", true),
            ("https://unrelated.example/", false),
        ] {
            assert_eq!(
                session.logins_for(url, None).iter().any(|e| e.uuid == id),
                expected
            );
            assert_eq!(
                session
                    .find_logins(&[url.into()], None, None, None)
                    .iter()
                    .any(|e| e.uuid == id),
                expected
            );
        }
        let db = session.require_db().unwrap();
        let entry = db
            .iter_all_entries()
            .find(|e| e.id().uuid().to_string() == id)
            .unwrap();
        let original: Json = serde_json::from_str(entry.fields[SOURCE_ITEM].get()).unwrap();
        assert_eq!(original["login"]["uris"], uris);
        assert!(entry.fields[SOURCE_ITEM].is_protected());
        let urls = crate::vault::helpers::entry_match_urls(&entry);
        assert!(urls
            .iter()
            .all(|u| !u.contains("never") && !u.contains("regex") && !u.contains('\u{feff}')));
        let archive = db
            .iter_all_entries()
            .find(|e| e.fields.contains_key("Bitwarden URI Source Settings"))
            .unwrap();
        assert!(archive.fields["Bitwarden URI Source Settings"].is_protected());
        assert_eq!(
            archive.fields["Bitwarden URI Source Settings"].get(),
            &serde_json::to_string(&settings).unwrap()
        );
    }
    let mut changed = policy.clone();
    changed.rules[0].mode = WebsiteMode::Never;
    session
        .set_entry_website_rules(&id, Some(changed.clone()))
        .unwrap();
    let history = session.get_entry_history(&id).unwrap();
    assert!(history[0].diff.website_rules);
    assert!(
        session
            .get_history_website_rules(
                &id,
                history[0].index,
                &history[0].selection_token,
                HistoryRuleSource::Version
            )
            .unwrap()
            == Some(policy.clone())
    );
    let xml = session.export_xml_content().unwrap();
    let lifetime = session.native_import_lifetime("root").unwrap();
    session
        .import_keepass_xml(
            crate::vault::prepare_keepass_xml_import(xml.as_bytes()).unwrap(),
            "root",
            lifetime,
        )
        .unwrap();
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    let db = session.require_db().unwrap();
    let imported = db
        .iter_all_entries()
        .filter(|e| e.fields.contains_key(FIELD_WEBSITE_RULES))
        .collect::<Vec<_>>();
    assert_eq!(imported.len(), 2);
    for e in imported {
        let raw = e.fields[FIELD_WEBSITE_RULES].get();
        assert!(e.fields[FIELD_WEBSITE_RULES].is_protected());
        let restored: crate::vault::WebsiteRuleSet = serde_json::from_str(raw).unwrap();
        assert!(restored == changed);
        let historical = e.historical(0).unwrap();
        let original: crate::vault::WebsiteRuleSet =
            serde_json::from_str(historical.fields[FIELD_WEBSITE_RULES].get()).unwrap();
        assert!(original == policy);
    }
}

#[test]
fn native_uri_mapping_rejects_shapes_limits_and_settings_before_publication() {
    let mut total = 128 * 1024 * 1024 - 64;
    assert!(reserve_uri_policy_bytes(&mut total, 64, 0).is_ok());
    assert!(reserve_uri_policy_bytes(&mut total, 1, 0).is_err());
    let before = total;
    assert!(reserve_uri_policy_bytes(&mut total, 0, 1).is_err());
    assert_eq!(total, before);
    total = usize::MAX;
    assert!(reserve_uri_policy_bytes(&mut total, 1, 0).is_err());
    for uri in [
        serde_json::json!({"uri":3}),
        serde_json::json!({"uri":"https://example.com","match":-1}),
        serde_json::json!({"uri":"https://example.com","match":6}),
        serde_json::json!({"uri":"https://example.com","match":false}),
        serde_json::json!(null),
    ] {
        let source=serde_json::json!({"items":[{"type":1,"login":{"uris":[{"uri":"https://valid.example"},uri]}}]}).to_string();
        assert!(prepare_bitwarden_import(source.as_bytes(), &uri_settings()).is_err());
    }
    let source=serde_json::json!({"items":[{"type":1,"login":{"uris":vec![serde_json::json!({"uri":"https://example.com"});129]}}]}).to_string();
    assert!(prepare_bitwarden_import(source.as_bytes(), &uri_settings()).is_err());
    let mut invalid = uri_settings();
    invalid.default_match = 6;
    assert!(prepare_bitwarden_import(&fixture(), &invalid).is_err());
    invalid.default_match = 0;
    invalid.equivalent_domains = vec![vec!["a".repeat(1025)]];
    assert!(prepare_bitwarden_import(&fixture(), &invalid).is_err());
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
    let plan = prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap();
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
    let mut plan = prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap();
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
        assert!(prepare_bitwarden_import(document.as_bytes(), &uri_settings()).is_ok());
    }
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend(fixture());
    assert!(prepare_bitwarden_import(&bom, &uri_settings()).is_ok());
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
        r#"{"items":[{"type":1,"login":{"uris":[{"uri":"https://a","match":6}]}}]}"#,
        r#"{"items":[{"type":1,"login":{"uris":[{"uri":"https://a"},{"uri":"https://b","match":"0"}]}}]}"#,
        r#"{"items":[{"type":1}],"encrypted":true}"#,
        r#"{"items":[{"type":1}],"passwordProtected":true}"#,
        r#"{"items":[]} trailing synthetic secret"#,
        r#"[1,2,3]"#,
    ] {
        let error = prepare_bitwarden_import(document.as_bytes(), &uri_settings())
            .err()
            .unwrap();
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
        prepare_bitwarden_import(source.as_bytes(), &uri_settings())
            .err()
            .unwrap(),
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
            prepare_bitwarden_import(source.as_bytes(), &uri_settings()).unwrap(),
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
            prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap(),
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
            prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap(),
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
            prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    session.save_failures = 0;
    assert!(session
        .import_bitwarden(
            prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap(),
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
            prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap(),
            "root",
            lifetime
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &reopened);
}
