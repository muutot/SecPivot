use super::*;
use keepass::{config::KdfConfig, DatabaseKey};
use tempfile::TempDir;

fn fixture(dir: &TempDir) -> (VaultSession, PathBuf) {
    let path = dir.path().join("synthetic-kdf.kdbx");
    let mut db = Database::new();
    db.config.kdf_config = KdfConfig::Aes { rounds: 1 };
    db.save(
        &mut std::fs::File::create(&path).unwrap(),
        DatabaseKey::new().with_password("synthetic-only"),
    )
    .unwrap();
    let mut session = VaultSession::default();
    session.open(&path, "synthetic-only", None).unwrap();
    (session, path)
}

#[test]
fn measured_kdf_suggestions_apply_and_reopen_for_all_algorithms() {
    for params in [
        KdfParameters::Aes {
            rounds: "9999999999999999".into(),
        },
        KdfParameters::Argon2 {
            iterations: "3".into(),
            memory_bytes: "1048577".into(),
            parallelism: 1,
            version: 16,
        },
        KdfParameters::Argon2id {
            iterations: "3".into(),
            memory_bytes: "1048576".into(),
            parallelism: 1,
            version: 19,
        },
    ] {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = fixture(&dir);
        let before = session.database_settings().unwrap().unwrap().kdf_parameters;
        let result = benchmark_kdf(&params, 100).unwrap();
        assert!(result.elapsed_ms > 0.0 && result.elapsed_ms.is_finite());
        assert!((1..=8).contains(&result.samples));
        assert_eq!(
            session.database_settings().unwrap().unwrap().kdf_parameters,
            before,
            "benchmark must not mutate the session"
        );
        let wire = serde_json::json!({ "kdfParameters": result.parameters });
        let patch: DatabaseSettingsPatch = serde_json::from_value(wire).unwrap();
        assert!(patch.changes_storage());
        let state = session.update_database_settings(&patch).unwrap();
        assert!(!state.dirty);
        let mut reopened = VaultSession::default();
        reopened.open(&path, "synthetic-only", None).unwrap();
        assert_eq!(
            reopened
                .database_settings()
                .unwrap()
                .unwrap()
                .kdf_parameters,
            result.parameters
        );
    }
}

#[test]
fn parameter_rewrite_completion_retains_concurrent_metadata_and_uses_new_costs() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = fixture(&dir);
    let params = KdfParameters::Argon2id {
        iterations: "2".into(),
        memory_bytes: "65537".into(),
        parallelism: 1,
        version: 19,
    };
    let patch = DatabaseSettingsPatch {
        kdf_parameters: Some(params.clone()),
        ..Default::default()
    };
    let job = session.prepare_database_settings_update(&patch).unwrap();
    let revision = job.revision;
    let (db, outcome) = persist_save_with_db(job).unwrap();
    session
        .update_database_settings(&DatabaseSettingsPatch {
            history_max_items: Some(Some(7)),
            ..Default::default()
        })
        .unwrap();
    assert!(
        session
            .complete_database_settings_update(&patch, revision, db, outcome)
            .unwrap()
            .dirty
    );
    assert_eq!(
        session.database_settings().unwrap().unwrap().kdf_parameters,
        params
    );
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    reopened.open(&path, "synthetic-only", None).unwrap();
    let settings = reopened.database_settings().unwrap().unwrap();
    assert_eq!(settings.kdf_parameters, params);
    assert_eq!(settings.history_max_items, Some(7));
}

#[test]
fn invalid_or_mismatched_parameter_patches_never_change_session_or_file() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = fixture(&dir);
    let bytes = std::fs::read(&path).unwrap();
    let before = session.require_db().unwrap().clone();
    for patch in [
        DatabaseSettingsPatch {
            kdf: Some("Argon2id".into()),
            kdf_parameters: Some(KdfParameters::Aes {
                rounds: "100".into(),
            }),
            ..Default::default()
        },
        DatabaseSettingsPatch {
            kdf_parameters: Some(KdfParameters::Argon2id {
                iterations: "4294967296".into(),
                memory_bytes: "65536".into(),
                parallelism: 1,
                version: 19,
            }),
            ..Default::default()
        },
    ] {
        assert!(session.update_database_settings(&patch).is_err());
        assert_eq!(session.require_db().unwrap(), &before);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
