use super::*;

fn fixture() -> Vec<u8> {
    b"<KeePassFile><Meta/><Root><Group><Name>synthetic source</Name><Entry><String><Key>Password</Key><Value ProtectInMemory=\"True\">synthetic-sensitive-marker</Value></String></Entry></Group></Root></KeePassFile>".to_vec()
}
fn open(vaults: &VaultSessions, active: &Mutex<VaultSession>, path: &Path) -> String {
    vaults
        .open(&mut active.lock().unwrap(), |s| {
            s.create(path, "synthetic-master", "Aes", "Aes256", "None", None)
        })
        .unwrap()
        .session_id
}
#[test]
fn actual_xml_worker_returns_masked_native_state_and_rejects_extension_override() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let id = open(&vaults, &active, &dir.path().join("target.kdbx"));
    let path = dir.path().join("synthetic.xml");
    std::fs::write(&path, fixture()).unwrap();
    let result = tauri::async_runtime::block_on(import_for_session(
        &vaults,
        &active,
        id,
        path.to_string_lossy().into_owned(),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(result.counts.items, 1);
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("synthetic-sensitive-marker"));
    assert!(prepare_file(&dir.path().join("synthetic.txt"), Some("export.xml")).is_err());
    assert!(prepare_file(Path::new("content://synthetic/document/42"), None).is_err());
}
#[test]
fn xml_completion_can_update_parked_origin_but_never_reopened_or_closed_origin() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let path = dir.path().join("a.kdbx");
    let a = open(&vaults, &active, &path);
    let lifetime = active.lock().unwrap().xml_import_lifetime("root").unwrap();
    let b = open(&vaults, &active, &dir.path().join("b.kdbx"));
    let before = active.lock().unwrap().require_db().unwrap().clone();
    complete(
        &vaults,
        &active,
        &a,
        "root",
        lifetime,
        vault::prepare_keepass_xml_import(&fixture()).unwrap(),
    )
    .unwrap();
    assert_eq!(active.lock().unwrap().require_db().unwrap(), &before);
    with_vault_session(&vaults, &active, Some(&a), |s| {
        s.close();
        Ok(())
    })
    .unwrap();
    assert!(complete(
        &vaults,
        &active,
        &a,
        "root",
        lifetime,
        vault::prepare_keepass_xml_import(&fixture()).unwrap()
    )
    .is_err());
    with_vault_session(&vaults, &active, Some(&a), |s| {
        s.open(&path, "synthetic-master", None)
    })
    .unwrap();
    assert!(complete(
        &vaults,
        &active,
        &a,
        "root",
        lifetime,
        vault::prepare_keepass_xml_import(&fixture()).unwrap()
    )
    .is_err());
    assert_eq!(active.lock().unwrap().require_db().unwrap(), &before);
    with_vault_session(&vaults, &active, Some(&b), |_| Ok(())).unwrap();
}
