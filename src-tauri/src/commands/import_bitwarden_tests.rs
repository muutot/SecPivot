use super::*;
fn uri_settings() -> crate::vault::BitwardenUriSettings {
    crate::vault::BitwardenUriSettings {
        default_match: 0,
        equivalent_domains: vec![],
    }
}

fn fixture() -> Vec<u8> {
    include_bytes!("../../../tests/fixtures/bitwarden-native.json").to_vec()
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
fn actual_bitwarden_worker_returns_masked_native_state_and_rejects_extension_override() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let id = open(&vaults, &active, &dir.path().join("target.kdbx"));
    let path = dir.path().join("synthetic.json");
    std::fs::write(&path, fixture()).unwrap();
    let result = tauri::async_runtime::block_on(import_for_session(
        &vaults,
        &active,
        id,
        path.to_string_lossy().into_owned(),
        None,
        None,
        uri_settings(),
    ))
    .unwrap();
    assert_eq!(result.counts.items, 2);
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("synthetic-sensitive-password"));
    assert!(prepare_file(
        &dir.path().join("synthetic.txt"),
        Some("export.json"),
        &uri_settings()
    )
    .is_err());
    assert!(prepare_file(
        Path::new("content://synthetic/document/42"),
        None,
        &uri_settings()
    )
    .is_err());
}
#[test]
fn bitwarden_completion_can_update_parked_origin_but_never_reopened_or_closed_origin() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let path = dir.path().join("a.kdbx");
    let a = open(&vaults, &active, &path);
    let lifetime = active
        .lock()
        .unwrap()
        .native_import_lifetime("root")
        .unwrap();
    let b = open(&vaults, &active, &dir.path().join("b.kdbx"));
    let before = active.lock().unwrap().require_db().unwrap().clone();
    complete(
        &vaults,
        &active,
        &a,
        "root",
        lifetime,
        vault::prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap(),
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
        vault::prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap()
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
        vault::prepare_bitwarden_import(&fixture(), &uri_settings()).unwrap()
    )
    .is_err());
    assert_eq!(active.lock().unwrap().require_db().unwrap(), &before);
    with_vault_session(&vaults, &active, Some(&b), |_| Ok(())).unwrap();
}
