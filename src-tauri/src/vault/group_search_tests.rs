//! Search-setting regressions use only synthetic entries and temporary KDBX.
use super::*;
use tempfile::TempDir;

const PASSWORD: &str = "synthetic-group-search-only";

struct Fixture {
    _dir: TempDir,
    session: VaultSession,
    path: PathBuf,
    parent: String,
    child: String,
    entry: String,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.session.close();
    }
}

fn find_group<'a>(root: &'a VaultGroup, name: &str) -> Option<&'a VaultGroup> {
    if root.name == name {
        return Some(root);
    }
    root.children
        .iter()
        .find_map(|child| find_group(child, name))
}

fn add_group(session: &mut VaultSession, parent: &str, name: &str) -> String {
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(parent.into()),
            name: name.into(),
            icon: None,
        })
        .unwrap();
    find_group(&state.root, name).unwrap().uuid.clone()
}

fn add_entry(session: &mut VaultSession, group: &str, title: &str) -> String {
    let input = serde_json::from_value::<EntryInput>(serde_json::json!({
        "groupUuid": group, "title": title, "username": "", "password": "", "url": "", "notes": ""
    }))
    .unwrap();
    let state = session.add_entry(&input).unwrap();
    fn find_entry(root: &VaultGroup, title: &str) -> Option<String> {
        root.entries
            .iter()
            .find(|e| e.title == title)
            .map(|e| e.uuid.clone())
            .or_else(|| root.children.iter().find_map(|g| find_entry(g, title)))
    }
    find_entry(&state.root, title).unwrap()
}

fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-search.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, PASSWORD, "Aes", "Aes256", "None", None)
        .unwrap();
    let parent = add_group(&mut session, ROOT_GROUP_UUID, "Synthetic parent");
    let child = add_group(&mut session, &parent, "Synthetic child");
    let entry = add_entry(&mut session, &child, "Synthetic child entry");
    Fixture {
        _dir: dir,
        session,
        path,
        parent,
        child,
        entry,
    }
}

fn set_flag(session: &mut VaultSession, uuid: &str, value: Option<bool>) {
    {
        let db = session.require_db_mut().unwrap();
        let mut group = if uuid == ROOT_GROUP_UUID {
            db.root_mut()
        } else {
            db.group_mut(helpers::parse_group_id(uuid).unwrap())
                .unwrap()
        };
        group.enable_searching = value;
    }
    session.mark_dirty();
}

fn assert_flags(root: &VaultGroup, name: &str, raw: Option<bool>, effective: bool) {
    let group = find_group(root, name).unwrap();
    assert_eq!(group.searching_setting, raw, "raw setting for {name}");
    assert_eq!(
        group.enable_searching, effective,
        "effective setting for {name}"
    );
}

#[test]
fn native_group_search_default_and_wire_preserve_inherit() {
    let mut f = fixture();
    let state = f.session.snapshot().unwrap();
    assert_eq!(state.root.searching_setting, None);
    assert!(state.root.enable_searching);
    assert_flags(&state.root, "Synthetic parent", None, true);
    assert_flags(&state.root, "Synthetic child", None, true);
    let child = serde_json::to_value(find_group(&state.root, "Synthetic child").unwrap()).unwrap();
    assert!(child["searchingSetting"].is_null());
    assert_eq!(child["enableSearching"], true);
    assert!(child.get("searching_setting").is_none());
}

#[test]
fn native_group_search_disabled_ancestor_is_resolved_in_child_snapshot() {
    let mut f = fixture();
    set_flag(&mut f.session, &f.parent, Some(false));
    let state = f.session.snapshot().unwrap();
    assert_flags(&state.root, "Synthetic parent", Some(false), false);
    assert_flags(&state.root, "Synthetic child", None, false);
    assert_eq!(
        find_group(&state.root, "Synthetic child")
            .unwrap()
            .entries
            .len(),
        1
    );
}

#[test]
fn native_group_search_explicit_overrides_apply_at_each_level() {
    let mut f = fixture();
    set_flag(&mut f.session, &f.parent, Some(false));
    set_flag(&mut f.session, &f.child, Some(true));
    let grandchild = add_group(&mut f.session, &f.child, "Synthetic grandchild");
    let leaf = add_group(&mut f.session, &grandchild, "Synthetic leaf");
    let state = f.session.snapshot().unwrap();
    assert_flags(&state.root, "Synthetic child", Some(true), true);
    assert_flags(&state.root, "Synthetic grandchild", None, true);
    set_flag(&mut f.session, &grandchild, Some(false));
    let state = f.session.snapshot().unwrap();
    assert_flags(&state.root, "Synthetic grandchild", Some(false), false);
    assert_flags(&state.root, "Synthetic leaf", None, false);
    set_flag(&mut f.session, &leaf, Some(true));
    assert_flags(
        &f.session.snapshot().unwrap().root,
        "Synthetic leaf",
        Some(true),
        true,
    );
}

#[test]
fn native_group_search_save_reopen_retains_raw_and_effective_values() {
    let mut f = fixture();
    set_flag(&mut f.session, ROOT_GROUP_UUID, Some(false));
    set_flag(&mut f.session, &f.parent, Some(true));
    let leaf = add_group(&mut f.session, &f.child, "Synthetic leaf");
    set_flag(&mut f.session, &leaf, Some(false));
    f.session.save().unwrap();
    f.session.close();
    let state = f.session.open(&f.path, PASSWORD, None).unwrap();
    assert_eq!(state.root.searching_setting, Some(false));
    assert!(!state.root.enable_searching);
    assert_flags(&state.root, "Synthetic parent", Some(true), true);
    assert_flags(&state.root, "Synthetic child", None, true);
    assert_flags(&state.root, "Synthetic leaf", Some(false), false);
}

