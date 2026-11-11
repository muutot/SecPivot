use super::*;
use serde_json::json;

fn request(uuid: Option<&str>) -> EntrySaveRequest {
    serde_json::from_value(json!({
        "target": uuid.map(|uuid| json!({"mode":"edit","uuid":uuid})).unwrap_or(json!({"mode":"create"})),
        "input":{"groupUuid":"root","title":"Synthetic saved","username":"","password":"",
        "url":"","notes":""},"autoType":{"enabled":false},"flags":{"qualityCheck":false}
    })).unwrap()
}

#[test]
fn editor_save_command_resolves_origin_without_active_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let open = |name: &str| {
        vaults
            .open(&mut active.lock().unwrap(), |target| {
                target.create(
                    &dir.path().join(name),
                    "synthetic-master",
                    "Aes",
                    "Aes256",
                    "None",
                    None,
                )
            })
            .unwrap()
            .session_id
    };
    let a = open("a.kdbx");
    let first = save_for_session(&vaults, &active, &a, request(None)).unwrap();
    let b = open("b.kdbx");
    let before_b = serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap();
    let saved = save_for_session(&vaults, &active, &a, request(Some(&first.uuid))).unwrap();
    assert_eq!(saved.uuid, first.uuid);
    assert!(serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap() == before_b);
    let own_b = save_for_session(&vaults, &active, &b, request(None)).unwrap();
    vaults.close(&mut active.lock().unwrap(), Some(&a)).unwrap();
    let before_b = serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap();
    for id in [&a, "", "synthetic-unknown"] {
        assert!(save_for_session(&vaults, &active, id, request(Some(&own_b.uuid))).is_err());
        assert!(save_for_session(&vaults, &active, id, request(None)).is_err());
        assert!(serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap() == before_b);
    }
}
