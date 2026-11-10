use super::*;

const FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/onepux/stored.1pux");

fn open(vaults: &VaultSessions, active: &Mutex<VaultSession>, path: &Path) -> String {
    vaults
        .open(&mut active.lock().unwrap(), |session| {
            session.create(path, "synthetic-master", "Aes", "Aes256", "None", None)
        })
        .unwrap()
        .session_id
}

#[test]
fn file_type_uses_native_local_extension_or_provider_display_name() {
    let uri = Path::new("content://synthetic/document/42");
    assert!(validate_path(uri, Some("Export.1PUX")).is_ok());
    assert!(validate_path(uri, None).is_err());
    assert!(validate_path(uri, Some("Export.json")).is_err());
    assert!(validate_path(Path::new("synthetic.txt"), Some("Export.1pux")).is_err());
    assert!(validate_path(Path::new("Export.1PUX"), None).is_ok());
}

#[test]
fn native_file_preparation_rejects_wrong_format_and_reads_only_synthetic_data() {
    let dir = tempfile::tempdir().unwrap();
    let valid = dir.path().join("export.1pux");
    std::fs::write(&valid, FIXTURE).unwrap();
    let plan = prepare_file(&valid, None).unwrap();
    assert_eq!(plan.counts().items, 0);
    assert_eq!(plan.counts().unreferenced_files, 3);
    let invalid = dir.path().join("invalid.1pux");
    std::fs::write(&invalid, b"synthetic-invalid-marker").unwrap();
    let error = prepare_file(&invalid, None).err().unwrap();
    assert!(!error.contains("synthetic-invalid-marker"));
    assert!(prepare_file(&dir.path().join("missing.json"), None)
        .err()
        .unwrap()
        .contains("仅支持"));
}

#[test]
fn actual_worker_path_returns_masked_state_counts_and_one_revision() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let id = open(&vaults, &active, &dir.path().join("target.kdbx"));
    let input = dir.path().join("export.1pux");
    std::fs::write(&input, FIXTURE).unwrap();
    let before = active.lock().unwrap().state().unwrap().unwrap();
    let result = tauri::async_runtime::block_on(import_for_session(
        &vaults,
        &active,
        id,
        input.to_string_lossy().into_owned(),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(result.state.revision, before.revision + 1);
    assert_eq!(result.counts.unreferenced_files, 3);
    let wire: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
    assert!(wire["counts"].get("unreferencedFiles").is_some());
    let fields = wire["state"]["root"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "1Password")
        .unwrap()["entries"][0]["customFields"]
        .as_array()
        .unwrap();
    assert!(fields
        .iter()
        .all(|field| field["protected"] == true && field["value"] == ""));
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("synthetic attachment"));
}

#[test]
fn completion_after_switch_updates_only_its_parked_origin() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let a = open(&vaults, &active, &dir.path().join("a.kdbx"));
    let plan = vault::prepare_1pux_import(FIXTURE).unwrap();
    let b = open(&vaults, &active, &dir.path().join("b.kdbx"));
    let before = active.lock().unwrap().require_db().unwrap().clone();
    let result = complete(&vaults, &active, &a, "root", plan).unwrap();
    assert!(result.state.path.ends_with("a.kdbx"));
    assert_eq!(active.lock().unwrap().require_db().unwrap(), &before);
    with_vault_session(&vaults, &active, Some(&b), |session| {
        assert!(session
            .state()?
            .unwrap()
            .root
            .children
            .iter()
            .all(|g| g.name != "1Password"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn closed_origin_cannot_read_or_redirect_late_completion_to_another_vault() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let a = open(&vaults, &active, &dir.path().join("a.kdbx"));
    let plan = vault::prepare_1pux_import(FIXTURE).unwrap();
    open(&vaults, &active, &dir.path().join("b.kdbx"));
    vaults.close(&mut active.lock().unwrap(), Some(&a)).unwrap();
    let before = active.lock().unwrap().require_db().unwrap().clone();
    assert!(complete(&vaults, &active, &a, "root", plan).is_err());
    // The actual orchestration rejects the closed origin before even trying a
    // missing input path. Its error must come from session admission, not I/O.
    let result = tauri::async_runtime::block_on(import_for_session(
        &vaults,
        &active,
        a,
        dir.path()
            .join("missing.1pux")
            .to_string_lossy()
            .into_owned(),
        None,
        None,
    ));
    let error = result.err().unwrap();
    assert!(!error.contains("无法读取 1PUX"));
    assert_eq!(active.lock().unwrap().require_db().unwrap(), &before);
}