#[test]
fn native_group_search_metadata_edit_preserves_unset_flag() {
    let mut f = fixture();
    set_flag(&mut f.session, &f.parent, Some(false));
    let state = f
        .session
        .update_group_meta(&f.child, Some("Synthetic note".into()), None, None)
        .unwrap();
    assert_flags(&state.root, "Synthetic child", None, false);
    let state = f
        .session
        .update_group_meta(&f.parent, None, None, Some(true))
        .unwrap();
    assert_flags(&state.root, "Synthetic child", None, true);
    f.session.save().unwrap();
    f.session.close();
    let state = f.session.open(&f.path, PASSWORD, None).unwrap();
    assert_flags(&state.root, "Synthetic child", None, true);
    assert_eq!(
        find_group(&state.root, "Synthetic child")
            .unwrap()
            .notes
            .as_deref(),
        Some("Synthetic note")
    );
}

#[test]
fn native_group_search_match_and_candidates_honor_explicit_enable() {
    let mut f = fixture();
    set_flag(&mut f.session, &f.parent, Some(false));
    assert!(f
        .session
        .autotype_match("Synthetic child entry window")
        .is_err());
    set_flag(&mut f.session, &f.child, Some(true));
    assert_eq!(
        f.session
            .autotype_match("Synthetic child entry window")
            .unwrap(),
        f.entry
    );
    let matches = f
        .session
        .autotype_match_candidates("Synthetic child entry window")
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].uuid, f.entry);
    set_flag(&mut f.session, &f.child, Some(false));
    assert!(f
        .session
        .autotype_match_candidates("Synthetic child entry window")
        .is_err());
}

#[test]
fn native_group_search_ref_honors_explicit_enable() {
    let mut f = fixture();
    set_flag(&mut f.session, &f.parent, Some(false));
    assert!(f
        .session
        .expand_autotype_sequence("{REF:T@T:Synthetic child entry}")
        .is_err());
    set_flag(&mut f.session, &f.child, Some(true));
    assert_eq!(
        f.session
            .expand_autotype_sequence("{REF:T@T:Synthetic child entry}")
            .unwrap(),
        "Synthetic child entry"
    );
    set_flag(&mut f.session, &f.child, Some(false));
    assert!(f
        .session
        .expand_autotype_sequence("{REF:T@T:Synthetic child entry}")
        .is_err());
}

#[test]
fn native_group_search_explicit_enable_does_not_include_recycle_bin() {
    let mut f = fixture();
    set_flag(&mut f.session, &f.parent, Some(false));
    set_flag(&mut f.session, &f.child, Some(true));
    f.session.require_db_mut().unwrap().meta.recyclebin_uuid =
        Some(helpers::parse_group_id(&f.child).unwrap().uuid());
    f.session.mark_dirty();
    assert!(f
        .session
        .autotype_match("Synthetic child entry window")
        .is_err());
    assert!(f
        .session
        .expand_autotype_sequence("{REF:T@T:Synthetic child entry}")
        .is_err());
    let visible = add_group(&mut f.session, ROOT_GROUP_UUID, "Synthetic visible");
    let id = add_entry(&mut f.session, &visible, "Synthetic visible entry");
    assert_eq!(
        f.session
            .autotype_match("Synthetic visible entry window")
            .unwrap(),
        id
    );
}

#[test]
fn native_group_search_wire_matches_shared_frontend_contract_after_reopen() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/group-search-snapshots.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let mut f = fixture();
        let grandchild = add_group(&mut f.session, &f.child, "Synthetic grandchild");
        add_group(&mut f.session, &grandchild, "Synthetic leaf");
        let rows = case["groups"].as_array().unwrap();
        for row in rows {
            let state = f.session.snapshot().unwrap();
            let uuid = find_group(&state.root, row["name"].as_str().unwrap())
                .unwrap()
                .uuid
                .clone();
            set_flag(&mut f.session, &uuid, row["searchingSetting"].as_bool());
            if row["name"] != "Synthetic child" {
                for title in row["titles"].as_array().unwrap() {
                    add_entry(&mut f.session, &uuid, title.as_str().unwrap());
                }
            }
        }
        f.session.save().unwrap();
        f.session.close();
        let state = f.session.open(&f.path, PASSWORD, None).unwrap();
        // Project the actual serialized tree onto the filter's wire fields.
        // Normalize random UUIDs to names; do not re-resolve searching here.
        fn project(group: &VaultGroup, parent: Option<&str>, out: &mut Vec<serde_json::Value>) {
            let wire = serde_json::to_value(group).unwrap();
            assert!(wire.get("searchingSetting").is_some());
            out.push(serde_json::json!({
                "uuid": wire["name"], "parentUuid": parent, "name": wire["name"],
                "searchingSetting": wire["searchingSetting"],
                "enableSearching": wire["enableSearching"],
                "titles": group.entries.iter().map(|entry| entry.title.as_str()).collect::<Vec<_>>()
            }));
            for child in &group.children {
                project(child, Some(&group.name), out);
            }
        }
        let mut actual = Vec::new();
        project(&state.root, None, &mut actual);
        assert_eq!(actual, *rows, "{}", case["name"]);
    }
}
