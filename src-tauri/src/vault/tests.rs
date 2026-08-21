use super::helpers::{save_database, wipe_secret_bytes, wipe_secret_string};
use crate::crypto::otp;
use crate::util::url_host;
use crate::vault::helpers::otp_kind_name;
use keepass::config::OuterCipherConfig;
use keepass::db::Icon;
use keepass::DatabaseKey;

fn compute_totp_at(seed: &str, unix_time: u64) -> Result<TotpCode, String> {
    let spec = otp::parse_totp_seed(seed)?;
    let code = otp::compute(&spec, unix_time)?;
    Ok(TotpCode {
        code: code.code,
        kind: otp_kind_name(code.kind).to_owned(),
        valid_for: code.valid_for,
        period: code.period,
        counter: code.counter,
    })
}
use super::serialize::{icon_to_data_url, parse_expiry};
use super::*;
use crate::bridge::BridgeHost;
use crate::rpc::{RpcError, RpcHost, RpcLoginWrite};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use tempfile::TempDir;

fn create_session(dir: &TempDir) -> (VaultSession, std::path::PathBuf) {
    let path = dir.path().join("test.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "master-password", "Aes", "Aes256", "None", None)
        .unwrap();
    (session, path)
}

/// KeePass "Download Favicons": jobs are grouped by URL host, fetched
/// bytes land in the database as custom icons on every entry of that
/// host, and survive a save + reopen round-trip.
#[test]
fn apply_favicons_persists_custom_icon_across_reopen() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    for (title, url) in [
        ("Login", "https://example.com/login"),
        ("Other", "https://example.com/other"),
    ] {
        session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: title.into(),
                username: "u".into(),
                password: "p".into(),
                url: url.into(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            })
            .unwrap();
    }
    let jobs = session.favicon_jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].host, "example.com");
    assert_eq!(jobs[0].entry_uuids.len(), 2);

    let bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    session
        .apply_favicons(
            &jobs,
            vec![FaviconFetch {
                host: "example.com".into(),
                bytes: bytes.clone(),
            }],
        )
        .unwrap();
    session.save().unwrap();
    drop(session);

    let mut session = VaultSession::default();
    session.open(&path, "master-password", None).unwrap();
    let db = session.require_db().unwrap();
    let mut icon_datas = Vec::new();
    for entry in db.root().entries() {
        match entry.icon().cloned() {
            Some(Icon::Custom(id)) => icon_datas.push(db.custom_icon(id).unwrap().data.clone()),
            _ => panic!("entry should reference a custom icon"),
        }
    }
    assert_eq!(icon_datas, vec![bytes.clone(), bytes.clone()]);
}

/// Applying favicon bytes writes real database content, so the session must
/// be marked dirty: with auto-save off the change is only persisted by an
/// explicit save, and the UI shows the unsaved state until then.
#[test]
fn apply_favicons_marks_session_dirty_until_saved() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com/login".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .unwrap();
    // The entry was just added; make sure the dirty flag reflects the
    // favicon application, not the add.
    session.save().unwrap();
    assert!(!session.state().unwrap().unwrap().dirty);

    let jobs = session.favicon_jobs().unwrap();
    session
        .apply_favicons(
            &jobs,
            vec![FaviconFetch {
                host: "example.com".into(),
                bytes: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            }],
        )
        .unwrap();
    assert!(session.state().unwrap().unwrap().dirty);

    session.save().unwrap();
    assert!(!session.state().unwrap().unwrap().dirty);
}

/// A no-op download (nothing fetched / nothing matched) must not dirty the
/// session, so manual-save mode never shows a phantom unsaved state.
#[test]
fn apply_favicons_without_changes_keeps_session_clean() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com/login".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .unwrap();
    session.save().unwrap();
    assert!(!session.state().unwrap().unwrap().dirty);

    let jobs = session.favicon_jobs().unwrap();
    // Fetched bytes for a host with no job and bytes identical to nothing:
    // neither case may mark the session dirty.
    session.apply_favicons(&jobs, vec![]).unwrap();
    assert!(!session.state().unwrap().unwrap().dirty);
    session
        .apply_favicons(
            &jobs,
            vec![FaviconFetch {
                host: "other-host.example".into(),
                bytes: vec![1, 2, 3],
            }],
        )
        .unwrap();
    assert!(!session.state().unwrap().unwrap().dirty);
}

/// Full snapshots carry the authoritative custom-icon map (including a
/// non-empty one), while mutation results omit it so favorites/expansion/CRUD
/// no longer re-transmit every favicon over IPC.
#[test]
fn light_mutation_snapshots_omit_custom_icons() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com/login".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .unwrap();
    let jobs = session.favicon_jobs().unwrap();
    session
        .apply_favicons(
            &jobs,
            vec![FaviconFetch {
                host: "example.com".into(),
                bytes: vec![1, 2, 3],
            }],
        )
        .unwrap();
    let full = session.state().unwrap().unwrap();
    assert!(full
        .custom_icons
        .as_ref()
        .is_some_and(|icons| !icons.is_empty()));
    let uuid = full.root.entries.last().unwrap().uuid.clone();
    let mutated = session.toggle_favorite(&uuid).unwrap();
    assert!(mutated.custom_icons.is_none());
    assert!(mutated.revision > full.revision);
}

/// Favorite toggling returns only the delta (revision + affected entry) and
/// never rebuilds/serializes the tree; the next full snapshot reflects it.
#[test]
fn favorite_delta_skips_tree_rebuild() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com/login".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .unwrap();
    let uuid = state.root.entries.last().unwrap().uuid.clone();
    let before = state.revision;
    let delta = session.toggle_favorite_delta(&uuid).unwrap();
    match delta {
        MutationDelta::Favorite {
            revision,
            uuid: delta_uuid,
            favorite,
        } => {
            assert_eq!(delta_uuid, uuid);
            assert!(favorite);
            assert!(revision > before);
        }
        other => panic!("expected favorite delta, got {other:?}"),
    }
    let state = session.state().unwrap().unwrap();
    assert!(state.root.entries.last().unwrap().favorite);
}

/// Group expansion returns a uuid→expanded map delta; unknown uuids abort
/// atomically without marking the vault dirty.
#[test]
fn group_expand_delta_maps_uuids_and_bumps_revision() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Mail".into(),
            icon: None,
        })
        .unwrap();
    let mail_uuid = state.root.children[0].uuid.clone();
    let before = state.revision;
    let delta = session
        .set_groups_expanded_delta(std::slice::from_ref(&mail_uuid), false)
        .unwrap();
    match delta {
        MutationDelta::GroupsExpanded { revision, groups } => {
            assert!(revision > before);
            assert_eq!(groups.get(&mail_uuid), Some(&false));
        }
        other => panic!("expected groups-expanded delta, got {other:?}"),
    }
    let state_after_ok = session.state().unwrap().unwrap();
    assert!(!state_after_ok.root.children[0].is_expanded);
    let revision_after_ok = state_after_ok.revision;

    let unknown = "00000000-0000-0000-0000-000000000000".to_owned();
    assert!(session
        .set_groups_expanded_delta(&[mail_uuid, unknown], true)
        .is_err());
    let state_after_fail = session.state().unwrap().unwrap();
    assert_eq!(state_after_fail.revision, revision_after_ok);
    assert_eq!(state_after_fail.dirty, state_after_ok.dirty);
}

/// Entry Auto-Type config (enabled/default sequence/window associations)
/// round-trips through save + reopen.
#[test]
fn entry_autotype_round_trips_save_and_reopen() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com/login".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .unwrap();
    let uuid = state.root.entries.last().unwrap().uuid.clone();
    session
        .update_entry_autotype(
            &uuid,
            &EntryAutoTypeInput {
                enabled: true,
                default_sequence: Some("{USERNAME}{ENTER}{PASSWORD}{ENTER}".into()),
                associations: vec![AutoTypeAssociationDto {
                    window: "GitHub*".into(),
                    sequence: "{TAB}{PASSWORD}{ENTER}".into(),
                }],
            },
        )
        .unwrap();
    let state = session.state().unwrap().unwrap();
    let autotype = state
        .root
        .entries
        .last()
        .unwrap()
        .autotype
        .as_ref()
        .unwrap();
    assert!(autotype.enabled);
    assert_eq!(
        autotype.default_sequence.as_deref(),
        Some("{USERNAME}{ENTER}{PASSWORD}{ENTER}")
    );
    assert_eq!(autotype.associations.len(), 1);
    assert_eq!(autotype.associations[0].window, "GitHub*");

    session.save().unwrap();
    drop(session);
    let mut session = VaultSession::default();
    session.open(&path, "master-password", None).unwrap();
    let state = session.state().unwrap().unwrap();
    let autotype = state
        .root
        .entries
        .last()
        .unwrap()
        .autotype
        .as_ref()
        .unwrap();
    assert_eq!(
        autotype.default_sequence.as_deref(),
        Some("{USERNAME}{ENTER}{PASSWORD}{ENTER}")
    );
    assert_eq!(autotype.associations[0].sequence, "{TAB}{PASSWORD}{ENTER}");
}

/// Group Auto-Type settings persist and gate descendants via the existing
/// inheritance resolution.
#[test]
fn group_autotype_round_trip_and_inheritance() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Mail".into(),
            icon: None,
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    session
        .update_group_autotype(
            &group_uuid,
            &GroupAutoTypeInput {
                enabled: Some(false),
                default_sequence: Some("{USERNAME}{TAB}{PASSWORD}{ENTER}".into()),
            },
        )
        .unwrap();
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_uuid.clone(),
            title: "Mail Login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://mail.example.com".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .unwrap();
    let entry_uuid = state.root.children[0].entries.last().unwrap().uuid.clone();
    assert_eq!(
        session.resolve_autotype_sequence(&entry_uuid).unwrap(),
        None,
        "group-disabled AutoType must disable descendants"
    );
    session
        .update_group_autotype(
            &group_uuid,
            &GroupAutoTypeInput {
                enabled: Some(true),
                default_sequence: None,
            },
        )
        .unwrap();
    assert_eq!(
        session
            .resolve_autotype_sequence(&entry_uuid)
            .unwrap()
            .as_deref(),
        Some("{USERNAME}{TAB}{PASSWORD}{ENTER}")
    );
}

#[test]
fn group_meta_notes_tags_search_round_trip() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Work".into(),
            icon: None,
        })
        .unwrap();
    let uuid = state.root.children[0].uuid.clone();

    // Set notes/tags and exclude the group from search.
    let updated = session
        .update_group_meta(
            &uuid,
            Some("shared vault".into()),
            Some("dev, web".into()),
            Some(false),
        )
        .unwrap();
    let group = updated
        .root
        .children
        .iter()
        .find(|g| g.uuid == uuid)
        .unwrap();
    assert_eq!(group.notes.as_deref(), Some("shared vault"));
    assert_eq!(group.tags.as_deref(), Some("dev, web"));
    assert!(!group.enable_searching);

    // Clear notes/tags and re-enable search; absent fields keep values.
    let updated = session
        .update_group_meta(&uuid, Some(String::new()), Some(String::new()), Some(true))
        .unwrap();
    let group = updated
        .root
        .children
        .iter()
        .find(|g| g.uuid == uuid)
        .unwrap();
    assert!(group.notes.is_none());
    assert!(group.tags.is_none());
    assert!(group.enable_searching);

    // The root group accepts meta too.
    let updated = session
        .update_group_meta(ROOT_GROUP_UUID, Some("root note".into()), None, None)
        .unwrap();
    assert_eq!(updated.root.notes.as_deref(), Some("root note"));

    // Save + reopen keeps the group meta.
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    let group = state.root.children.iter().find(|g| g.uuid == uuid).unwrap();
    assert!(group.notes.is_none());
    assert!(group.tags.is_none());
    assert!(group.enable_searching);
    assert_eq!(state.root.notes.as_deref(), Some("root note"));
}

/// Global-hotkey resolution picks the first matching window association
/// before falling back to the entry/group default sequence.
#[test]
fn window_association_picks_sequence() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .unwrap();
    let uuid = state.root.entries.last().unwrap().uuid.clone();
    session
        .update_entry_autotype(
            &uuid,
            &EntryAutoTypeInput {
                enabled: true,
                default_sequence: Some("{USERNAME}{ENTER}".into()),
                associations: vec![
                    AutoTypeAssociationDto {
                        window: "GitHub*".into(),
                        sequence: "{TAB}{PASSWORD}{ENTER}".into(),
                    },
                    AutoTypeAssociationDto {
                        window: "*Mail*".into(),
                        sequence: "{PASSWORD}{ENTER}".into(),
                    },
                ],
            },
        )
        .unwrap();

    assert!(VaultSession::window_title_matches(
        "GitHub*",
        "GitHub - Home"
    ));
    assert!(VaultSession::window_title_matches(
        "*Mail*",
        "Inbox - Mail Client"
    ));
    assert!(!VaultSession::window_title_matches(
        "GitHub*",
        "Mail Client"
    ));

    assert_eq!(
        session
            .resolve_autotype_sequence_for_window(&uuid, "GitHub - Home")
            .unwrap()
            .as_deref(),
        Some("{TAB}{PASSWORD}{ENTER}")
    );
    assert_eq!(
        session
            .resolve_autotype_sequence_for_window(&uuid, "Mail Client")
            .unwrap()
            .as_deref(),
        Some("{PASSWORD}{ENTER}")
    );
    assert_eq!(
        session
            .resolve_autotype_sequence_for_window(&uuid, "Other Window")
            .unwrap()
            .as_deref(),
        Some("{USERNAME}{ENTER}")
    );
}

/// Global-hotkey multi-match returns every scoring entry (recycle-bin
/// entries excluded), best first.
#[test]
fn autotype_match_candidates_returns_sorted_and_skips_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    for (title, url) in [
        ("GitHub", "https://github.com"),
        ("GitHub", "https://github.com"),
        ("Mail", "https://mail.example.com"),
    ] {
        session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: title.into(),
                username: "u".into(),
                password: "p".into(),
                url: url.into(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            })
            .unwrap();
    }
    // A matching entry inside the recycle bin must never be offered.
    let state = session.state().unwrap().unwrap();
    let entry_uuid = state.root.entries[1].uuid.clone();
    session.delete_entry(&entry_uuid).unwrap();

    let candidates = session.autotype_match_candidates("GitHub - Home").unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title, "GitHub");
    assert!(!candidates.iter().any(|c| c.uuid == entry_uuid));
}

/// Database settings read the current KDF/cipher/compression/history/recycle
/// flags, including the closed-session `None` case.
#[test]
fn database_settings_reports_current_config() {
    assert!(VaultSession::default()
        .database_settings()
        .unwrap()
        .is_none());

    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.kdf, "Aes");
    assert_eq!(settings.cipher, "Aes256");
    assert_eq!(settings.compression, "None");
    assert!(settings.recycle_bin_enabled);
    assert_eq!(settings.history_max_items, None);

    {
        let db = session.require_db_mut().unwrap();
        db.meta.history_max_items = Some(3);
        db.meta.recyclebin_enabled = Some(false);
    }
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.history_max_items, Some(3));
    assert!(!settings.recycle_bin_enabled);

    session.require_db_mut().unwrap().config.outer_cipher_config = OuterCipherConfig::Twofish;
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.cipher, "Twofish");

    let path = dir.path().join("argon.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "master-password", "Argon2", "ChaCha20", "Gzip", None)
        .unwrap();
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.kdf, "Argon2");
    assert_eq!(settings.cipher, "ChaCha20");
    assert_eq!(settings.compression, "Gzip");
}

/// Existing Twofish databases remain writable when a settings patch omits
/// the cipher, so compatibility does not force an implicit migration.
#[test]
fn existing_twofish_database_survives_meta_update_and_reopen() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    session.require_db_mut().unwrap().config.outer_cipher_config = OuterCipherConfig::Twofish;
    session.mark_dirty();
    session.save().unwrap();

    session
        .update_database_settings(&DatabaseSettingsPatch {
            history_max_items: Some(Some(4)),
            ..Default::default()
        })
        .unwrap();
    session.save().unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    reopened.open(&path, "master-password", None).unwrap();
    let settings = reopened.database_settings().unwrap().unwrap();
    assert_eq!(settings.cipher, "Twofish");
    assert_eq!(settings.history_max_items, Some(4));
}

/// Database-setting writes persist through save/reopen and explicit `null`
/// resets history/recycle flags to KeePass defaults.
#[test]
fn update_database_settings_persists_history_and_recycle_flag() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    session
        .update_database_settings(&DatabaseSettingsPatch {
            history_max_items: Some(Some(5)),
            recycle_bin_enabled: Some(Some(false)),
            ..Default::default()
        })
        .unwrap();
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.history_max_items, Some(5));
    assert!(!settings.recycle_bin_enabled);

    session.save().unwrap();
    drop(session);
    let mut session = VaultSession::default();
    session.open(&path, "master-password", None).unwrap();
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.history_max_items, Some(5));
    assert!(!settings.recycle_bin_enabled);

    session
        .update_database_settings(&DatabaseSettingsPatch {
            history_max_items: Some(None),
            recycle_bin_enabled: Some(None),
            ..Default::default()
        })
        .unwrap();
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.history_max_items, None);
    assert!(settings.recycle_bin_enabled);
}

/// KDF/cipher/compression changes re-encrypt the database with the same
/// master key and survive save + reopen.
#[test]
fn update_database_settings_reencrypts_storage_config() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let before = session.snapshot_without_icons().unwrap();
    let updated = session
        .update_database_settings(&DatabaseSettingsPatch {
            kdf: Some("Argon2".into()),
            cipher: Some(WritableDatabaseCipher::ChaCha20),
            compression: Some("Gzip".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(updated.revision > before.revision);
    assert!(!updated.dirty);
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.kdf, "Argon2");
    assert_eq!(settings.cipher, "ChaCha20");
    assert_eq!(settings.compression, "Gzip");

    drop(session);
    let mut session = VaultSession::default();
    session.open(&path, "master-password", None).unwrap();
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.kdf, "Argon2");
    assert_eq!(settings.cipher, "ChaCha20");
    assert_eq!(settings.compression, "Gzip");
}

#[test]
fn database_settings_reencrypt_completion_preserves_concurrent_edits() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let patch = DatabaseSettingsPatch {
        kdf: Some("Argon2".into()),
        cipher: Some(WritableDatabaseCipher::ChaCha20),
        compression: Some("Gzip".into()),
        history_max_items: Some(Some(5)),
        ..Default::default()
    };

    let job = session.prepare_database_settings_update(&patch).unwrap();
    let revision = job.revision;
    let (persisted_db, new_hash) = persist_save_with_db(job).unwrap();
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "ConcurrentEdit".into(),
            username: "u".into(),
            password: "p".into(),
            url: String::new(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();

    let completed = session
        .complete_database_settings_update(&patch, revision, persisted_db, new_hash)
        .unwrap();
    assert!(completed.dirty);
    assert!(completed
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "ConcurrentEdit"));
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.kdf, "Argon2");
    assert_eq!(settings.cipher, "ChaCha20");
    assert_eq!(settings.compression, "Gzip");
    assert_eq!(settings.history_max_items, Some(5));

    // The persisted snapshot contains the settings rewrite but not the later
    // edit; a normal save writes that retained edit with the new config.
    let mut persisted = VaultSession::default();
    persisted.open(&path, "master-password", None).unwrap();
    assert!(!persisted
        .state()
        .unwrap()
        .unwrap()
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "ConcurrentEdit"));
    drop(persisted);
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    reopened.open(&path, "master-password", None).unwrap();
    assert!(reopened
        .state()
        .unwrap()
        .unwrap()
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "ConcurrentEdit"));
}

/// History-size cap and templates-group UUID persist through save/reopen,
/// `null` clears them, and an invalid UUID is rejected.
#[test]
fn update_database_settings_persists_history_size_and_template_group() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Templates".into(),
            icon: None,
        })
        .unwrap();
    let templates_uuid = state.root.children[0].uuid.clone();
    session
        .update_database_settings(&DatabaseSettingsPatch {
            history_max_size: Some(Some(4096)),
            entry_templates_group: Some(Some(templates_uuid.clone())),
            ..Default::default()
        })
        .unwrap();
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.history_max_size, Some(4096));
    assert_eq!(
        settings.entry_templates_group.as_deref(),
        Some(templates_uuid.as_str())
    );

    session.save().unwrap();
    drop(session);
    let mut session = VaultSession::default();
    session.open(&path, "master-password", None).unwrap();
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.history_max_size, Some(4096));
    assert_eq!(
        settings.entry_templates_group.as_deref(),
        Some(templates_uuid.as_str())
    );

    assert!(session
        .update_database_settings(&DatabaseSettingsPatch {
            entry_templates_group: Some(Some("not-a-uuid".into())),
            ..Default::default()
        })
        .is_err());
    session
        .update_database_settings(&DatabaseSettingsPatch {
            history_max_size: Some(None),
            entry_templates_group: Some(None),
            ..Default::default()
        })
        .unwrap();
    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.history_max_size, None);
    assert_eq!(settings.entry_templates_group, None);
}

/// A content-only edit (icon omitted) must keep the entry's icon — both a
/// built-in icon and a downloaded favicon custom icon — while an explicit
/// `icon: null` still clears it.
#[test]
fn update_without_icon_keeps_existing_icon() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let input = || EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: "Login".into(),
        username: "u".into(),
        password: "p".into(),
        url: "https://example.com/login".into(),
        notes: String::new(),
        totp: None,
        expires: None,
        icon: None,
        color: None,
        tags: None,
        custom_fields: Vec::new(),
        attachments: Vec::new(),
    };
    let state = session.add_entry(&input()).unwrap();
    let uuid = state.root.entries.last().unwrap().uuid.clone();

    // Built-in icon survives a content-only update.
    session
        .update_entry(
            &uuid,
            &EntryInput {
                icon: Some(Some(5)),
                ..input()
            },
        )
        .unwrap();
    let state = session.update_entry(&uuid, &input()).unwrap();
    let entry = state.root.entries.last().unwrap();
    assert_eq!(entry.icon, Some(5));
    assert_eq!(entry.custom_icon, None);

    // A downloaded favicon (custom icon) also survives a content-only update.
    let jobs = session.favicon_jobs().unwrap();
    session
        .apply_favicons(
            &jobs,
            vec![FaviconFetch {
                host: "example.com".into(),
                bytes: vec![0x89, 0x50, 0x4E, 0x47],
            }],
        )
        .unwrap();
    let state = session.update_entry(&uuid, &input()).unwrap();
    let entry = state.root.entries.last().unwrap();
    assert_eq!(entry.icon, None);
    assert!(entry.custom_icon.is_some(), "custom favicon must be kept");

    // An explicit `icon: null` clears both kinds.
    session
        .update_entry(
            &uuid,
            &EntryInput {
                icon: Some(None),
                ..input()
            },
        )
        .unwrap();
    let state = session.update_entry(&uuid, &input()).unwrap();
    let entry = state.root.entries.last().unwrap();
    assert_eq!(entry.icon, None);
    assert_eq!(entry.custom_icon, None);
}

/// Multi-select "Download Favicons": `favicon_jobs_selected` scopes jobs
/// to the given entries only — same-host entries outside the selection
/// never share the icon, and URL-less entries are skipped.
#[test]
fn favicon_jobs_selected_scopes_to_given_entries() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let mut uuids = Vec::new();
    for (title, url) in [
        ("Login", "https://example.com/login"),
        ("Other", "https://example.com/other"),
        ("Elsewhere", "https://elsewhere.test"),
        ("NoUrl", ""),
    ] {
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: title.into(),
                username: "u".into(),
                password: "p".into(),
                url: url.into(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            })
            .unwrap();
        uuids.push(state.root.entries.last().unwrap().uuid.clone());
    }

    let jobs = session.favicon_jobs_selected(&[uuids[1].clone()]).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].host, "example.com");
    assert_eq!(jobs[0].entry_uuids, vec![uuids[1].clone()]);

    let jobs = session
        .favicon_jobs_selected(&[uuids[0].clone(), uuids[1].clone()])
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].entry_uuids.len(), 2);

    let jobs = session.favicon_jobs_selected(&[uuids[3].clone()]).unwrap();
    assert!(jobs.is_empty(), "URL-less entry yields no job");

    let jobs = session
        .favicon_jobs_selected(&[
            "00000000-0000-0000-0000-000000000000".to_owned(),
            uuids[2].clone(),
        ])
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].host, "elsewhere.test");
}

/// The plugin tree served through GetAllDatabases must actually contain
/// the vault's entries — an empty `childLightEntries` made the Kee browser
/// extension show nothing, so adds/edits could not be seen either.
#[test]
fn plugin_tree_includes_root_and_subgroup_entries() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let group_uuid = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap()
        .root
        .children[0]
        .uuid
        .clone();
    session
        .add_entry(&EntryInput {
            group_uuid,
            title: "Webmail".into(),
            username: "alice".into(),
            password: "s3cret".into(),
            url: "https://webmail.example.com".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .expect("group entry added");
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "RootEntry".into(),
            username: "bob".into(),
            password: "pw".into(),
            url: "https://root.example".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .expect("root entry added");

    use crate::rpc::RpcHost;
    let db = session
        .database()
        .expect("open session exposes plugin tree");
    assert!(
        db.root.entries.iter().any(|e| e.title == "RootEntry"),
        "root-level entries must appear in the plugin tree"
    );
    let mail = db
        .root
        .children
        .iter()
        .find(|g| g.title == "Mail")
        .expect("Mail group must appear in the plugin tree");
    assert!(
        mail.entries.iter().any(|e| e.title == "Webmail"),
        "sub-group entries must appear in the plugin tree"
    );
    assert!(
        db.root.entries.iter().all(|e| e.password.is_empty()),
        "plugin tree light entries must never carry credentials"
    );
}

#[test]
fn icon_to_data_url_guesses_media_types() {
    let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
    let url = icon_to_data_url(&png);
    assert!(url.starts_with("data:image/png;base64,"));
    assert_eq!(BASE64.decode(url.split_once(',').unwrap().1).unwrap(), png,);

    assert!(icon_to_data_url(&[0x00, 0x00, 0x01, 0x00, 1]).starts_with("data:image/x-icon;base64,"));
    assert!(icon_to_data_url(&[0xFF, 0xD8, 0xFF]).starts_with("data:image/jpeg;base64,"));
    assert!(icon_to_data_url(b"GIF89a").starts_with("data:image/gif;base64,"));
    assert!(icon_to_data_url(b"BMXXXX").starts_with("data:image/bmp;base64,"));
    assert!(
        icon_to_data_url(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>")
            .starts_with("data:image/svg+xml;base64,")
    );
    let unknown = b"binary payload";
    assert!(icon_to_data_url(unknown).starts_with("data:image/png;base64,"));
}

/// Write a KeePass-style binary keyfile and return its path.
fn write_keyfile(dir: &TempDir) -> std::path::PathBuf {
    let keyfile = dir.path().join("test.key");
    let mut bytes = Vec::new();
    for i in 0..64u8 {
        bytes.push(i.wrapping_mul(7).wrapping_add(3));
    }
    std::fs::write(&keyfile, bytes).unwrap();
    keyfile
}

#[test]
fn create_then_reopen_round_trip() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session.state().unwrap().unwrap();
    assert_eq!(state.root.name, "Root");
    assert_eq!(state.root.uuid, ROOT_GROUP_UUID);
    assert_eq!(state.file_name, "test.kdbx");
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert_eq!(state.root.children.len(), 0);
    assert!(reopened.is_open());
}

#[test]
fn wrong_password_is_rejected() {
    let dir = TempDir::new().unwrap();
    let (session, path) = create_session(&dir);
    drop(session);
    let mut reopened = VaultSession::default();
    let err = reopened.open(&path, "wrong", None).unwrap_err();
    assert!(err.contains("密码"), "unexpected error: {err}");
}

#[test]
fn empty_password_is_rejected() {
    let dir = TempDir::new().unwrap();
    let (_session, path) = create_session(&dir);
    let err = VaultSession::default().open(&path, "", None).unwrap_err();
    assert!(err.contains("主密码"));
    assert!(!VaultSession::default().is_open());
}

#[test]
fn group_and_entry_crud_flow() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "  Web  ".into(),
        })
        .unwrap();
    let group = &state.root.children[0];
    assert_eq!(group.name, "Web");
    assert_eq!(group.parent_uuid.as_deref(), Some(ROOT_GROUP_UUID));

    let state = session
        .add_entry(&EntryInput {
            group_uuid: group.uuid.clone(),
            title: "GitHub".into(),
            username: "alice".into(),
            password: "s3cret".into(),
            url: "https://github.com".into(),
            notes: "work".into(),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let group = &state.root.children[0];
    assert_eq!(group.entries.len(), 1);
    let entry = &group.entries[0];
    assert_eq!(entry.title, "GitHub");
    assert_eq!(session.get_entry_password(&entry.uuid).unwrap(), "s3cret");
    assert!(entry.has_totp);
    assert_eq!(
        session.get_entry_totp(&entry.uuid).unwrap().as_deref(),
        Some("JBSWY3DPEHPK3PXP")
    );
    let entry_uuid = entry.uuid.clone();
    assert!(entry.created.is_some());
    assert!(entry.modified.is_some());
    assert!(state.dirty);

    let state = session
        .update_entry(
            &entry_uuid,
            &EntryInput {
                group_uuid: group.uuid.clone(),
                title: "GitHub (work)".into(),
                username: "alice".into(),
                password: "s3cret2".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    let entry = &state.root.children[0].entries[0];
    assert_eq!(entry.title, "GitHub (work)");
    assert_eq!(session.get_entry_password(&entry_uuid).unwrap(), "s3cret2");
    assert!(!entry.has_totp);
    assert_eq!(session.get_entry_totp(&entry_uuid).unwrap(), None);

    let state = session.rename_group(&group.uuid, "Accounts").unwrap();
    assert_eq!(state.root.children[0].name, "Accounts");

    let state = session.delete_entry(&entry_uuid).unwrap();
    assert_eq!(state.root.children[0].entries.len(), 0);
}

#[test]
fn entry_history_tracks_versions_and_restores() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "A".into(),
            username: "u".into(),
            password: "p1".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    let input = |title: &str, password: &str, attachments: Vec<AttachmentInput>| EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: title.into(),
        username: "u".into(),
        password: password.into(),
        url: "".into(),
        notes: "".into(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments,
    };

    assert!(session.get_entry_history(&uuid).unwrap().is_empty());
    session
        .update_entry(&uuid, &input("B", "p2", vec![]))
        .unwrap();
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].title, "A");
    // Passwords never leave the backend in history payloads.
    assert_eq!(session.get_entry_password(&uuid).unwrap(), "p2");

    session
        .update_entry(
            &uuid,
            &input(
                "C",
                "p3",
                vec![AttachmentInput {
                    name: "note.txt".into(),
                    data: Some(BASE64.encode(b"hello attachment")),
                }],
            ),
        )
        .unwrap();
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].title, "B");
    assert!(history[0].attachments.is_empty());
    assert_eq!(history[1].title, "A");
    assert!(history[1].attachments.is_empty());
    // The current state carries the attachment, not the history snapshots.
    let current = session.snapshot().unwrap();
    let current_entry = current
        .root
        .entries
        .iter()
        .find(|e| e.uuid == uuid)
        .unwrap();
    assert_eq!(current_entry.attachments.len(), 1);
    assert_eq!(current_entry.attachments[0].name, "note.txt");
    assert_eq!(current_entry.attachments[0].size, b"hello attachment".len());

    // Restoring the snapshot replaces fields and pushes the pre-restore
    // state into the history itself.
    let state = session.restore_entry_version(&uuid, 0).unwrap();
    assert_eq!(state.root.entries[0].title, "B");
    assert_eq!(session.get_entry_password(&uuid).unwrap(), "p2");
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].title, "C");
    assert_eq!(history[0].attachments.len(), 1);
    // Restore overwrites fields only; attachments are left untouched.
    assert_eq!(state.root.entries[0].attachments.len(), 1);

    assert!(session
        .restore_entry_version(&uuid, 99)
        .is_err_and(|err| err.contains("历史版本不存在")));
}

#[test]
fn entry_history_covers_all_snapshot_fields() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    // v1 carries tags, color, a TOTP seed, the favorite marker and custom data.
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "v1".into(),
            username: "u".into(),
            password: "p1".into(),
            url: "https://example.com".into(),
            notes: "n1".into(),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            expires: None,
            icon: Some(Some(3)),
            color: Some("#336699".into()),
            tags: Some("work, 高优".into()),
            custom_fields: vec![CustomField {
                name: "Note".into(),
                value: "x".into(),
                protected: false,
            }],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    // v2 strips the TOTP, tags and color so the snapshots diverge on them.
    session
        .update_entry(
            &uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "v2".into(),
                username: "u".into(),
                password: "p2".into(),
                url: "https://example.com".into(),
                notes: "n1".into(),
                totp: Some("".into()),
                expires: None,
                icon: Some(Some(3)),
                color: None,
                tags: Some("".into()),
                custom_fields: vec![CustomField {
                    name: "Note".into(),
                    value: "y".into(),
                    protected: false,
                }],
                attachments: vec![],
            },
        )
        .unwrap();

    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 1);
    let snapshot = &history[0];
    assert_eq!(snapshot.title, "v1");
    assert!(snapshot.has_totp, "v1 carried a TOTP seed");
    assert_eq!(snapshot.tags.as_deref(), Some("work, 高优"));
    assert_eq!(snapshot.color.as_deref(), Some("#336699"));
    assert_eq!(snapshot.icon, Some(3));
    assert!(!snapshot.favorite);
    assert!(snapshot.quality_check);
    assert!(snapshot.custom_fields.iter().any(|f| f.name == "Note"));
}

#[test]
fn entry_history_diff_reports_backend_authoritative_changes() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    // v1: baseline with a protected custom field whose value never leaves the
    // backend in snapshots — only the backend diff can see its changes.
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "v1".into(),
            username: "u".into(),
            password: "p1".into(),
            url: "https://example.com".into(),
            notes: "n1".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![
                CustomField {
                    name: "Note".into(),
                    value: "x".into(),
                    protected: false,
                },
                CustomField {
                    name: "Secret".into(),
                    value: "top".into(),
                    protected: true,
                },
            ],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    // v2 changes title + password, edits Note, removes Secret, adds Added.
    session
        .update_entry(
            &uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "v2".into(),
                username: "u".into(),
                password: "p2".into(),
                url: "https://example.com".into(),
                notes: "n1".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![
                    CustomField {
                        name: "Note".into(),
                        value: "y".into(),
                        protected: false,
                    },
                    CustomField {
                        name: "Added".into(),
                        value: "new".into(),
                        protected: false,
                    },
                ],
                attachments: vec![],
            },
        )
        .unwrap();

    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 1);
    let diff = &history[0].diff;
    assert!(diff.title);
    // The password change is detected even though passwords are never
    // serialized into history payloads.
    assert!(diff.password);
    assert!(!diff.username);
    assert!(!diff.url);
    assert!(!diff.notes);
    assert!(!diff.expires);
    assert!(!diff.has_totp);
    assert!(!diff.icon);
    assert!(!diff.color);
    assert!(!diff.tags);
    assert!(!diff.favorite);
    assert!(!diff.quality_check);

    let change_of = |name: &str| {
        diff.custom_fields
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.change.as_str())
    };
    assert_eq!(change_of("Note"), Some("modified"));
    assert_eq!(change_of("Secret"), Some("removed"));
    assert_eq!(change_of("Added"), Some("added"));
    assert!(diff.custom_data.is_empty());
    assert!(diff.attachments.is_empty());
}

#[test]
fn update_custom_field_edits_one_field_and_keeps_protection() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "acct".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![
                CustomField {
                    name: "API Key".into(),
                    value: "plain".into(),
                    protected: false,
                },
                CustomField {
                    name: "Secret".into(),
                    value: "top".into(),
                    protected: true,
                },
            ],
            attachments: Vec::new(),
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    // Edit the unprotected field; flag stays false, other fields untouched.
    let updated = session
        .update_custom_field(&uuid, "API Key", "plain-v2", false)
        .unwrap();
    let entry = &updated.root.entries[0];
    let api = entry
        .custom_fields
        .iter()
        .find(|f| f.name == "API Key")
        .unwrap();
    assert_eq!(api.value, "plain-v2");
    assert!(!api.protected);
    let secret = entry
        .custom_fields
        .iter()
        .find(|f| f.name == "Secret")
        .unwrap();
    // Protected values are redacted in snapshots; the flag must survive.
    assert_eq!(secret.value, "");
    assert!(secret.protected);
    assert_eq!(entry.username, "u");

    // Edit the protected field; flag stays true.
    session
        .update_custom_field(&uuid, "Secret", "top-v2", true)
        .unwrap();
    let fetched_secret = session.get_custom_field_value(&uuid, "Secret").unwrap();
    assert_eq!(fetched_secret.as_deref(), Some("top-v2"));

    // Reserved/standard names are rejected.
    assert!(session
        .update_custom_field(&uuid, "UserName", "x", false)
        .is_err());
    assert!(session
        .update_custom_field(&uuid, "  ", "x", false)
        .is_err());

    // History captured the two custom-field edits.
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 2);

    // Save + reopen keeps values and the protection flag.
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    let entry = state.root.entries.iter().find(|e| e.uuid == uuid).unwrap();
    let api = entry
        .custom_fields
        .iter()
        .find(|f| f.name == "API Key")
        .unwrap();
    assert_eq!(api.value, "plain-v2");
    assert!(!api.protected);
    let secret = entry
        .custom_fields
        .iter()
        .find(|f| f.name == "Secret")
        .unwrap();
    assert!(secret.protected);
    assert_eq!(
        reopened
            .get_custom_field_value(&uuid, "Secret")
            .unwrap()
            .as_deref(),
        Some("top-v2")
    );
}

#[test]
fn entry_history_supports_manual_delete() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "v1".into(),
            username: "u".into(),
            password: "p1".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    let input = |title: &str, password: &str| EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: title.into(),
        username: "u".into(),
        password: password.into(),
        url: "".into(),
        notes: "".into(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    };

    // Two updates create two snapshots: [v1, v2] (newest first).
    session.update_entry(&uuid, &input("v2", "p2")).unwrap();
    session.update_entry(&uuid, &input("v3", "p3")).unwrap();
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].title, "v2");
    assert_eq!(history[1].title, "v1");

    // Deleting the newest snapshot leaves only the older one, reordered.
    let state = session.delete_entry_history(&uuid, 0).unwrap();
    assert_eq!(state.root.entries[0].title, "v3");
    assert_eq!(session.get_entry_password(&uuid).unwrap(), "p3");
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].title, "v1");

    // Deleting the remaining snapshot empties the history.
    session.delete_entry_history(&uuid, 0).unwrap();
    assert!(session.get_entry_history(&uuid).unwrap().is_empty());

    // Out-of-range and nonexistent indices error out.
    session.update_entry(&uuid, &input("v4", "p4")).unwrap();
    assert!(session
        .delete_entry_history(&uuid, 99)
        .is_err_and(|err| err.contains("历史版本不存在")));
    assert!(session
        .delete_entry_history(&uuid, 99)
        .is_err_and(|err| err.contains("历史版本不存在")));
    let _ = session.add_entry(&EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: "fresh".into(),
        username: "".into(),
        password: "p".into(),
        url: "".into(),
        notes: "".into(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    });
    let fresh_uuid = session.snapshot().unwrap().root.entries[1].uuid.clone();
    assert!(session
        .delete_entry_history(&fresh_uuid, 0)
        .is_err_and(|err| err.contains("历史版本不存在")));
}

#[test]
fn entry_storage_counts_fields_attachments_and_history() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "storage".into(),
            username: "user".into(),
            password: "pass".into(),
            url: "https://example.com".into(),
            notes: "notes".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![CustomField {
                name: "custom".into(),
                value: "value".into(),
                protected: false,
            }],
            attachments: vec![AttachmentInput {
                name: "payload.bin".into(),
                data: Some(BASE64.encode(vec![0u8; 128])),
            }],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    let storage = session.get_entry_storage(&uuid).unwrap();
    // fields: title+username+password+url+notes+custom (+ "custom" key, not
    // counted) + empty-key overhead; assert exact field bytes.
    assert!(storage.fields >= "storage".len() + "user".len() + "pass".len());
    assert_eq!(storage.attachments, 128);
    assert_eq!(storage.history, 0);
    assert_eq!(
        storage.total,
        storage.fields + storage.attachments + storage.history
    );

    // A second update adds one historical snapshot; its fields count toward
    // history while the current attachment stays.
    session
        .update_entry(
            &uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "storage2".into(),
                username: "user".into(),
                password: "pass2".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    let storage = session.get_entry_storage(&uuid).unwrap();
    assert!(storage.history > 0);
    assert_eq!(storage.attachments, 0);
    assert_eq!(
        storage.total,
        storage.fields + storage.attachments + storage.history
    );
}

#[test]
fn entry_snapshot_size_matches_storage_total() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "sized".into(),
            username: "user".into(),
            password: "pass".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![AttachmentInput {
                name: "payload.bin".into(),
                data: Some(BASE64.encode(vec![0u8; 64])),
            }],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    // The snapshot column value equals the on-demand storage breakdown.
    let entry = &session.snapshot().unwrap().root.entries[0];
    assert_eq!(entry.size as usize, session.get_entry_storage(&uuid).unwrap().total);
    assert!(entry.size >= 64);

    // An edit snapshots history, which counts toward the reported size.
    session
        .update_entry(
            &uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "sized2".into(),
                username: "user".into(),
                password: "pass".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    let entry = &session.snapshot().unwrap().root.entries[0];
    assert_eq!(entry.size as usize, session.get_entry_storage(&uuid).unwrap().total);
}

#[test]
fn entry_history_caps_at_ten_versions() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "v0".into(),
            username: "".into(),
            password: "p0".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();
    for i in 1..=14 {
        session
            .update_entry(
                &uuid,
                &EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: format!("v{i}"),
                    username: "".into(),
                    password: format!("p{i}"),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    icon: Some(None),
                    color: None,
                    tags: None,
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
    }
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 10);
    assert_eq!(history[0].title, "v13");
    assert_eq!(history[9].title, "v4");
}

#[test]
fn entry_history_cap_reads_database_meta() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    // A cap of 3 snapshots comes from the DB meta (KeePass HistoryMaxItems).
    {
        let db = session.require_db_mut().unwrap();
        db.meta.history_max_items = Some(3);
    }
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "v0".into(),
            username: "".into(),
            password: "p0".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();
    for i in 1..=8 {
        session
            .update_entry(
                &uuid,
                &EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: format!("v{i}"),
                    username: "".into(),
                    password: format!("p{i}"),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    icon: Some(None),
                    color: None,
                    tags: None,
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
    }
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 3, "meta cap of 3 must bound the history");
    assert_eq!(history[0].title, "v7");
    assert_eq!(history[2].title, "v5");
    // Zero in meta means unlimited: no trimming at all.
    {
        let db = session.require_db_mut().unwrap();
        db.meta.history_max_items = Some(0);
    }
    for i in 9..=12 {
        session
            .update_entry(
                &uuid,
                &EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: format!("v{i}"),
                    username: "".into(),
                    password: format!("p{i}"),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    icon: Some(None),
                    color: None,
                    tags: None,
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
    }
    let history = session.get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), 7, "zero cap must keep every snapshot");
    // The cap is read from the database meta after a save + reopen too.
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let _state = reopened
        .open(&dir.path().join("test.kdbx"), "master-password", None)
        .unwrap();
    {
        let db = reopened.require_db_mut().unwrap();
        assert_eq!(db.meta.history_max_items, Some(0));
    }
}

#[test]
fn entry_icon_and_color_round_trip_and_clear() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Iconic".into(),
            username: "".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(Some(1)),
            color: Some("#FF8800".into()),
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let entry = &state.root.entries[0];
    assert_eq!(entry.icon, Some(1));
    assert_eq!(entry.color.as_deref(), Some("#FF8800"));

    // Clearing icon/color reverts to defaults.
    let state = session
        .update_entry(
            &entry.uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Iconic".into(),
                username: "".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    assert_eq!(state.root.entries[0].icon, None);
    assert_eq!(state.root.entries[0].color, None);

    // Icon survives a save/reopen round trip.
    let state = session
        .update_entry(
            &entry.uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Iconic".into(),
                username: "".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(Some(3)),
                color: Some("#2288FF".into()),
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    assert_eq!(state.root.entries[0].icon, Some(3));
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert_eq!(state.root.entries[0].icon, Some(3));
    assert_eq!(state.root.entries[0].color.as_deref(), Some("#2288FF"));
}

#[test]
fn foreground_color_survives_edits_from_other_clients() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    // A foreign KeePass client set both a foreground and background color.
    {
        let db = session.require_db_mut().unwrap();
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", "Colored");
        entry.background_color = Some("#112233".parse().unwrap());
        entry.foreground_color = Some("#AABBCC".parse().unwrap());
    }
    // SecPivot edits the entry (background managed, foreground untouched).
    let uuid = {
        let db = session.require_db().unwrap();
        let root = db.root();
        let entry = root.entries().next().unwrap();
        entry.id().uuid().to_string()
    };
    let state = session
        .update_entry(
            &uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Colored".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "https://example.com".into(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: None,
                color: Some("#FF8800".into()),
                tags: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(state.root.entries[0].color.as_deref(), Some("#FF8800"));
    {
        let db = session.require_db().unwrap();
        let entry = db
            .entry(parse_entry_id(&state.root.entries[0].uuid).unwrap())
            .unwrap();
        assert_eq!(
            entry.foreground_color.as_ref().map(ToString::to_string),
            Some("#AABBCC".to_owned()),
            "foreground color must survive an edit that only touches background"
        );
    }
    // Batch edit likewise preserves the foreground color.
    let state = session
        .update_entries(
            &[state.root.entries[0].uuid.clone()],
            &EntryPatch {
                color: Some("#00CC66".into()),
                tags: None,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(state.root.entries[0].color.as_deref(), Some("#00CC66"));
    {
        let db = session.require_db().unwrap();
        let entry = db
            .entry(parse_entry_id(&state.root.entries[0].uuid).unwrap())
            .unwrap();
        assert_eq!(
            entry.foreground_color.as_ref().map(ToString::to_string),
            Some("#AABBCC".to_owned()),
            "batch color edit must also preserve the foreground color"
        );
    }
    // Save + reopen keeps the foreground color too.
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let _state = reopened.open(&path, "master-password", None).unwrap();
    let db = reopened.require_db().unwrap();
    let root = db.root();
    let entry = root.entries().next().unwrap();
    assert_eq!(
        entry.foreground_color.as_ref().map(ToString::to_string),
        Some("#AABBCC".to_owned()),
        "foreground color survives save/reopen"
    );
}

#[test]
fn entry_tags_round_trip_and_batch_edit() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    // Create with tags.
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Tagged".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: None,
            color: None,
            tags: Some("work,  email ,,".into()),
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    assert_eq!(state.root.entries[0].tags.as_deref(), Some("work, email"));

    // Single edit rewrites tags; empty string clears them.
    let uuid = state.root.entries[0].uuid.clone();
    let state = session
        .update_entry(
            &uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Tagged".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: None,
                color: None,
                tags: Some("  ".into()),
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    assert_eq!(state.root.entries[0].tags, None);

    // Batch edit sets tags on the target.
    let state = session
        .update_entries(
            std::slice::from_ref(&uuid),
            &EntryPatch {
                tags: Some("personal, bank".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        state.root.entries[0].tags.as_deref(),
        Some("personal, bank")
    );

    // Untouched tags survive a batch patch that touches other fields.
    let state = session
        .update_entries(
            std::slice::from_ref(&uuid),
            &EntryPatch {
                notes: Some("note".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        state.root.entries[0].tags.as_deref(),
        Some("personal, bank")
    );

    // Tags survive save + reopen.
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert_eq!(
        state.root.entries[0].tags.as_deref(),
        Some("personal, bank")
    );
}

#[test]
fn group_icon_round_trip() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Mail".into(),
            icon: Some(4),
        })
        .unwrap();
    assert_eq!(state.root.children[0].icon, Some(4));
}

#[test]
fn set_group_icon_updates_and_resets() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let group = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Mail".into(),
            icon: None,
        })
        .unwrap()
        .root
        .children
        .remove(0);
    // Update to a built-in index.
    let state = session.set_group_icon(&group.uuid, Some(7)).unwrap();
    assert_eq!(state.root.children[0].icon, Some(7));
    assert_eq!(state.root.children[0].name, "Mail");
    // `None` resets to the default icon.
    let state = session.set_group_icon(&group.uuid, None).unwrap();
    assert_eq!(state.root.children[0].icon, None);
    // Unknown group id still errors.
    assert!(session
        .set_group_icon("00000000-0000-0000-0000-000000000000", Some(1))
        .is_err());
}

#[test]
fn group_expand_state_persists_and_survives_reopen() {
    use keepass::db::{CustomDataItem, CustomDataValue};
    use std::collections::HashMap as StdHashMap;

    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Mail".into(),
            icon: None,
        })
        .unwrap();
    let group = state.root.children[0].clone();
    let group_id = super::helpers::parse_group_id(&group.uuid).unwrap();

    // New groups default to expanded; a collapse writes `is_expanded=false`.
    assert!(group.is_expanded);
    let state = session.set_group_expanded(&group.uuid, false).unwrap();
    assert!(!state.root.children[0].is_expanded);

    // Notes/tags authored by another KeePass client are exposed read-only.
    {
        let db = session.require_db_mut().unwrap();
        let mut group = db.group_mut(group_id).expect("group must exist");
        group.notes = Some("内部账号".into());
        group.tags = vec!["工作".into(), "紧急".into()];
        let mut map = StdHashMap::new();
        map.insert(
            "grp.key".into(),
            CustomDataItem {
                value: Some(CustomDataValue::String("v".into())),
                last_modification_time: None,
            },
        );
        group.custom_data = map;
    }
    // A write op bumps the revision so the snapshot is rebuilt.
    let state = session.set_group_expanded(&group.uuid, true).unwrap();
    let group = &state.root.children[0];
    assert_eq!(group.notes.as_deref(), Some("内部账号"));
    assert_eq!(group.tags.as_deref(), Some("工作, 紧急"));
    assert!(group.is_expanded);
    assert_eq!(group.custom_data.len(), 1);

    // Unknown group id still errors.
    assert!(session
        .set_group_expanded("00000000-0000-0000-0000-000000000000", true)
        .is_err());

    // Save + reopen: expansion, notes, tags, and CustomData all survive.
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let _ = reopened
        .open(&dir.path().join("test.kdbx"), "master-password", None)
        .unwrap();
    let group = &reopened.snapshot().unwrap().root.children[0];
    assert!(group.is_expanded);
    assert_eq!(group.notes.as_deref(), Some("内部账号"));
    assert_eq!(group.tags.as_deref(), Some("工作, 紧急"));
    assert_eq!(group.custom_data.len(), 1);
}

#[test]
fn group_expand_batch_is_atomic_and_persists() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Mail".into(),
            icon: None,
        })
        .unwrap();
    let mail_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Work".into(),
            icon: None,
        })
        .unwrap();
    let work_uuid = state
        .root
        .children
        .iter()
        .find(|group| group.name == "Work")
        .unwrap()
        .uuid
        .clone();
    session.save().unwrap();

    let unknown = "00000000-0000-0000-0000-000000000000".to_owned();
    assert!(session
        .set_groups_expanded(&[mail_uuid.clone(), unknown, work_uuid.clone()], false,)
        .is_err());
    let state = session.snapshot().unwrap();
    assert!(!state.dirty, "failed batch must not mark the vault dirty");
    assert!(state.root.children.iter().all(|group| group.is_expanded));

    let state = session
        .set_groups_expanded(&[mail_uuid.clone(), work_uuid.clone()], false)
        .unwrap();
    assert!(!state.dirty, "expansion must not mark the vault dirty");
    assert!(state.root.children.iter().all(|group| !group.is_expanded));
    session.save().unwrap();

    let state = session.set_groups_expanded(&[], true).unwrap();
    assert!(!state.dirty, "empty batch must remain a no-op");
    assert!(state.root.children.iter().all(|group| !group.is_expanded));

    drop(session);
    let mut reopened = VaultSession::default();
    reopened.open(&path, "master-password", None).unwrap();
    let state = reopened.snapshot().unwrap();
    assert!(state.root.children.iter().all(|group| !group.is_expanded));
}

#[test]
fn db_meta_name_and_description_round_trip() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    // Fresh database: no name/description set.
    let state = session.snapshot().unwrap();
    assert_eq!(state.database_name, None);
    assert_eq!(state.database_description, None);

    // Set both.
    let state = session
        .update_db_meta(Some("工作库".into()), Some("团队共享".into()))
        .unwrap();
    assert_eq!(state.database_name.as_deref(), Some("工作库"));
    assert_eq!(state.database_description.as_deref(), Some("团队共享"));

    // Update name only; description is untouched (absent field).
    let state = session.update_db_meta(Some("个人库".into()), None).unwrap();
    assert_eq!(state.database_name.as_deref(), Some("个人库"));
    assert_eq!(state.database_description.as_deref(), Some("团队共享"));

    // Empty string clears a field.
    let state = session.update_db_meta(Some("".into()), None).unwrap();
    assert_eq!(state.database_name, None);
    assert_eq!(state.database_description.as_deref(), Some("团队共享"));

    // Save + reopen: the meta persists.
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let _ = reopened
        .open(&dir.path().join("test.kdbx"), "master-password", None)
        .unwrap();
    let state = reopened.snapshot().unwrap();
    assert_eq!(state.database_name, None);
    assert_eq!(state.database_description.as_deref(), Some("团队共享"));
}

#[test]
fn move_entry_between_groups() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "A".into(),
            icon: None,
        })
        .unwrap();
    let group_a = state.root.children[0].uuid.clone();
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "B".into(),
            icon: None,
        })
        .unwrap();
    let group_b = state.root.children[1].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_a.clone(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let entry_uuid = state.root.children[0].entries[0].uuid.clone();

    let state = session.move_entry(&entry_uuid, &group_b).unwrap();
    assert_eq!(state.root.children[0].entries.len(), 0);
    assert_eq!(state.root.children[1].entries.len(), 1);
    assert_eq!(state.root.children[1].entries[0].uuid, entry_uuid);
    assert_eq!(state.root.children[1].entries[0].group_uuid, group_b);

    // Moving into the same group is a no-op.
    let state = session.move_entry(&entry_uuid, &group_b).unwrap();
    assert_eq!(state.root.children[1].entries.len(), 1);
}

#[test]
fn delete_entries_moves_all_to_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let mut uuids = Vec::new();
    for i in 0..3 {
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: format!("E{i}"),
                username: "".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        uuids.push(state.root.entries.last().unwrap().uuid.clone());
    }
    let state = session.delete_entries(&uuids).unwrap();
    assert!(state.root.entries.is_empty());
    assert_eq!(state.root.children[0].entries.len(), 3);
    assert!(state.root.children[0].is_recycle_bin);

    // Second pass permanently deletes the recycled entries; the now-empty
    // recycle bin no longer occupies a slot in the tree.
    let state = session.delete_entries(&uuids).unwrap();
    assert!(state.root.children.iter().all(|g| !g.is_recycle_bin));
}

#[test]
fn update_entries_applies_patch_to_all_uuids_and_skips_absent_fields() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let mut uuids = Vec::new();
    for i in 0..3 {
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: format!("E{i}"),
                username: format!("user{i}"),
                password: "secret".into(),
                url: format!("https://e{i}.example"),
                notes: "note".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        uuids.push(state.root.entries.last().unwrap().uuid.clone());
    }

    let patch = EntryPatch {
        title: Some("Renamed".into()),
        username: Some("shared".into()),
        ..EntryPatch::default()
    };
    let state = session.update_entries(&uuids, &patch).unwrap();
    assert_eq!(state.root.entries.len(), 3);
    for (i, entry) in state.root.entries.iter().enumerate() {
        assert_eq!(entry.title, "Renamed");
        assert_eq!(entry.username, "shared");
        assert_eq!(
            session.get_entry_password(&entry.uuid).unwrap(),
            "secret",
            "untouched password must survive"
        );
        assert_eq!(
            entry.url,
            format!("https://e{i}.example"),
            "absent url field must stay untouched"
        );
    }
    let history = session.get_entry_history(&uuids[0]).unwrap();
    assert!(
        !history.is_empty(),
        "each patched entry gains a history snapshot"
    );
}

#[test]
fn update_entries_empty_strings_and_clear_flags_clear_optional_attributes() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "E".into(),
            username: "u".into(),
            password: "p".into(),
            url: "".into(),
            notes: "".into(),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            expires: Some("2026-12-31T23:59:00Z".into()),
            icon: Some(Some(7)),
            color: Some("#2288FF".into()),
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();
    assert!(state.root.entries[0].has_totp);

    let patch = EntryPatch {
        totp: Some("".into()),
        clear_expires: true,
        clear_icon: true,
        clear_color: true,
        ..EntryPatch::default()
    };
    let state = session.update_entries(&[uuid], &patch).unwrap();
    let entry = &state.root.entries[0];
    assert!(!entry.has_totp, "empty totp clears the seed");
    assert_eq!(entry.expires, None, "clear_expires removes the expiry");
    assert!(
        entry.icon.is_none(),
        "clear_icon resets to the default icon"
    );
    assert!(entry.color.is_none(), "clear_color removes the tag");
    assert_eq!(entry.title, "E", "absent fields stay untouched");
    assert_eq!(
        session.get_entry_password(&entry.uuid).unwrap(),
        "p",
        "untouched password must survive"
    );
}

#[test]
fn update_entries_sets_expiry_icon_and_color() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "E".into(),
            username: "u".into(),
            password: "p".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    let patch = EntryPatch {
        expires: Some("2027-06-01T12:00:00Z".into()),
        icon: Some(5),
        color: Some("#00CC66".into()),
        tags: None,
        ..EntryPatch::default()
    };
    let state = session.update_entries(&[uuid], &patch).unwrap();
    let entry = &state.root.entries[0];
    assert_eq!(entry.expires.as_deref(), Some("2027-06-01T12:00:00Z"));
    assert_eq!(entry.icon, Some(5));
    assert_eq!(entry.color.as_deref(), Some("#00CC66"));
}

#[test]
fn update_entries_unknown_uuid_errors_and_empty_list_is_a_noop() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let before = session.state().unwrap().unwrap();
    assert!(!before.dirty, "fresh session is clean");
    let patch = EntryPatch {
        title: Some("X".into()),
        ..EntryPatch::default()
    };
    let err = session
        .update_entries(&["00000000-0000-0000-0000-000000000000".into()], &patch)
        .unwrap_err();
    assert!(err.contains("条目不存在"));
    assert_eq!(
        session.state().unwrap().unwrap().dirty,
        before.dirty,
        "failed batch must not change the dirty flag"
    );

    let state = session.update_entries(&[], &patch).unwrap();
    assert_eq!(
        state.dirty, before.dirty,
        "empty batch must not mark the vault dirty"
    );
}

#[test]
fn update_entries_is_atomic_on_unknown_uuid() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let mut uuids = Vec::new();
    for i in 0..2 {
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: format!("E{i}"),
                username: "".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        uuids.push(state.root.entries.last().unwrap().uuid.clone());
    }
    // A batch with a valid entry first and an unknown uuid second must
    // abort the whole batch: no entry may change, no history recorded.
    let patch = EntryPatch {
        title: Some("Batch".into()),
        ..EntryPatch::default()
    };
    let err = session
        .update_entries(
            &[
                uuids[0].clone(),
                "00000000-0000-0000-0000-000000000000".into(),
            ],
            &patch,
        )
        .unwrap_err();
    assert!(err.contains("条目不存在"));
    let state = session.state().unwrap().unwrap();
    assert_eq!(state.root.entries[0].title, "E0", "no partial application");
    assert_eq!(state.root.entries[1].title, "E1");
    let history = session.get_entry_history(&uuids[0]).unwrap();
    assert!(history.is_empty(), "no history snapshot on aborted batch");
}

#[test]
fn save_clears_dirty_and_persists() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    session
        .add_entry(&EntryInput {
            group_uuid,
            title: "Inbox".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let saved = session.save().unwrap();
    assert!(!saved.dirty);
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "Mail");
    assert_eq!(state.root.children[0].entries.len(), 1);
}

#[test]
fn save_as_writes_new_file_and_switches_session_target() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let new_path = dir.path().join("copy.kdbx");
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    session
        .add_entry(&EntryInput {
            group_uuid,
            title: "Inbox".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();

    let state = session.save_as(&new_path).unwrap();
    assert_eq!(state.path, new_path.to_string_lossy());
    assert!(!state.dirty, "save as marks the session clean");

    // The new file holds the data; the original file was never touched.
    let mut reopened = VaultSession::default();
    let state = reopened.open(&new_path, "master-password", None).unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "Mail");
    assert_eq!(state.root.children[0].entries.len(), 1);
    drop(reopened);
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert!(
        state.root.children.is_empty(),
        "original file must keep its pre-save-as content"
    );
    drop(reopened);

    // Subsequent edits and saves go to the new target.
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "After".into(),
            username: "".into(),
            password: "p".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let state = reopened.open(&new_path, "master-password", None).unwrap();
    assert_eq!(state.root.entries.len(), 1);
    assert_eq!(state.root.entries[0].title, "After");
}

#[test]
fn save_as_failure_keeps_session_untouched() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap();
    let missing_dir = dir.path().join("no-such-dir").join("v.kdbx");

    let err = session.save_as(&missing_dir).unwrap_err();
    assert!(!err.is_empty(), "saving into a missing directory must fail");
    let state = session.state().unwrap().unwrap();
    assert_eq!(
        state.path,
        path.to_string_lossy(),
        "session target unchanged"
    );
    assert_eq!(state.root.children.len(), 1);
    assert!(state.dirty, "unsaved edits remain dirty");
}

#[test]
fn save_as_from_remote_session_switches_to_local() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local_dir = dir.path().join("local");
    let mut session = VaultSession::default();
    let state = session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local_dir,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert!(state.path.starts_with("s3://"));
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Local".into(),
        })
        .unwrap();
    session
        .add_entry(&EntryInput {
            group_uuid: state.root.children[0].uuid.clone(),
            title: "Exported".into(),
            username: "".into(),
            password: "p".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();

    let local_path = dir.path().join("exported.kdbx");
    let state = session.save_as(&local_path).unwrap();
    assert_eq!(state.path, local_path.to_string_lossy());
    assert!(!state.dirty);

    // Later saves are local: the S3 object must not receive the group.
    session.save().unwrap();
    let remote_db = Database::parse(
        &storage.get("vaults/seed.kdbx").unwrap(),
        DatabaseKey::new().with_password("pw"),
    )
    .unwrap();
    assert_eq!(
        remote_db.root().groups().count(),
        0,
        "remote target must not receive post-save-as changes"
    );
    let mut reopened = VaultSession::default();
    let state = reopened.open(&local_path, "pw", None).unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "Local");
}

#[test]
fn snapshot_cache_serves_unchanged_state_without_rebuild() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap();
    // Repeated reads must be consistent even with the cache: same tree,
    // same dirty flag, and edits invalidate the cache.
    let first = session.state().unwrap().unwrap();
    assert_eq!(first.root.children.len(), 1);
    let second = session.state().unwrap().unwrap();
    assert_eq!(second.root.children.len(), 1);
    assert_eq!(second.root.children[0].name, "Mail");
    assert_eq!(second.dirty, first.dirty);

    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Work".into(),
        })
        .unwrap();
    let third = session.state().unwrap().unwrap();
    assert_eq!(third.root.children.len(), 2);
    assert!(third.dirty, "edit must be reflected and keep dirty=true");
}

#[test]
fn concurrent_edit_during_save_keeps_dirty_flag() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap();
    session
        .add_entry(&EntryInput {
            group_uuid: state.root.children[0].uuid.clone(),
            title: "Inbox".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    // An edit lands between the save's prepare (locked) and completion
    // (locked again): the completion must not clear the new dirty state.
    let job = session.prepare_save(false).unwrap();
    let revision = job.revision;
    persist_save(job).unwrap();
    session.mark_dirty();
    let state = session.complete_save(revision, [0u8; 32]).unwrap();
    assert!(state.dirty, "edit during save must stay dirty");
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "Mail");
    assert_eq!(state.root.children[0].entries.len(), 1);
}

#[test]
fn empty_group_remains_visible_after_reopen() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);

    // A freshly created empty group is visible so the user can populate it.
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "New".into(),
        })
        .unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "New");
    assert!(state.root.children[0].entries.is_empty());

    // Even without saving, re-reading the session keeps it visible.
    let again = session.state().unwrap().unwrap();
    assert_eq!(again.root.children.len(), 1);

    // After persisting and reopening, the still-empty group stays visible.
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "New");
    assert!(state.root.children[0].entries.is_empty());
}

#[test]
fn empty_child_group_stays_visible_after_reopen() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let parent = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Parent".into(),
        })
        .unwrap()
        .root
        .children[0]
        .uuid
        .clone();
    // A nested group inside the parent is empty; both levels stay visible.
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(parent.clone()),
            icon: None,
            name: "EmptyChild".into(),
        })
        .unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].children.len(), 1);
    session.save().unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].children.len(), 1);
    assert_eq!(state.root.children[0].children[0].name, "EmptyChild");
}

#[test]
fn entry_expiry_roundtrip_and_clear() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Expiring".into(),
            username: "u".into(),
            password: "p".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: Some("2020-01-01T00:00:00Z".to_owned()),
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let entry = &state.root.entries[0];
    assert_eq!(entry.expires.as_deref(), Some("2020-01-01T00:00:00Z"));
    assert!(entry.expired, "past expiry should be flagged");

    // Clearing the expiry marks the entry as not expired.
    let state = session
        .update_entry(
            &entry.uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Expiring".into(),
                username: "u".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    let entry = &state.root.entries[0];
    assert!(entry.expires.is_none());
    assert!(!entry.expired);

    // A future expiry persists across save/reopen.
    session
        .update_entry(
            &entry.uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Expiring".into(),
                username: "u".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: Some("2099-12-31T23:59:59Z".to_owned()),
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    session.save().unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    let entry = &state.root.entries[0];
    assert_eq!(entry.expires.as_deref(), Some("2099-12-31T23:59:59Z"));
    assert!(!entry.expired);
}

#[test]
fn disabled_expiry_flag_never_marks_entry_expired() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("official.kdbx");
    let mut db = keepass::Database::new();
    let past = chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let mut root = db.root_mut();
    // KeePass default: expiry timestamp present, expiry disabled.
    let mut disabled = root.add_entry();
    disabled.set_unprotected(FIELD_TITLE, "Disabled");
    disabled.set_unprotected(FIELD_PASSWORD, "p");
    disabled.times.expiry = Some(past);
    disabled.times.expires = Some(false);
    // Expiry status never set: `expires = None`.
    let mut unset = root.add_entry();
    unset.set_unprotected(FIELD_TITLE, "Unset");
    unset.set_unprotected(FIELD_PASSWORD, "p");
    unset.times.expiry = Some(past);
    unset.times.expires = None;
    save_database(
        &db,
        &path,
        DatabaseKey::new().with_password("master-password"),
    )
    .unwrap();
    let mut session = VaultSession::default();
    let state = session.open(&path, "master-password", None).unwrap();
    assert_eq!(state.root.entries.len(), 2);
    for entry in &state.root.entries {
        assert!(
            entry.expires.is_none(),
            "{} should not expose expiry",
            entry.title
        );
        assert!(
            !entry.expired,
            "{} should not be flagged expired",
            entry.title
        );
    }

    // A genuinely enabled past expiry is still flagged after reopen.
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Genuine".into(),
            username: "u".into(),
            password: "p".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: Some("2020-01-01T00:00:00Z".to_owned()),
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let state = session.state().unwrap().unwrap();
    let genuine = state
        .root
        .entries
        .iter()
        .find(|e| e.title == "Genuine")
        .expect("added entry should be present");
    assert!(genuine.expired, "enabled past expiry should be flagged");
}

#[test]
fn change_master_key_reencrypts_and_reopens_with_new_credentials() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Loopback".into(),
            username: "root".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let state = session.change_master_key("new-password", None).unwrap();
    assert!(!state.dirty);
    assert_eq!(state.root.entries.len(), 1);
    drop(session);

    // The old password no longer opens the vault.
    let mut wrong = VaultSession::default();
    assert!(wrong.open(&path, "master-password", None).is_err());
    // The new password does, and the entry is intact.
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "new-password", None).unwrap();
    assert_eq!(state.root.entries.len(), 1);
    assert_eq!(state.root.entries[0].title, "Loopback");
}

#[test]
fn change_master_key_supports_keyfile_only_and_keeps_session_alive() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let keyfile = dir.path().join("keyfile.key");
    std::fs::write(&keyfile, b"0123456789abcdef0123456789abcdef").unwrap();

    let state = session
        .change_master_key("", Some(&keyfile))
        .expect("keyfile-only vault should save");
    assert!(!state.dirty);
    // Session continues to work with the new key in memory.
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "After".into(),
            username: "u".into(),
            password: "p".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    assert_eq!(state.root.entries.len(), 1);
    let saved = session.save().unwrap();
    assert!(!saved.dirty);
    drop(session);

    // Reopens with the keyfile only.
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "", Some(&keyfile)).unwrap();
    assert_eq!(state.root.entries.len(), 1);
    assert_eq!(state.root.entries[0].title, "After");
}

#[test]
fn delete_group_moves_whole_subtree_to_recycle_bin_and_restores() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Parent".into(),
        })
        .unwrap();
    let parent_uuid = state.root.children[0].uuid.clone();

    session
        .add_group(&GroupInput {
            parent_uuid: Some(parent_uuid.clone()),
            icon: None,
            name: "Child".into(),
        })
        .unwrap();

    session
        .add_entry(&EntryInput {
            group_uuid: parent_uuid.clone(),
            title: "Loopback".into(),
            username: "root".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let entry_uuid = session.state().unwrap().unwrap().root.children[0].entries[0]
        .uuid
        .clone();

    session.delete_group(&parent_uuid).unwrap();
    let root = session.state().unwrap().unwrap().root;
    // The whole subtree now lives under the recycle bin.
    let bin = root
        .children
        .iter()
        .find(|g| g.is_recycle_bin)
        .expect("recycle bin should exist");
    assert_eq!(bin.children.len(), 1);
    assert_eq!(bin.children[0].name, "Parent");
    assert_eq!(bin.children[0].children[0].name, "Child");
    assert_eq!(bin.children[0].entries[0].uuid, entry_uuid);
    assert!(!root.children.iter().any(|g| g.uuid == parent_uuid));

    // Restoring brings the group (with its subtree) back to root.
    session.restore_group(&parent_uuid).unwrap();
    let root = session.state().unwrap().unwrap().root;
    let parent = root
        .children
        .iter()
        .find(|g| g.uuid == parent_uuid)
        .unwrap();
    assert_eq!(parent.children[0].name, "Child");
    assert_eq!(parent.entries[0].uuid, entry_uuid);
}

#[test]
fn recycle_bin_deletes_entry_then_restores_and_empties() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Loopback".into(),
            username: "root".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let entry_uuid = session.state().unwrap().unwrap().root.entries[0]
        .uuid
        .clone();

    // Deleting an entry moves it to the recycle bin.
    session.delete_entry(&entry_uuid).unwrap();
    let root = session.state().unwrap().unwrap().root;
    assert!(root.entries.is_empty());
    let bin = root.children.iter().find(|g| g.is_recycle_bin).unwrap();
    assert_eq!(bin.entries.len(), 1);
    assert_eq!(bin.entries[0].uuid, entry_uuid);

    // Restoring returns it to its original group.
    session.restore_entry(&entry_uuid).unwrap();
    let root = session.state().unwrap().unwrap().root;
    assert_eq!(root.entries[0].uuid, entry_uuid);

    // Deleting again, then emptying the bin permanently removes it.
    session.delete_entry(&entry_uuid).unwrap();
    session.empty_recycle_bin().unwrap();
    let state = session.state().unwrap().unwrap();
    // The recycle bin now holds nothing, so it is filtered out of the
    // tree like any other empty group.
    assert!(
        !state.root.children.iter().any(|g| g.is_recycle_bin),
        "empty recycle bin should be hidden"
    );
    session.save().unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    assert!(state.root.entries.is_empty());
    assert!(
        !state.root.children.iter().any(|g| g.is_recycle_bin),
        "empty recycle bin should not reappear after reopen"
    );
}

#[test]
fn recycle_bin_is_persisted_across_reopen() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Loopback".into(),
            username: "root".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let entry_uuid = session.state().unwrap().unwrap().root.entries[0]
        .uuid
        .clone();
    session.delete_entry(&entry_uuid).unwrap();
    session.save().unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    let bin = state
        .root
        .children
        .iter()
        .find(|g| g.is_recycle_bin)
        .unwrap();
    assert_eq!(bin.entries.len(), 1);
    // The recycled entry is still restorable after reopen.
    reopened.restore_entry(&entry_uuid).unwrap();
    let state = reopened.state().unwrap().unwrap();
    assert!(state.root.entries.iter().any(|e| e.uuid == entry_uuid));
}

#[test]
fn rejects_invalid_parameters() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    let err = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "   ".into(),
        })
        .unwrap_err();
    assert!(err.contains("分组名称"));

    let err = session
        .add_group(&GroupInput {
            parent_uuid: Some("not-a-uuid".into()),
            icon: None,
            name: "X".into(),
        })
        .unwrap_err();
    assert!(err.contains("UUID"));

    let err = session.delete_group(ROOT_GROUP_UUID).unwrap_err();
    assert!(err.contains("根分组"));

    let err = session
        .add_entry(&EntryInput {
            group_uuid: "missing".into(),
            title: "T".into(),
            username: "".into(),
            password: "".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap_err();
    assert!(err.contains("UUID"));

    // Unknown kdf/cipher/compression rejected at create time.
    let path = dir.path().join("bad.kdbx");
    let err = VaultSession::default()
        .create(&path, "pw", "scrypt", "Aes256", "None", None)
        .unwrap_err();
    assert!(err.contains("kdf"));
}

#[test]
fn dto_wire_format_uses_camel_case() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Web".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid,
            title: "GitHub".into(),
            username: "alice".into(),
            password: "s3cret".into(),
            url: "https://github.com".into(),
            notes: "work".into(),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();

    let json = serde_json::to_value(&state).unwrap();
    let obj = json.as_object().unwrap();
    for key in ["path", "fileName", "root", "dirty", "modifiedAt"] {
        assert!(obj.contains_key(key), "missing VaultState key {key}");
    }
    assert!(
        !obj.contains_key("password"),
        "master password leaked in VaultState"
    );
    let root = json["root"].as_object().unwrap();
    for key in ["uuid", "parentUuid", "name", "children", "entries"] {
        assert!(root.contains_key(key), "missing VaultGroup key {key}");
    }
    let group = &json["root"]["children"][0];
    assert_eq!(group["parentUuid"].as_str(), Some(ROOT_GROUP_UUID));
    let entry = &group["entries"][0];
    for key in ["uuid", "groupUuid", "title", "username", "url", "notes"] {
        assert!(entry.get(key).is_some(), "missing VaultEntry key {key}");
    }
    assert!(
        entry.get("password").is_none(),
        "entry password leaked in VaultEntry"
    );
    // The TOTP seed must never leave the backend in a snapshot; only the
    // presence flag is serialized.
    assert!(
        entry.get("totp").is_none(),
        "TOTP seed leaked in VaultEntry"
    );
    assert!(entry["hasTotp"].is_boolean());
    // Optional fields absent on the entry are skipped entirely (not null).
    assert!(entry.get("icon").is_none());
    assert!(entry.get("tags").is_none());
    // Favorite is always present and a boolean.
    assert!(entry["favorite"].is_boolean());
}

#[test]
fn inputs_deserialize_from_camel_case() {
    let entry: EntryInput = serde_json::from_value(serde_json::json!({
        "groupUuid": "g1",
        "title": "T",
        "username": "u",
        "password": "p",
        "url": "https://x",
        "notes": "n",
        "totp": null,
    }))
    .unwrap();
    assert_eq!(entry.group_uuid, "g1");
    assert_eq!(entry.totp, None);

    let group: GroupInput = serde_json::from_value(serde_json::json!({
        "parentUuid": null,
        "name": "Root",
    }))
    .unwrap();
    assert_eq!(group.parent_uuid, None);

    let nested: GroupInput = serde_json::from_value(serde_json::json!({
        "parentUuid": "abc",
        "name": "Web",
    }))
    .unwrap();
    assert_eq!(nested.parent_uuid.as_deref(), Some("abc"));

    let patch: EntryPatch = serde_json::from_value(serde_json::json!({
        "title": "Batch",
        "clearExpires": true,
        "clearIcon": true,
        "clearColor": true,
    }))
    .unwrap();
    assert_eq!(patch.title.as_deref(), Some("Batch"));
    assert_eq!(patch.username, None, "absent fields stay untouched");
    assert!(patch.clear_expires && patch.clear_icon && patch.clear_color);

    let partial: EntryPatch = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(partial.title.is_none());
    assert!(!partial.clear_expires);
}

#[test]
fn entry_input_icon_tristate_deserializes_absent_null_and_index() {
    let base = |mut value: serde_json::Value| {
        let obj = value.as_object_mut().unwrap();
        obj.insert("groupUuid".into(), "g1".into());
        obj.insert("title".into(), "T".into());
        obj.insert("username".into(), "u".into());
        obj.insert("password".into(), "p".into());
        obj.insert("url".into(), "https://x".into());
        obj.insert("notes".into(), "n".into());
        value
    };
    // Absent icon (content-only edit) keeps the current icon.
    let absent: EntryInput = serde_json::from_value(base(serde_json::json!({}))).unwrap();
    assert_eq!(absent.icon, None);
    // Explicit null resets to the default icon.
    let clear: EntryInput =
        serde_json::from_value(base(serde_json::json!({"icon": null}))).unwrap();
    assert_eq!(clear.icon, Some(None));
    // A number sets the built-in index.
    let set: EntryInput = serde_json::from_value(base(serde_json::json!({"icon": 7}))).unwrap();
    assert_eq!(set.icon, Some(Some(7)));
}

#[test]
fn totp_computes_rfc6238_vector_codes() {
    // RFC 6238 Appendix B: secret = ASCII "12345678901234567890".
    let seed =
        "otpauth://totp/RFC6238:test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8&period=30";
    let at_59 = compute_totp_at(seed, 59).unwrap();
    assert_eq!(at_59.code, "94287082");
    assert_eq!(at_59.period, 30);
    assert_eq!(at_59.valid_for, 1);
    let at_2e9 = compute_totp_at(seed, 2_000_000_000).unwrap();
    assert_eq!(at_2e9.code, "69279037");
}

#[test]
fn totp_accepts_raw_base32_seed() {
    // Same secret as above, provided as a raw Base32 key → SHA-1 / 6 digits.
    let at_59 = compute_totp_at("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59).unwrap();
    assert_eq!(at_59.code, "287082");
    assert_eq!(at_59.period, 30);
    assert_eq!(at_59.valid_for, 1);
}

#[test]
fn totp_uri_without_digits_defaults_to_six() {
    // Google Authenticator exports omit `digits`; keepass 0.13 would
    // default to 8. The RFC 6238 vector secret must still yield the
    // 6-digit code, like KeePass and the raw-seed path above.
    let seed =
            "otpauth://totp/Google:user@example.com?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Google";
    let at_59 = compute_totp_at(seed, 59).unwrap();
    assert_eq!(at_59.code, "287082");
    assert_eq!(at_59.valid_for, 1);

    let no_query = compute_totp_at(
        "otpauth://totp/Google:user?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
        59,
    )
    .unwrap();
    assert_eq!(no_query.code, "287082");

    // An explicit `digits=8` is respected and unchanged.
    let explicit = compute_totp_at(
        "otpauth://totp/RFC6238:test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8&period=30",
        59,
    )
    .unwrap();
    assert_eq!(explicit.code, "94287082");
}

#[test]
fn totp_accepts_lowercase_secret_in_uri() {
    // keepass decodes secrets with the `base32` crate whose RFC 4648
    // table only accepts A-Z / 2-7; a lowercase secret (typed by hand or
    // scraped from a QR code) must be uppercased before parsing.
    let user_uri =
            "otpauth://totp/Google:m2uyoo@gmail.com?secret=2r23njeqijx7zfia7u2b2ena4lhkkuwt&issuer=Google";
    let code = compute_totp_at(user_uri, 1_700_000_000).expect("lowercase secret must decode");
    assert_eq!(code.code.len(), 6);
    assert_eq!(code.period, 30);

    // The lowercase URI must yield the same code as its uppercased twin.
    let upper_uri =
            "otpauth://totp/Google:m2uyoo@gmail.com?secret=2R23NJEQIJX7ZFIA7U2B2ENA4LHKKUWT&issuer=Google";
    assert_eq!(
        compute_totp_at(upper_uri, 1_700_000_000).unwrap().code,
        code.code,
    );

    // Raw lowercase Base32 keys were already normalized; keep it working.
    let raw = compute_totp_at("2r23njeqijx7zfia7u2b2ena4lhkkuwt", 1_700_000_000)
        .expect("raw lowercase key must decode");
    assert_eq!(raw.code, code.code);
}

#[test]
fn totp_rejects_invalid_seed() {
    let err = compute_totp_at("INVALID!", 59).unwrap_err();
    assert!(err.contains("Base32"), "unexpected error: {err}");
}

#[test]
fn totp_code_requires_totp_field() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid,
            title: "Plain".into(),
            username: "".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.children[0].entries[0].uuid.clone();
    let err = session.totp_code(&uuid).unwrap_err();
    assert!(err.contains("没有 OTP"), "unexpected error: {err}");
}

#[test]
fn totp_code_session_returns_current_code() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid,
            title: "2FA".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.children[0].entries[0].uuid.clone();
    let code = session.totp_code(&uuid).unwrap();
    assert_eq!(code.code.len(), 6);
    assert_eq!(code.period, 30);
    assert_eq!(code.kind, "totp");
    assert!((1..=code.period).contains(&code.valid_for));
}

/// HOTP reads its counter from the `HmacOtp` field and advances it on
/// each request, writing `counter+1` back server-side.
#[test]
fn hotp_code_advances_counter_and_writes_back() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Hotp".into(),
            username: "".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![CustomField {
                name: "HmacOtp".into(),
                value: "JBSWY3DPEHPK3PXP".into(),
                protected: false,
            }],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();
    let first = session.totp_code(&uuid).unwrap();
    assert_eq!(first.kind, "hotp");
    assert_eq!(first.period, 0);
    assert_eq!(first.counter, Some(0));
    let second = session.totp_code(&uuid).unwrap();
    assert_eq!(second.counter, Some(1));
    // A third call keeps advancing (no repeat of an earlier code).
    let third = session.totp_code(&uuid).unwrap();
    assert_eq!(third.counter, Some(2));
}

/// A Steam guard entry yields a 5-character code from the Steam alphabet
/// with a live countdown (time-based).
#[test]
fn steam_code_is_five_chars_with_countdown() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid,
            title: "Steam".into(),
            username: "".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![CustomField {
                name: "SteamOtp".into(),
                value: "CNBNMZBN".into(),
                protected: false,
            }],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.children[0].entries[0].uuid.clone();
    let code = session.totp_code(&uuid).unwrap();
    assert_eq!(code.kind, "steam");
    assert_eq!(code.code.len(), 5);
    assert_eq!(code.period, 30);
    assert!((1..=code.period).contains(&code.valid_for));
}

#[test]
fn totp_code_wire_format_uses_camel_case() {
    let code = compute_totp_at("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59).unwrap();
    let json = serde_json::to_value(&code).unwrap();
    let obj = json.as_object().unwrap();
    for key in ["code", "kind", "validFor", "period"] {
        assert!(obj.contains_key(key), "missing TotpCode key {key}");
    }
}

/// TOTP seeds never serialize into `VaultState` snapshots: the renderer
/// learns only `hasTotp` and fetches the seed (or a code) on demand.
#[test]
fn totp_seed_never_serializes_into_snapshot() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let mut input = EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: "2FA".into(),
        username: "u".into(),
        password: "pw".into(),
        url: "".into(),
        notes: "".into(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    };
    input.totp = Some("JBSWY3DPEHPK3PXP".into());
    let state = session.add_entry(&input).unwrap();
    let entry = &state.root.entries[0];
    assert!(entry.has_totp);
    let json = serde_json::to_value(&state).unwrap();
    let serialized = serde_json::to_string(&json["root"]["entries"][0]).unwrap();
    assert!(
        !serialized.contains("JBSWY3DPEHPK3PXP"),
        "TOTP seed leaked into snapshot JSON: {serialized}"
    );
    assert!(serialized.contains("hasTotp"));
}

#[test]
fn toggle_favorite_round_trips_field() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid,
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.children[0].entries[0].uuid.clone();

    assert!(!session.snapshot().unwrap().root.children[0].entries[0].favorite);
    session.toggle_favorite(&uuid).unwrap();
    assert!(session.snapshot().unwrap().root.children[0].entries[0].favorite);
    // Second toggle removes the marker again.
    session.toggle_favorite(&uuid).unwrap();
    assert!(!session.snapshot().unwrap().root.children[0].entries[0].favorite);
}

#[test]
fn favorite_persists_after_save_and_reopen() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid,
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.children[0].entries[0].uuid.clone();
    session.toggle_favorite(&uuid).unwrap();
    session.save().unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    let _ = reopened
        .open(&dir.path().join("test.kdbx"), "master-password", None)
        .unwrap();
    let favorite = reopened.snapshot().unwrap().root.children[0].entries[0].favorite;
    assert!(favorite);
}

#[test]
fn custom_fields_and_attachments_round_trip() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();

    let data = BASE64.encode(b"hello attachment".as_slice());
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_uuid.clone(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![
                CustomField {
                    name: "PIN".into(),
                    value: "1234".into(),
                    protected: false,
                },
                CustomField {
                    name: "Question".into(),
                    value: "Answer".into(),
                    protected: false,
                },
            ],
            attachments: vec![AttachmentInput {
                name: "note.txt".into(),
                data: Some(data),
            }],
        })
        .unwrap();
    let entry = &state.root.children[0].entries[0];
    assert_eq!(entry.custom_fields.len(), 2);
    assert_eq!(
        entry
            .custom_fields
            .iter()
            .find(|f| f.name == "PIN")
            .map(|f| f.value.as_str()),
        Some("1234")
    );
    assert_eq!(entry.attachments.len(), 1);
    assert_eq!(entry.attachments[0].name, "note.txt");
    assert_eq!(entry.attachments[0].size, b"hello attachment".len());
    let uuid = entry.uuid.clone();

    // Update: drop one field, keep the attachment untouched (no data), add one.
    let state = session
        .update_entry(
            &uuid,
            &EntryInput {
                group_uuid: group_uuid.clone(),
                title: "E".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![CustomField {
                    name: "PIN".into(),
                    value: "9999".into(),
                    protected: false,
                }],
                attachments: vec![
                    AttachmentInput {
                        name: "note.txt".into(),
                        data: None,
                    },
                    AttachmentInput {
                        name: "second.bin".into(),
                        data: Some(BASE64.encode([1u8, 2, 3, 4].as_slice())),
                    },
                ],
            },
        )
        .unwrap();
    let entry = &state.root.children[0].entries[0];
    assert_eq!(entry.custom_fields.len(), 1);
    assert_eq!(entry.custom_fields[0].name, "PIN");
    assert_eq!(entry.custom_fields[0].value, "9999");
    assert_eq!(entry.attachments.len(), 2);
    let note = entry
        .attachments
        .iter()
        .find(|a| a.name == "note.txt")
        .expect("note.txt attachment present");
    assert_eq!(note.size, b"hello attachment".len());

    // Persist and reopen: everything survives.
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let state = reopened
        .open(&dir.path().join("test.kdbx"), "master-password", None)
        .unwrap();
    let entry = &state.root.children[0].entries[0];
    assert_eq!(entry.custom_fields.len(), 1);
    assert_eq!(entry.attachments.len(), 2);
}

#[test]
fn add_attachments_appends_without_rewriting_fields() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![AttachmentInput {
                name: "note.txt".into(),
                data: Some(BASE64.encode(b"original".as_slice())),
            }],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    // Add two new attachments; the existing one is kept untouched.
    let state = session
        .add_attachments(
            &uuid,
            &[
                AttachmentInput {
                    name: "second.bin".into(),
                    data: Some(BASE64.encode([1u8, 2, 3, 4].as_slice())),
                },
                AttachmentInput {
                    name: "note.txt".into(),
                    data: Some(BASE64.encode(b"replaced".as_slice())),
                },
            ],
        )
        .unwrap();
    let entry = state.root.entries.iter().find(|e| e.uuid == uuid).unwrap();
    assert_eq!(entry.attachments.len(), 2);
    let second = entry
        .attachments
        .iter()
        .find(|a| a.name == "second.bin")
        .expect("second attachment present");
    assert_eq!(second.size, 4);
    let note = entry
        .attachments
        .iter()
        .find(|a| a.name == "note.txt")
        .expect("note.txt attachment present");
    assert_eq!(note.size, b"replaced".len());
    // Fields (including the password) were never touched.
    assert_eq!(entry.title, "E");
    assert_eq!(session.get_entry_password(&uuid).unwrap(), "pw");

    // A bad base64 payload aborts the whole mutation.
    assert!(session
        .add_attachments(
            &uuid,
            &[AttachmentInput {
                name: "bad".into(),
                data: Some("!!!".into())
            }]
        )
        .is_err());
    let state = session.snapshot().unwrap();
    let entry = state.root.entries.iter().find(|e| e.uuid == uuid).unwrap();
    assert_eq!(entry.attachments.len(), 2);
}

#[test]
fn attachment_preview_text_image_and_binary_in_memory() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![
                AttachmentInput {
                    name: "note.txt".into(),
                    data: Some(BASE64.encode(b"hello attachment".as_slice())),
                },
                AttachmentInput {
                    name: "pic.png".into(),
                    data: Some(BASE64.encode([0x89u8, 0x50, 0x4E, 0x47])),
                },
                AttachmentInput {
                    name: "blob.bin".into(),
                    data: Some(BASE64.encode([0u8, 1, 2, 3])),
                },
                AttachmentInput {
                    name: "big.log".into(),
                    data: Some(BASE64.encode(vec![b'x'; 2 * 1024 * 1024 + 4])),
                },
            ],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    let text = session.attachment_preview(&uuid, "note.txt").unwrap();
    assert_eq!(text.kind, "text");
    assert_eq!(text.data, "hello attachment");
    assert!(!text.truncated);
    assert_eq!(text.size, 16);

    let image = session.attachment_preview(&uuid, "pic.png").unwrap();
    assert_eq!(image.kind, "image");
    assert!(image.data.starts_with("data:image/png;base64,"));
    assert!(!image.truncated);

    let binary = session.attachment_preview(&uuid, "blob.bin").unwrap();
    assert_eq!(binary.kind, "binary");
    assert!(binary.data.is_empty());

    let big = session.attachment_preview(&uuid, "big.log").unwrap();
    assert_eq!(big.kind, "text");
    assert!(big.truncated);
    assert_eq!(big.size, 2 * 1024 * 1024 + 4);
}

#[test]
fn import_attachment_from_temp_replaces_bytes_and_persists() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let store = AttachmentTempStore::default();
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![AttachmentInput {
                name: "note.txt".into(),
                data: Some(BASE64.encode(b"original".as_slice())),
            }],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    // The external viewer "edited" the registered temp file.
    let (token, _) = store.create("s1", "note.txt", b"edited content").unwrap();
    let data = store.read_for_session(&token, "s1").unwrap();
    let updated = session
        .import_attachment_bytes(&uuid, "note.txt", data)
        .unwrap();
    let entry = updated
        .root
        .entries
        .iter()
        .find(|e| e.uuid == uuid)
        .unwrap();
    assert_eq!(entry.attachments[0].size, b"edited content".len());
    assert_eq!(
        session.attachment_data(&uuid, "note.txt").unwrap(),
        b"edited content"
    );

    // Save + reopen keeps the imported bytes.
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    let entry = state.root.entries.iter().find(|e| e.uuid == uuid).unwrap();
    assert_eq!(entry.attachments[0].size, b"edited content".len());

    // Unknown tokens are rejected; arbitrary paths are never accepted.
    assert!(store.read_for_session("nope", "s1").is_err());

    // A valid token cannot be replayed into another open vault session even
    // when that vault happens to contain the same entry UUID.
    let (foreign_token, _) = store.create("s2", "note.txt", b"foreign").unwrap();
    assert_eq!(
        store.read_for_session(&foreign_token, "s1").unwrap_err(),
        "临时附件不属于当前数据库会话"
    );
    assert_eq!(
        session.attachment_data(&uuid, "note.txt").unwrap(),
        b"edited content"
    );
}

/// Twofish remains readable for compatibility, but the settings write DTO
/// only accepts ciphers SecPivot intentionally offers for a rewrite.
#[test]
fn database_settings_patch_rejects_twofish_write_cipher() {
    let error = serde_json::from_value::<DatabaseSettingsPatch>(serde_json::json!({
        "cipher": "Twofish"
    }))
    .unwrap_err();
    assert!(error.to_string().contains("unknown variant `Twofish`"));

    let patch = serde_json::from_value::<DatabaseSettingsPatch>(serde_json::json!({
        "cipher": "ChaCha20"
    }))
    .unwrap();
    assert_eq!(patch.cipher, Some(WritableDatabaseCipher::ChaCha20));
}

/// Invalid storage settings must not leave the live database with meta
/// changes from the same rejected patch.
#[test]
fn rejected_database_storage_patch_is_atomic() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let before = session.snapshot_without_icons().unwrap();

    let error = session
        .update_database_settings(&DatabaseSettingsPatch {
            kdf: Some("Unsupported".into()),
            history_max_items: Some(Some(7)),
            recycle_bin_enabled: Some(Some(false)),
            ..Default::default()
        })
        .unwrap_err();
    assert!(error.contains("kdf 取值"));

    let settings = session.database_settings().unwrap().unwrap();
    assert_eq!(settings.history_max_items, None);
    assert!(settings.recycle_bin_enabled);
    let after = session.snapshot_without_icons().unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.dirty, before.dirty);
}

/// An empty partial patch keeps the session unchanged instead of creating a
/// phantom unsaved edit.
#[test]
fn empty_database_settings_patch_is_a_noop() {
    let dir = TempDir::new().unwrap();
    let (mut session, _path) = create_session(&dir);
    let before = session.snapshot_without_icons().unwrap();
    let after = session
        .update_database_settings(&DatabaseSettingsPatch::default())
        .unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.dirty, before.dirty);
}

#[test]
fn emergency_sheet_includes_passwords_only_when_requested() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "GitHub".into(),
            username: "octocat".into(),
            password: "s3cret".into(),
            url: "https://github.com".into(),
            notes: "note <tag>".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();

    let without = session.emergency_sheet_content(false).unwrap();
    assert!(without.starts_with("<!doctype html>"));
    assert!(without.contains("GitHub"));
    assert!(without.contains("octocat"));
    assert!(!without.contains("s3cret"));
    assert!(!without.contains("本文件包含明文密码"));
    // Notes are HTML-escaped so a `<tag>` can never inject markup.
    assert!(without.contains("note &lt;tag&gt;"));

    let with_passwords = session.emergency_sheet_content(true).unwrap();
    assert!(with_passwords.contains("s3cret"));
    assert!(with_passwords.contains("本文件包含明文密码"));
    assert!(with_passwords.contains("<td class=\"mono\">s3cret</td>"));

    // The file-write path persists the sheet.
    let out = dir.path().join("emergency.html");
    session
        .export_emergency_sheet(&out.to_string_lossy(), true)
        .unwrap();
    let written = std::fs::read_to_string(&out).unwrap();
    assert!(written.contains("s3cret"));
}

#[test]
fn similar_passwords_clusters_edits_and_skips_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let mk = |title: &str, password: &str| EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: title.into(),
        username: "u".into(),
        password: password.into(),
        url: String::new(),
        notes: String::new(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    };
    let state = session
        .add_entries(&[
            mk("A", "Password1!"),
            mk("B", "Password2!"),
            mk("C", "TotallyDifferent9"),
            mk("D", "Password1!"),
        ])
        .unwrap();
    let uuid_of = |title: &str| {
        state
            .root
            .entries
            .iter()
            .find(|e| e.title == title)
            .unwrap()
            .uuid
            .clone()
    };
    let a = uuid_of("A");

    // A~B (one edit), D~B and D==A: transitive clustering joins A/B/D; C
    // stays separate. Passwords never appear in the DTO.
    let groups = session.similar_passwords().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].entries.len(), 3);
    let serialized = format!("{groups:?}");
    assert!(!serialized.contains("Password1!"));
    assert!(!serialized.contains("TotallyDifferent9"));

    // Deleting A moves it to the recycle bin, which is excluded.
    session.delete_entry(&a).unwrap();
    let groups = session.similar_passwords().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].entries.len(), 2);
    assert!(groups[0].entries.iter().all(|e| e.title != "A"));
}

#[test]
fn clear_all_history_wipes_every_snapshot_and_persists() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let mk = |password: &str| EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: "E".into(),
        username: "u".into(),
        password: password.into(),
        url: String::new(),
        notes: String::new(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    };
    let state = session.add_entry(&mk("pw1")).unwrap();
    let uuid = state.root.entries[0].uuid.clone();
    session.update_entry(&uuid, &mk("pw2")).unwrap();
    session.update_entry(&uuid, &mk("pw3")).unwrap();
    assert!(!session.get_entry_history(&uuid).unwrap().is_empty());

    let result = session.clear_all_history().unwrap();
    assert!(result.cleared >= 1);
    assert!(session.get_entry_history(&uuid).unwrap().is_empty());

    session.save().unwrap();
    let mut reopened = VaultSession::default();
    reopened.open(&path, "master-password", None).unwrap();
    assert!(reopened.get_entry_history(&uuid).unwrap().is_empty());
}

#[test]
fn expired_entries_lists_past_expiry_and_skips_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let mk = |title: &str, expires: Option<&str>| EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: title.into(),
        username: "u".into(),
        password: "pw".into(),
        url: String::new(),
        notes: String::new(),
        totp: None,
        expires: expires.map(str::to_owned),
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    };
    let state = session
        .add_entries(&[
            mk("Old", Some("2020-01-01T00:00:00.000Z")),
            mk("Future", Some("2099-01-01T00:00:00.000Z")),
            mk("NoExpiry", None),
        ])
        .unwrap();
    let old_uuid = state
        .root
        .entries
        .iter()
        .find(|e| e.title == "Old")
        .unwrap()
        .uuid
        .clone();

    let list = session.expired_entries().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title, "Old");
    assert!(list[0].expires.starts_with("2020-"));

    // Deleting moves the entry to the recycle bin, which is excluded.
    session.delete_entry(&old_uuid).unwrap();
    assert!(session.expired_entries().unwrap().is_empty());
}

#[test]
fn probe_vault_classifies_headers_and_open_rejects_non_kdbx() {
    let dir = TempDir::new().unwrap();
    let garbage = dir.path().join("garbage.kdbx");
    std::fs::write(&garbage, b"this is not a vault").unwrap();

    let probe = super::helpers::probe_vault(&garbage).unwrap();
    assert_eq!(probe.kind, "unknown");
    assert!(probe.note.contains("不是 KeePass"));

    // Opening a non-KDBX file fails fast with the actionable message.
    let err = super::prepare_local_open(&garbage, "pw", None).unwrap_err();
    assert!(
        err.contains("不是 KeePass 数据库"),
        "unexpected error: {err}"
    );

    // A real vault probes as kdbx and still opens normally.
    let (session, path) = create_session(&dir);
    let probe = super::helpers::probe_vault(&path).unwrap();
    assert_eq!(probe.kind, "kdbx");
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    assert!(session.is_open());
}

#[test]
fn classify_open_error_suggests_xml_recovery_for_parse_failures() {
    let key_err = super::helpers::classify_open_error("invalid HMAC");
    assert!(key_err.contains("密码或密钥文件错误"));
    assert!(!key_err.contains("导入 XML"));

    let parse_err = super::helpers::classify_open_error("unsupported file version");
    assert!(parse_err.contains("无法打开数据库"));
    assert!(parse_err.contains("导入 XML"));
}

#[test]
fn repeated_save_failures_degrade_to_read_only_and_save_as_recovers() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    assert!(!session.is_read_only());

    // Make the save target unwritable by removing its parent directory.
    std::fs::remove_dir_all(dir.path()).unwrap();
    for _ in 0..3 {
        assert!(session.save().is_err());
    }

    assert!(session.is_read_only());
    assert!(session.state().unwrap().unwrap().read_only);
    // Save and master-key changes are refused while read-only.
    assert!(session.save().unwrap_err().contains("只读模式"));
    assert!(session
        .change_master_key("new-master", None)
        .unwrap_err()
        .contains("只读模式"));

    // Save-as stays available as the recovery path and resets the counter.
    std::fs::create_dir_all(dir.path()).unwrap();
    let recovered = dir.path().join("recovered.kdbx");
    session.save_as(&recovered).unwrap();
    assert!(!session.is_read_only());
    assert!(!session.state().unwrap().unwrap().read_only);
    assert!(session.save().is_ok());
}

#[test]
fn protected_custom_fields_never_leak_in_snapshot() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();

    // Add an entry with a protected PIN plus an unprotected public field.
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_uuid.clone(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![
                CustomField {
                    name: "PIN".into(),
                    value: "s3cret-pin".into(),
                    protected: true,
                },
                CustomField {
                    name: "Tag".into(),
                    value: "public".into(),
                    protected: false,
                },
            ],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.children[0].entries[0].uuid.clone();

    // The snapshot must carry the flag but never the protected value.
    let pin = state.root.children[0].entries[0]
        .custom_fields
        .iter()
        .find(|f| f.name == "PIN")
        .expect("PIN present");
    assert!(pin.protected);
    assert!(
        pin.value.is_empty(),
        "protected value must not reach the snapshot"
    );
    let public = state.root.children[0].entries[0]
        .custom_fields
        .iter()
        .find(|f| f.name == "Tag")
        .expect("Tag present");
    assert!(!public.protected);
    assert_eq!(public.value, "public");

    // On-demand access resolves the real value.
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "PIN")
            .unwrap()
            .as_deref(),
        Some("s3cret-pin")
    );
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Tag")
            .unwrap()
            .as_deref(),
        Some("public")
    );
    assert_eq!(
        session.get_custom_field_value(&uuid, "Missing").unwrap(),
        None,
        "unknown field resolves to None"
    );
    assert_eq!(
        session.get_custom_field_value(&uuid, "Title").unwrap(),
        None,
        "reserved columns are not custom fields"
    );

    // Persist and reopen: the protected flag survives and the value stays
    // out of the snapshot while remaining readable on demand.
    session.save().unwrap();
    drop(session);
    let mut reopened = VaultSession::default();
    let state = reopened
        .open(&dir.path().join("test.kdbx"), "master-password", None)
        .unwrap();
    let entry = &state.root.children[0].entries[0];
    let pin = entry
        .custom_fields
        .iter()
        .find(|f| f.name == "PIN")
        .unwrap();
    assert!(pin.protected);
    assert!(pin.value.is_empty());
    assert_eq!(
        reopened
            .get_custom_field_value(&entry.uuid, "PIN")
            .unwrap()
            .as_deref(),
        Some("s3cret-pin")
    );
}

#[test]
fn protected_custom_fields_round_trip_and_history() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();

    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_uuid.clone(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![CustomField {
                name: "Secret".into(),
                value: "hunter2".into(),
                protected: true,
            }],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.children[0].entries[0].uuid.clone();

    // Editing a protected field keeps its value readable on demand and writes
    // it back as protected; unprotected custom fields stay unprotected.
    let state = session
        .update_entry(
            &uuid,
            &EntryInput {
                group_uuid: group_uuid.clone(),
                title: "E".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![
                    CustomField {
                        name: "Secret".into(),
                        value: "hunter2".into(),
                        protected: true,
                    },
                    CustomField {
                        name: "Public".into(),
                        value: "hello".into(),
                        protected: false,
                    },
                ],
                attachments: vec![],
            },
        )
        .unwrap();
    let entry = &state.root.children[0].entries[0];
    assert_eq!(entry.custom_fields.len(), 2);
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Secret")
            .unwrap()
            .as_deref(),
        Some("hunter2")
    );
    let secret = entry
        .custom_fields
        .iter()
        .find(|f| f.name == "Secret")
        .unwrap();
    assert!(secret.protected && secret.value.is_empty());

    // History snapshots keep the value server-side for restore (never shown
    // in the UI), and a restore brings the protected field back intact.
    let history = session.get_entry_history(&uuid).unwrap();
    assert!(!history.is_empty());
    let old_secret = history
        .last()
        .expect("a prior snapshot exists")
        .custom_fields
        .iter()
        .find(|f| f.name == "Secret")
        .expect("history keeps the custom field");
    assert!(old_secret.protected);
    assert_eq!(old_secret.value, "hunter2");
    session
        .restore_entry_version(&uuid, history.len() - 1)
        .unwrap();
    let restored = session.snapshot().unwrap();
    let entry = &restored
        .root
        .children
        .iter()
        .find(|g| g.uuid == group_uuid)
        .unwrap()
        .entries[0];
    let secret = entry
        .custom_fields
        .iter()
        .find(|f| f.name == "Secret")
        .unwrap();
    assert!(secret.protected);
    assert_eq!(
        session
            .get_custom_field_value(&entry.uuid, "Secret")
            .unwrap()
            .as_deref(),
        Some("hunter2")
    );
}

#[test]
fn custom_fields_exclude_reserved_names() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid,
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "n".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![
                CustomField {
                    name: FIELD_OTP.to_owned(),
                    value: "should-not-appear".into(),
                    protected: false,
                },
                CustomField {
                    name: FIELD_TITLE.to_owned(),
                    value: "should-not-appear".into(),
                    protected: false,
                },
                CustomField {
                    name: "   ".into(),
                    value: "ignored".into(),
                    protected: false,
                },
                CustomField {
                    name: "Nickname".into(),
                    value: "alice".into(),
                    protected: false,
                },
            ],
            attachments: vec![],
        })
        .unwrap();
    let entry = &state.root.children[0].entries[0];
    assert_eq!(entry.custom_fields.len(), 1);
    assert_eq!(entry.custom_fields[0].name, "Nickname");
}

#[test]
fn save_attachment_writes_file() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let payload = b"\x00\x01binary data\xff".to_vec();
    let state = session
        .add_entry(&EntryInput {
            group_uuid,
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![AttachmentInput {
                name: "blob.bin".into(),
                data: Some(BASE64.encode(payload.clone())),
            }],
        })
        .unwrap();
    let uuid = state.root.children[0].entries[0].uuid.clone();
    let dest = dir.path().join("out.bin");
    session
        .save_attachment(&uuid, "blob.bin", dest.to_str().unwrap())
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
}

#[test]
fn keyfile_round_trip_requires_the_keyfile() {
    let dir = TempDir::new().unwrap();
    let keyfile = write_keyfile(&dir);
    let path = dir.path().join("secured.kdbx");
    let mut session = VaultSession::default();
    session
        .create(
            &path,
            "master-password",
            "Aes",
            "Aes256",
            "None",
            Some(&keyfile),
        )
        .unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    let err = reopened.open(&path, "master-password", None).unwrap_err();
    assert!(err.contains("密码"), "unexpected error: {err}");

    let mut reopened = VaultSession::default();
    let state = reopened
        .open(&path, "master-password", Some(&keyfile))
        .unwrap();
    assert!(reopened.is_open());
    assert_eq!(state.root.name, "Root");

    let mut session = reopened;
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap();
    session
        .add_entry(&EntryInput {
            group_uuid: state.root.children[0].uuid.clone(),
            title: "Inbox".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    session.save().unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened
        .open(&path, "master-password", Some(&keyfile))
        .unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "Mail");
}

#[test]
fn keyfile_only_database_opens_without_password() {
    let dir = TempDir::new().unwrap();
    let keyfile = write_keyfile(&dir);
    let path = dir.path().join("keyonly.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "", "Aes", "Aes256", "None", Some(&keyfile))
        .unwrap();
    drop(session);

    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "", Some(&keyfile)).unwrap();
    assert!(reopened.is_open());
    assert_eq!(state.root.name, "Root");
    let err = reopened.open(&path, "anything", None).unwrap_err();
    assert!(err.contains("密码"), "unexpected error: {err}");
}

#[test]
fn missing_keyfile_path_is_rejected() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("nope.key");
    let path = dir.path().join("x.kdbx");
    let err = VaultSession::default()
        .open(&path, "pw", Some(&missing))
        .unwrap_err();
    assert!(err.contains("密钥文件"), "unexpected error: {err}");
}

fn add_entry_with_password(
    session: &mut VaultSession,
    group_uuid: &str,
    title: &str,
    password: &str,
) -> String {
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_uuid.to_owned(),
            title: title.into(),
            username: "u".into(),
            password: password.into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    state.root.children[0].entries.last().unwrap().uuid.clone()
}

#[test]
fn get_entry_password_returns_field_on_demand() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let uuid = add_entry_with_password(&mut session, &group_uuid, "E", "hunter2");
    assert_eq!(session.get_entry_password(&uuid).unwrap(), "hunter2");
    let err = session.get_entry_password("not-a-uuid").unwrap_err();
    assert!(err.contains("UUID"));
}

#[test]
fn security_report_flags_empty_weak_and_duplicate_passwords() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();

    let empty_uuid = add_entry_with_password(&mut session, &group_uuid, "Empty", "");
    let weak_uuid = add_entry_with_password(&mut session, &group_uuid, "Weak", "abc");
    let strong_pw = "StrongPass#1!";
    let dup_a = add_entry_with_password(&mut session, &group_uuid, "DupA", strong_pw);
    let dup_b = add_entry_with_password(&mut session, &group_uuid, "DupB", strong_pw);

    let report = session.security_report().unwrap();
    assert_eq!(report.total, 4);
    assert_eq!(report.empty, vec![empty_uuid]);
    assert_eq!(
        report.weak,
        vec![WeakEntry {
            uuid: weak_uuid,
            bits: 14
        }]
    );
    assert_eq!(report.duplicates.len(), 1);
    assert_eq!(report.duplicates[0].count, 2);
    assert!(report.duplicates[0].uuids.contains(&dup_a));
    assert!(report.duplicates[0].uuids.contains(&dup_b));
}

#[test]
fn security_report_skips_entries_with_quality_check_disabled() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();

    // Same strong password shared by a checked and an unchecked entry so the
    // duplicate finding (reuse is independent of quality checking) still fires.
    let strong_pw = "StrongPass#1!";
    let weak_uuid = add_entry_with_password(&mut session, &group_uuid, "Weak", "abc");
    let empty_uuid = add_entry_with_password(&mut session, &group_uuid, "Empty", "");
    let dup_a = add_entry_with_password(&mut session, &group_uuid, "DupA", strong_pw);
    let dup_b = add_entry_with_password(&mut session, &group_uuid, "DupB", strong_pw);

    // Disable quality checking on the weak, empty, and one duplicate entry.
    for uuid in [&weak_uuid, &empty_uuid, &dup_b] {
        let db = session.require_db_mut().unwrap();
        let mut entry = db
            .entry_mut(parse_entry_id(uuid).unwrap())
            .expect("entry must exist");
        entry.quality_check = false;
    }

    let report = session.security_report().unwrap();
    // total still counts every entry, quality-checked or not.
    assert_eq!(report.total, 4);
    // The unchecked weak + empty entries vanish from their findings...
    assert!(!report.weak.iter().any(|w| w.uuid == weak_uuid));
    assert!(!report.empty.contains(&empty_uuid));
    // ...but duplicates still include the unchecked entry (reuse check stays).
    assert_eq!(report.duplicates.len(), 1);
    assert_eq!(report.duplicates[0].count, 2);
    assert!(report.duplicates[0].uuids.contains(&dup_a));
    assert!(report.duplicates[0].uuids.contains(&dup_b));

    // A write op bumps the revision so the snapshot below is rebuilt.
    session
        .update_entry(
            &dup_a,
            &EntryInput {
                group_uuid: group_uuid.clone(),
                title: "DupA".into(),
                username: "u".into(),
                password: strong_pw.into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();

    // The snapshot exposes the per-entry quality-check flag.
    let state = session.snapshot().unwrap();
    let by_uuid: HashMap<_, _> = state.root.children[0]
        .entries
        .iter()
        .map(|e| (e.uuid.clone(), e))
        .collect();
    assert!(!by_uuid[&weak_uuid].quality_check);
    assert!(!by_uuid[&empty_uuid].quality_check);
    assert!(!by_uuid[&dup_b].quality_check);
    assert!(by_uuid[&dup_a].quality_check);
}

#[test]
fn custom_data_round_trips_through_edit_and_save() {
    use keepass::db::{CustomDataItem, CustomDataValue};
    use std::collections::HashMap as StdHashMap;

    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "G".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_uuid.clone(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let entry_uuid = state.root.children[0].entries[0].uuid.clone();
    let group_id = super::helpers::parse_group_id(&group_uuid).unwrap();
    let entry_id = parse_entry_id(&entry_uuid).unwrap();

    // Seed plugin-style CustomData on the entry, its group, and the database
    // meta — as another KeePass client (e.g. KeePassRPC) would.
    {
        let db = session.require_db_mut().unwrap();
        let mut entry = db.entry_mut(entry_id).expect("entry must exist");
        let mut map = StdHashMap::new();
        map.insert(
            "plugin.binary".into(),
            CustomDataItem {
                value: Some(CustomDataValue::Binary(vec![1, 2, 3])),
                last_modification_time: None,
            },
        );
        entry.custom_data = map;
        let mut group = db.group_mut(group_id).expect("group must exist");
        let mut map = StdHashMap::new();
        map.insert(
            "grp.key".into(),
            CustomDataItem {
                value: Some(CustomDataValue::String("v".into())),
                last_modification_time: None,
            },
        );
        group.custom_data = map;
        let mut map = StdHashMap::new();
        map.insert(
            "meta.key".into(),
            CustomDataItem {
                value: Some(CustomDataValue::String("m".into())),
                last_modification_time: None,
            },
        );
        db.meta.custom_data = map;
    }

    // A normal SecPivot edit (bumps the session revision and rebuilds the
    // snapshot) must not clobber the CustomData at any level.
    session
        .update_entry(
            &entry_uuid,
            &EntryInput {
                group_uuid: group_uuid.clone(),
                title: "E2".into(),
                username: "u2".into(),
                password: "pw2".into(),
                url: "https://x.example".into(),
                notes: "n".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();

    // The snapshot exposes all three levels, read-only.
    let state = session.snapshot().unwrap();
    assert_eq!(state.meta_custom_data.len(), 1);
    assert_eq!(state.meta_custom_data[0].key, "meta.key");
    assert_eq!(state.meta_custom_data[0].value.as_deref(), Some("m"));
    let group = &state.root.children[0];
    assert_eq!(group.custom_data.len(), 1);
    assert_eq!(group.custom_data[0].key, "grp.key");
    let entry = &group.entries[0];
    assert_eq!(entry.custom_data.len(), 1);
    assert_eq!(entry.custom_data[0].key, "plugin.binary");
    assert_eq!(
        entry.custom_data[0].binary.as_deref(),
        Some(BASE64.encode([1u8, 2, 3]).as_str())
    );

    session.save().unwrap();
    drop(session);

    // Save + reopen: CustomData survives at every level.
    let mut reopened = VaultSession::default();
    let _ = reopened
        .open(&dir.path().join("test.kdbx"), "master-password", None)
        .unwrap();
    let state = reopened.snapshot().unwrap();
    assert_eq!(state.meta_custom_data.len(), 1);
    assert_eq!(state.meta_custom_data[0].value.as_deref(), Some("m"));
    let group = &state.root.children[0];
    assert_eq!(group.custom_data.len(), 1);
    assert_eq!(group.custom_data[0].value.as_deref(), Some("v"));
    let entry = &group.entries[0];
    assert_eq!(entry.title, "E2");
    assert_eq!(entry.custom_data.len(), 1);
    assert_eq!(
        entry.custom_data[0].binary.as_deref(),
        Some(BASE64.encode([1u8, 2, 3]).as_str())
    );
}

#[test]
fn foreign_attributes_survive_edits_and_flags_round_trip() {
    use keepass::db::{CustomDataItem, CustomDataValue};
    use std::collections::HashMap as StdHashMap;

    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "G".into(),
            icon: None,
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_uuid.clone(),
            title: "E".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "https://stored.example".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let entry_uuid = state.root.children[0].entries[0].uuid.clone();
    let group_id = super::helpers::parse_group_id(&group_uuid).unwrap();
    let entry_id = parse_entry_id(&entry_uuid).unwrap();

    // Seed attributes as another KeePass client would: entry CustomData,
    // OverrideURL, disabled quality check, foreground+background colors,
    // plus group CustomData/notes/tags.
    {
        let db = session.require_db_mut().unwrap();
        let mut entry = db.entry_mut(entry_id).expect("entry must exist");
        let mut map = StdHashMap::new();
        map.insert(
            "plugin.binary".into(),
            CustomDataItem {
                value: Some(CustomDataValue::Binary(vec![4, 5, 6])),
                last_modification_time: None,
            },
        );
        entry.custom_data = map;
        entry.override_url = Some("https://real.example".into());
        entry.quality_check = false;
        entry.background_color = Some("#00FF00".parse().unwrap());
        entry.foreground_color = Some("#AABBCC".parse().unwrap());
        let mut group = db.group_mut(group_id).expect("group must exist");
        let mut map = StdHashMap::new();
        map.insert(
            "grp.key".into(),
            CustomDataItem {
                value: Some(CustomDataValue::String("v".into())),
                last_modification_time: None,
            },
        );
        group.custom_data = map;
        group.notes = Some("foreign note".into());
        group.tags = vec!["legacy".into()];
    }

    // SecPivot edits the entry fields and then manages the flags/colors.
    session
        .update_entry(
            &entry_uuid,
            &EntryInput {
                group_uuid: group_uuid.clone(),
                title: "E2".into(),
                username: "u2".into(),
                password: "pw2".into(),
                url: "https://stored.example".into(),
                notes: "edited".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: Some("#00FF00".into()),
                tags: Some("work".into()),
                custom_fields: vec![],
                attachments: vec![],
            },
        )
        .unwrap();
    session
        .update_entry_flags(
            &entry_uuid,
            Some("https://real.example".into()),
            Some(true),
            Some("#112233".into()),
        )
        .unwrap();
    session
        .update_group_meta(
            &group_uuid,
            Some("managed note".into()),
            Some("dev".into()),
            Some(false),
        )
        .unwrap();

    session.save().unwrap();
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();

    // Entry: field edits applied, flags/colors persisted, CustomData intact.
    let entry = state.root.children[0].entries[0].clone();
    assert_eq!(entry.title, "E2");
    assert_eq!(entry.notes, "edited");
    assert_eq!(entry.tags.as_deref(), Some("work"));
    assert_eq!(entry.color.as_deref(), Some("#00FF00"));
    assert_eq!(entry.override_url.as_deref(), Some("https://real.example"));
    assert_eq!(entry.foreground_color.as_deref(), Some("#112233"));
    assert!(entry.quality_check);
    let binary = entry
        .custom_data
        .iter()
        .find(|item| item.key == "plugin.binary")
        .unwrap();
    assert_eq!(binary.binary.as_deref(), Some("BAUG"));

    // Group: managed meta persisted and group CustomData survived.
    let group = &state.root.children[0];
    assert_eq!(group.notes.as_deref(), Some("managed note"));
    assert_eq!(group.tags.as_deref(), Some("dev"));
    assert!(!group.enable_searching);
    assert!(group
        .custom_data
        .iter()
        .any(|item| item.key == "grp.key" && item.value.as_deref() == Some("v")));
}

#[test]
fn export_csv_writes_escaped_rows_and_bom() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Web".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    session
        .add_entry(&EntryInput {
            group_uuid,
            title: "Git,Hub".into(),
            username: "alice".into(),
            password: "s3cret".into(),
            url: "https://x".into(),
            notes: "line1\nline2".into(),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();

    let dest = dir.path().join("export.csv");
    session.export_csv(dest.to_str().unwrap()).unwrap();
    let text = std::fs::read_to_string(&dest).unwrap();

    assert!(text.starts_with('\u{FEFF}'));
    assert!(text.contains("Group,Title,Username,Password,URL,Notes,TOTP,Favorite\r\n"));
    assert!(text.contains("\"Git,Hub\""));
    assert!(text.contains("\"line1\nline2\""));
    assert!(text.contains("Web,\"Git,Hub\",alice,s3cret,https://x"));
    assert!(text.contains("JBSWY3DPEHPK3PXP"));
}

/// Create a local vault and seed it into an in-memory remote storage.
fn seed_remote_storage(dir: &TempDir) -> (crate::remote::MemoryStorage, std::path::PathBuf) {
    let seed_path = dir.path().join("seed.kdbx");
    {
        let mut session = VaultSession::default();
        session
            .create(&seed_path, "pw", "Aes", "Aes256", "None", None)
            .unwrap();
    }
    let storage = crate::remote::MemoryStorage::default();
    storage.seed("vaults/seed.kdbx", std::fs::read(&seed_path).unwrap());
    (storage, seed_path)
}

#[test]
fn remote_open_save_round_trip_via_memory_storage() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("local");

    let mut session = VaultSession::default();
    let state = session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert_eq!(state.path, "s3://vaults/seed.kdbx");
    assert_eq!(state.file_name, "seed.kdbx");

    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Web".into(),
        })
        .unwrap();
    session
        .add_entry(&EntryInput {
            group_uuid: state.root.children[0].uuid.clone(),
            title: "Site".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let saved = session.save().unwrap();
    assert!(!saved.dirty);

    let mut reopened = VaultSession::default();
    let state = reopened
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "Web");
    assert_eq!(state.root.children[0].entries.len(), 1);
}

#[test]
fn remote_save_detects_external_change_and_conflicts_do_not_trigger_read_only() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("local");

    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Local".into(),
            username: "u".into(),
            password: "pw".into(),
            url: String::new(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();

    // Another device overwrites the remote file since we opened it.
    let original = storage.get("vaults/seed.kdbx").unwrap();
    storage.put("vaults/seed.kdbx", &[0u8; 64]).unwrap();
    let err = session.save().unwrap_err();
    assert!(err.starts_with("REMOTE_CHANGED"), "unexpected: {err}");
    assert!(
        !session.is_read_only(),
        "conflicts must not degrade the session to read-only"
    );
    assert!(session.state().unwrap().unwrap().dirty);

    // Restoring the remote bytes makes the save succeed and advances the
    // base hash, so a second external change is detected again.
    storage.put("vaults/seed.kdbx", &original).unwrap();
    let saved = session.save().unwrap();
    assert!(!saved.dirty);
    storage.put("vaults/seed.kdbx", &[1u8; 64]).unwrap();
    let err = session.save().unwrap_err();
    assert!(err.starts_with("REMOTE_CHANGED"), "unexpected: {err}");
}

#[test]
fn remote_conflict_resolution_force_save_and_refresh() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("local");

    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    let revision_before_local_edit = session.state().unwrap().unwrap().revision;
    session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Local".into(),
            username: "u".into(),
            password: "pw".into(),
            url: String::new(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let original = storage.get("vaults/seed.kdbx").unwrap();
    storage.put("vaults/seed.kdbx", &[7u8; 32]).unwrap();
    assert!(session.save().unwrap_err().starts_with("REMOTE_CHANGED"));

    // 覆盖远程: force save overwrites and advances the base hash.
    let job = session.prepare_save(true).unwrap();
    let revision = job.revision;
    let new_hash = persist_save(job).unwrap();
    session.complete_save(revision, new_hash).unwrap();
    let remote_now = storage.get("vaults/seed.kdbx").unwrap();
    assert_ne!(remote_now.as_slice(), [7u8; 32]);
    storage.put("vaults/seed.kdbx", &[9u8; 32]).unwrap();
    assert!(session.save().unwrap_err().starts_with("REMOTE_CHANGED"));

    // 下载远程: refresh replaces the session (local edit discarded) and
    // advances the base hash so the next save succeeds.
    storage.put("vaults/seed.kdbx", &original).unwrap();
    let job = session.prepare_remote_refresh().unwrap();
    let revision = job.revision;
    let refreshed = session
        .complete_remote_refresh(revision, persist_remote_refresh(job).unwrap())
        .unwrap();
    assert!(refreshed.revision > revision_before_local_edit);
    assert!(!refreshed.dirty);
    assert!(
        !refreshed.root.entries.iter().any(|e| e.title == "Local"),
        "refresh must discard the local unsaved edit"
    );
    assert!(
        session.save().is_ok(),
        "base hash must advance after refresh"
    );
}

#[test]
fn remote_refresh_completion_rejects_edits_landed_during_download() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("local");
    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();

    let job = session.prepare_remote_refresh().unwrap();
    let revision = job.revision;
    let downloaded = persist_remote_refresh(job).unwrap();
    session
        .add_entry(&merge_test_input("EditDuringRefresh"))
        .unwrap();

    let err = session
        .complete_remote_refresh(revision, downloaded)
        .unwrap_err();
    assert!(err.contains("已发生修改"));
    let state = session.state().unwrap().unwrap();
    assert!(state.dirty);
    assert!(state
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "EditDuringRefresh"));
}

#[test]
fn remote_save_local_writes_mirror_and_rotates_backups() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("mirror");

    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::SaveLocal,
            &local,
            1,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert!(local.join("seed.kdbx").exists());

    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap();
    session.save().unwrap();
    session.save().unwrap();
    session.save().unwrap();

    let bytes = std::fs::read(local.join("seed.kdbx")).unwrap();
    let key = DatabaseKey::new().with_password("pw");
    let db = Database::parse(&bytes, key).unwrap();
    assert_eq!(db.root().groups().count(), 1);

    let backups: Vec<_> = std::fs::read_dir(&local)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".kdbx.bak"))
        .collect();
    assert_eq!(backups.len(), 1);
}

#[test]
fn remote_backup_uses_custom_template_and_prunes_by_shape() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("mirror");
    const TEMPLATE: &str = "{name}-backup-{timestamp}.{ext}.old";

    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::SaveLocal,
            &local,
            2,
            TEMPLATE,
        )
        .unwrap();
    assert!(local.join("seed.kdbx").exists());

    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Mail".into(),
        })
        .unwrap();
    session.save().unwrap();
    session.save().unwrap();
    session.save().unwrap();

    let backups: Vec<_> = std::fs::read_dir(&local)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("seed-backup-") && name.ends_with(".kdbx.old"))
        .collect();
    assert_eq!(backups.len(), 2, "keeps only the newest two");
    for name in &backups {
        assert!(
            !name.ends_with(".bak"),
            "custom template must shape the backup name: {name}"
        );
    }
    let old_style: Vec<_> = std::fs::read_dir(&local)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".kdbx.bak"))
        .collect();
    assert!(old_style.is_empty(), "no default-template backups");
}

#[test]
fn opening_local_vault_clears_stale_remote_target() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("local");

    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();

    let local_path = dir.path().join("local.kdbx");
    session
        .create(&local_path, "pw", "Aes", "Aes256", "None", None)
        .unwrap();
    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "LocalOnly".into(),
        })
        .unwrap();
    session.save().unwrap();

    let remote_bytes = storage.get("vaults/seed.kdbx").unwrap();
    let remote_db = Database::parse(&remote_bytes, DatabaseKey::new().with_password("pw")).unwrap();
    assert_eq!(
        remote_db.root().groups().count(),
        0,
        "remote target must not receive local vault data"
    );

    let local_bytes = std::fs::read(&local_path).unwrap();
    let local_db = Database::parse(&local_bytes, DatabaseKey::new().with_password("pw")).unwrap();
    assert_eq!(local_db.root().groups().count(), 1);
}

#[test]
fn add_entry_with_invalid_attachment_does_not_partially_commit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "pw", "Aes", "Aes256", "None", None)
        .unwrap();

    let err = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Bad".into(),
            username: "u".into(),
            password: "p".into(),
            url: String::new(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![AttachmentInput {
                name: "a.bin".into(),
                data: Some("!!!not-base64!!!".into()),
            }],
        })
        .unwrap_err();
    assert!(err.contains("附件数据解码失败"));

    let state = session.state().unwrap().unwrap();
    assert!(state.root.entries.is_empty(), "no entry must be committed");
    assert!(!state.dirty, "dirty must not be set after a failed add");
}

#[test]
fn add_entries_batches_multiple_rows_in_one_transaction() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "pw", "Aes", "Aes256", "None", None)
        .unwrap();

    let inputs: Vec<EntryInput> = vec![
        entry_input(ROOT_GROUP_UUID, "A", "u1", "p1", "https://a"),
        entry_input(ROOT_GROUP_UUID, "B", "u2", "p2", "https://b"),
        entry_input(ROOT_GROUP_UUID, "C", "u3", "p3", "https://c"),
    ];
    let state = session.add_entries(&inputs).unwrap();
    assert_eq!(state.root.entries.len(), 3);
    assert!(state.dirty);
    assert_eq!(session.state().unwrap().unwrap().root.entries.len(), 3);
}

#[test]
fn add_entries_empty_input_is_a_noop() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "pw", "Aes", "Aes256", "None", None)
        .unwrap();
    let state = session.add_entries(&[]).unwrap();
    assert!(state.root.entries.is_empty());
    assert!(!state.dirty);
}

#[test]
fn add_entries_aborts_whole_batch_on_bad_attachment() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "pw", "Aes", "Aes256", "None", None)
        .unwrap();

    let inputs: Vec<EntryInput> = vec![
        entry_input(ROOT_GROUP_UUID, "Good", "u", "p", "https://ok"),
        EntryInput {
            attachments: vec![AttachmentInput {
                name: "a.bin".into(),
                data: Some("!!!not-base64!!!".into()),
            }],
            ..entry_input(ROOT_GROUP_UUID, "Bad", "u", "p", "https://bad")
        },
    ];
    let err = session.add_entries(&inputs).unwrap_err();
    assert!(err.contains("附件数据解码失败"));
    let state = session.state().unwrap().unwrap();
    assert!(state.root.entries.is_empty(), "no entry must be committed");
    assert!(!state.dirty);
}

#[test]
fn update_entry_with_invalid_attachment_keeps_original_and_history() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "pw", "Aes", "Aes256", "None", None)
        .unwrap();
    let added = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Original".into(),
            username: "u".into(),
            password: "p".into(),
            url: String::new(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let uuid = &added.root.entries[0].uuid;

    let bad = EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: "Rewritten".into(),
        username: "u".into(),
        password: "p".into(),
        url: String::new(),
        notes: String::new(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![AttachmentInput {
            name: "a.bin".into(),
            data: Some("@@@".into()),
        }],
    };
    let err = session.update_entry(uuid, &bad).unwrap_err();
    assert!(err.contains("附件数据解码失败"));

    let state = session.state().unwrap().unwrap();
    assert_eq!(
        state.root.entries[0].title, "Original",
        "title must be unchanged"
    );
    let history = session.get_entry_history(uuid).unwrap();
    assert!(history.is_empty(), "history must not be polluted");
}

#[test]
fn remote_rejects_invalid_key_or_mode() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("local");
    let mut session = VaultSession::default();

    let err = session
        .open_remote(
            Arc::new(storage.clone()),
            "  /  ",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap_err();
    assert!(err.contains("Key"));

    let err = session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.txt",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap_err();
    assert!(err.contains("下载"));

    let err = session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/missing.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap_err();
    assert!(err.contains("下载"));

    let err = RemoteMode::parse("cloud").unwrap_err();
    assert!(err.contains("模式"));
    assert_eq!(RemoteMode::parse("memory").unwrap(), RemoteMode::InMemory);
    assert_eq!(RemoteMode::parse("local").unwrap(), RemoteMode::SaveLocal);
}

#[test]
fn remote_opens_database_under_any_key_name() {
    let dir = TempDir::new().unwrap();
    let (storage, seed_path) = seed_remote_storage(&dir);
    let storage = Arc::new(storage);
    let local = dir.path().join("local");
    let seed_bytes = std::fs::read(&seed_path).unwrap();

    // A valid database under a key WITHOUT a `.kdbx` suffix opens normally.
    storage.seed("vaults/backup-noext", seed_bytes.clone());
    let mut session = VaultSession::default();
    let state = session
        .open_remote(
            storage.clone(),
            "vaults/backup-noext",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert_eq!(state.path, "s3://vaults/backup-noext");

    // A non-database object under any key name fails at parse with a clear error.
    storage.seed("vaults/notes", b"not a kdbx at all".to_vec());
    let err = session
        .open_remote(
            storage.clone(),
            "vaults/notes",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap_err();
    assert!(err.contains("无法打开数据库"));
}

#[test]
fn remote_create_uploads_and_saves_back() {
    let storage = crate::remote::MemoryStorage::default();
    let dir = TempDir::new().unwrap();
    let local = dir.path().join("local");

    let mut session = VaultSession::default();
    let state = session
        .create_remote(
            Arc::new(storage.clone()),
            "new/vault.kdbx",
            "pw",
            "Aes",
            "Aes256",
            "None",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert_eq!(state.path, "s3://new/vault.kdbx");
    assert!(storage.get("new/vault.kdbx").is_ok());

    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Web".into(),
        })
        .unwrap();
    session
        .add_entry(&EntryInput {
            group_uuid: state.root.children[0].uuid.clone(),
            title: "Site".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    session.save().unwrap();

    let mut reopened = VaultSession::default();
    let state = reopened
        .open_remote(
            Arc::new(storage.clone()),
            "new/vault.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert_eq!(state.root.children[0].name, "Web");
    assert_eq!(state.root.children[0].entries.len(), 1);
}

#[test]
fn remote_close_clears_session_and_storage() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("local");

    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    session.close();
    assert!(!session.is_open());
    assert!(session.state().unwrap().is_none());
}

/// The wipe helpers must zero the *heap* bytes of a secret before it is
/// dropped, not just replace the logical value with an empty string.
/// Read the allocation through a raw pointer while it is still alive.
#[test]
fn wipe_helpers_zero_heap_bytes() {
    let mut password = String::from("master-password-123");
    let p_ptr = password.as_mut_ptr();
    let p_len = password.len();
    wipe_secret_string(&mut password);
    let p_bytes = unsafe { std::slice::from_raw_parts(p_ptr, p_len) };
    assert!(p_bytes.iter().all(|&b| b == 0));

    let mut keyfile = Vec::from("keyfile-bytes");
    let k_ptr = keyfile.as_mut_ptr();
    let k_len = keyfile.len();
    wipe_secret_bytes(&mut keyfile);
    let k_bytes = unsafe { std::slice::from_raw_parts(k_ptr, k_len) };
    assert!(k_bytes.iter().all(|&b| b == 0));
}

/// `close` must clear the master password and keyfile from the session
/// (the wipe itself is covered by `wipe_helpers_zero_heap_bytes`).
#[test]
fn close_clears_stored_secrets() {
    let dir = TempDir::new().unwrap();
    let keyfile = write_keyfile(&dir);
    let path = dir.path().join("test.kdbx");
    let mut session = VaultSession::default();
    session
        .create(
            &path,
            "master-password",
            "Aes",
            "Aes256",
            "None",
            Some(&keyfile),
        )
        .unwrap();
    assert!(session.password.is_some());
    assert!(session.keyfile.is_some());
    session.close();
    assert!(session.password.is_none());
    assert!(session.keyfile.is_none());
    assert!(!session.is_open());
}

#[test]
fn autotype_match_ranks_url_host_above_title_and_skips_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let entry = |title: &str, url: &str| EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: title.into(),
        username: "u".into(),
        password: "p".into(),
        url: url.into(),
        notes: "".into(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    };
    let state = session
        .add_entry(&entry("GitHub", "https://github.com"))
        .unwrap();
    let github = state.root.entries[0].uuid.clone();
    session
        .add_entry(&entry("GitHub", "https://example.com"))
        .unwrap();
    session
        .add_entry(&entry("Notebook", "https://notes.dev"))
        .unwrap();

    // URL host wins over title when both match.
    assert_eq!(
        session
            .autotype_match("GitHub - Home · github.com")
            .unwrap(),
        github
    );
    // Title-only match still works.
    assert_eq!(
        session.autotype_match("Log in to Notebook").unwrap().len(),
        36
    );
    // No match.
    let err = session.autotype_match("Random app").unwrap_err();
    assert!(err.contains("没有找到匹配"));

    // Trashed entries are never matched.
    let trash = session.delete_entry(&github).unwrap();
    let bin = &trash.root.children[0];
    assert_eq!(bin.name, "回收站");
    session
        .add_entry(&EntryInput {
            group_uuid: bin.uuid.clone(),
            title: "Trashy".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://github.com".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let err = session.autotype_match("Trashy dashboard").unwrap_err();
    assert!(err.contains("没有找到匹配"));
}

#[test]
fn autotype_match_skips_groups_with_searching_disabled() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    // Group A is normally searchable; its entry matches the window title.
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "A".into(),
            icon: None,
        })
        .unwrap();
    let group_a = state.root.children[0].uuid.clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_a.clone(),
            title: "Secret".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://secret.example".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let secret = state.root.children[0].entries[0].uuid.clone();
    assert_eq!(
        session.autotype_match("Secret dashboard").unwrap(),
        secret,
        "searchable group must match"
    );

    // Disable searching on the group (KeePass EnableSearching=false); its
    // entries are then invisible to auto-type matching.
    {
        let db = session.require_db_mut().unwrap();
        let mut group = db
            .group_mut(super::helpers::parse_group_id(&group_a).unwrap())
            .expect("group must exist");
        group.enable_searching = Some(false);
    }
    let err = session.autotype_match("Secret dashboard").unwrap_err();
    assert!(
        err.contains("没有找到匹配"),
        "disabled group must not match"
    );

    // The flag is per-group: a descendant group with searching enabled still
    // matches on its own entries.
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(group_a.clone()),
            name: "A1".into(),
            icon: None,
        })
        .unwrap();
    let group_a1 = state
        .root
        .children
        .iter()
        .find(|g| g.uuid == group_a)
        .unwrap()
        .children[0]
        .uuid
        .clone();
    let state = session
        .add_entry(&EntryInput {
            group_uuid: group_a1.clone(),
            title: "Child secret".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://child.example".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let child_uuid = state
        .root
        .children
        .iter()
        .find(|g| g.uuid == group_a)
        .unwrap()
        .children[0]
        .entries[0]
        .uuid
        .clone();
    assert_eq!(
        session.autotype_match("Child secret window").unwrap(),
        child_uuid,
        "descendant with searching enabled still matches"
    );
    let snapshot = session.snapshot().unwrap();
    let group = snapshot
        .root
        .children
        .iter()
        .find(|g| g.uuid == group_a)
        .expect("group in snapshot");
    assert!(
        !group.enable_searching,
        "group A must expose the disabled flag"
    );
    let a1 = group.children[0].clone();
    assert!(a1.enable_searching);
}

#[test]
fn url_host_strips_scheme_port_and_path() {
    assert_eq!(
        url_host("https://github.com/login"),
        Some("github.com".into())
    );
    assert_eq!(url_host("http://a.b.c:8080/x?y=1"), Some("a.b.c".into()));
    assert_eq!(url_host("plain-host"), Some("plain-host".into()));
    assert_eq!(url_host(""), None);
}

#[test]
fn ref_match_skips_groups_with_searching_disabled() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    // Entry inside a search-disabled group must be invisible to {REF:...}.
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Hidden".into(),
            icon: None,
        })
        .unwrap();
    let hidden = state.root.children[0].uuid.clone();
    session
        .add_entry(&EntryInput {
            group_uuid: hidden.clone(),
            title: "HiddenBank".into(),
            username: "h-u".into(),
            password: "h-pass".into(),
            url: "https://hidden.example".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    {
        let db = session.require_db_mut().unwrap();
        let mut group = db
            .group_mut(super::helpers::parse_group_id(&hidden).unwrap())
            .expect("group must exist");
        group.enable_searching = Some(false);
    }
    let err = session
        .expand_autotype_sequence("{REF:P@T:HiddenBank}")
        .unwrap_err();
    assert!(
        err.contains("未找到匹配"),
        "entry in a search-disabled group must not resolve by title: {err}"
    );

    // A sibling entry (searchable) still resolves.
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "Visible".into(),
            icon: None,
        })
        .unwrap();
    let visible = state
        .root
        .children
        .iter()
        .find(|g| g.name == "Visible")
        .expect("visible group in snapshot")
        .uuid
        .clone();
    session
        .add_entry(&EntryInput {
            group_uuid: visible.clone(),
            title: "VisibleBank".into(),
            username: "v-u".into(),
            password: "v-pass".into(),
            url: "https://visible.example".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    assert_eq!(
        session
            .expand_autotype_sequence("{REF:P@T:VisibleBank}")
            .unwrap(),
        "v-pass"
    );
}

#[test]
fn autotype_sequence_resolves_entry_then_group_then_default() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let entry = |group_uuid: &str, title: &str| EntryInput {
        group_uuid: group_uuid.to_owned(),
        title: title.into(),
        username: "u".into(),
        password: "p".into(),
        url: "".into(),
        notes: "".into(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    };

    // Group G with a default sequence; entry E inside it without its own.
    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            name: "G".into(),
            icon: None,
        })
        .unwrap();
    let g = state.root.children[0].uuid.clone();
    {
        let db = session.require_db_mut().unwrap();
        let mut group = db
            .group_mut(super::helpers::parse_group_id(&g).unwrap())
            .expect("group must exist");
        group.default_autotype_sequence = Some("{TITLE}{ENTER}".into());
    }
    let state = session.add_entry(&entry(&g, "E")).unwrap();
    let e = state.root.children[0].entries[0].uuid.clone();
    assert_eq!(
        session.resolve_autotype_sequence(&e).unwrap().as_deref(),
        Some("{TITLE}{ENTER}"),
        "group sequence is used when the entry defines none"
    );

    // An entry-level AutoType sequence overrides the group's.
    {
        let db = session.require_db_mut().unwrap();
        let mut entry = db
            .entry_mut(parse_entry_id(&e).unwrap())
            .expect("entry must exist");
        entry.autotype = Some(keepass::db::AutoType {
            enabled: true,
            default_sequence: Some("{USERNAME}{ENTER}".into()),
            ..Default::default()
        });
    }
    assert_eq!(
        session.resolve_autotype_sequence(&e).unwrap().as_deref(),
        Some("{USERNAME}{ENTER}"),
        "entry sequence overrides the group sequence"
    );

    // Entry with AutoType disabled yields None (no auto-type at all).
    {
        let db = session.require_db_mut().unwrap();
        let mut entry = db
            .entry_mut(parse_entry_id(&e).unwrap())
            .expect("entry must exist");
        if let Some(autotype) = entry.autotype.as_mut() {
            autotype.enabled = false;
        }
    }
    assert_eq!(
        session.resolve_autotype_sequence(&e).unwrap(),
        None,
        "disabled entry AutoType yields no sequence"
    );

    // Group with auto-type disabled yields None for a sequence-less entry.
    {
        let db = session.require_db_mut().unwrap();
        let mut group = db
            .group_mut(super::helpers::parse_group_id(&g).unwrap())
            .expect("group must exist");
        group.enable_autotype = Some(false);
        group.default_autotype_sequence = Some("{TITLE}{ENTER}".into());
    }
    assert_eq!(
        session.resolve_autotype_sequence(&e).unwrap(),
        None,
        "disabled group auto-type yields no sequence"
    );

    // Root entry with no sequences anywhere falls back to the global default.
    let state = session
        .add_entry(&entry(ROOT_GROUP_UUID, "RootEntry"))
        .unwrap();
    let root_entry = state.root.entries[0].uuid.clone();
    assert_eq!(
        session.resolve_autotype_sequence(&root_entry).unwrap(),
        None,
        "no entry/group sequence resolves to None; caller uses the global default"
    );
}

#[test]
fn expand_autotype_sequence_resolves_refs_across_entries() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let entry = |title: &str, username: &str, password: &str, url: &str| EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: title.into(),
        username: username.into(),
        password: password.into(),
        url: url.into(),
        notes: "".into(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    };
    let state = session
        .add_entry(&entry("Bank", "alice", "secret123", "https://bank.example"))
        .unwrap();
    let bank_uuid = state.root.entries[0].uuid.clone();
    session
        .add_entry(&entry(
            "Mail",
            "mail-bot",
            "mail-pass",
            "https://mail.example",
        ))
        .unwrap();

    // By UUID (case-insensitive, dashes tolerated).
    let expanded = session
        .expand_autotype_sequence(&format!(
            "{{REF:U@I:{bank_uuid}}}{{TAB}}{{REF:P@I:{bank_uuid}}}"
        ))
        .unwrap();
    assert_eq!(expanded, format!("alice{{TAB}}secret123"));
    // By title / URL substring.
    assert_eq!(
        session.expand_autotype_sequence("{REF:P@T:bank}").unwrap(),
        "secret123"
    );
    assert_eq!(
        session
            .expand_autotype_sequence("{REF:U@A:mail.example}")
            .unwrap(),
        "mail-bot"
    );
    // UUID as wanted field.
    assert_eq!(
        session.expand_autotype_sequence("{REF:I@T:Bank}").unwrap(),
        bank_uuid
    );
    // Custom-string name as search (O), standard field as target.
    session
        .update_entry(
            &bank_uuid,
            &EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Bank".into(),
                username: "alice".into(),
                password: "secret123".into(),
                url: "https://bank.example".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: vec![CustomField {
                    name: "Customer Id".into(),
                    value: "CUST-42".into(),
                    protected: false,
                }],
                attachments: vec![],
            },
        )
        .unwrap();
    assert_eq!(
        session
            .expand_autotype_sequence("{REF:U@O:Customer Id}")
            .unwrap(),
        "alice"
    );
    // Unresolvable reference fails with a Chinese message.
    let err = session
        .expand_autotype_sequence("{REF:P@T:missing}")
        .unwrap_err();
    assert!(err.contains("未找到匹配条目"));
}

#[test]
fn expand_autotype_sequence_skips_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Old".into(),
            username: "u".into(),
            password: "p".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![],
            attachments: vec![],
        })
        .unwrap();
    let old_uuid = state.root.entries[0].uuid.clone();
    session.delete_entry(&old_uuid).unwrap();
    let err = session
        .expand_autotype_sequence(&format!("{{REF:P@I:{old_uuid}}}"))
        .unwrap_err();
    assert!(err.contains("未找到匹配条目"));
}

#[test]
fn parse_expiry_accepts_frontend_iso_with_milliseconds() {
    assert_eq!(
        parse_expiry(Some("2026-08-01T12:34:56.000Z")),
        Some(
            chrono::NaiveDate::from_ymd_opt(2026, 8, 1)
                .unwrap()
                .and_hms_opt(12, 34, 56)
                .unwrap()
        )
    );
    assert_eq!(
        parse_expiry(Some("2099-12-31T23:59:59.500Z")).map(|d| d.and_utc().timestamp_millis()),
        Some(
            chrono::NaiveDate::from_ymd_opt(2099, 12, 31)
                .unwrap()
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc()
                .timestamp_millis()
                + 500
        )
    );
    assert_eq!(
        parse_expiry(Some("2020-01-01T00:00:00Z")),
        Some(
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        )
    );
}

#[test]
fn parse_expiry_accepts_legacy_naive_and_rejects_garbage() {
    assert_eq!(
        parse_expiry(Some("2020-01-01T00:00:00")),
        Some(
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        )
    );
    assert_eq!(parse_expiry(Some("")), None);
    assert_eq!(parse_expiry(None), None);
    assert_eq!(parse_expiry(Some("not-a-date")), None);
}

// -- browser bridge (KeePassHttp) --------------------------------------

fn entry_input(
    group_uuid: &str,
    title: &str,
    username: &str,
    password: &str,
    url: &str,
) -> EntryInput {
    EntryInput {
        group_uuid: group_uuid.to_owned(),
        title: title.to_owned(),
        username: username.to_owned(),
        password: password.to_owned(),
        url: url.to_owned(),
        notes: String::new(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: Vec::new(),
        attachments: Vec::new(),
    }
}

#[test]
fn bridge_client_keys_are_session_held_and_wiped_on_close() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    assert_eq!(session.list_clients(), Vec::<String>::new());
    assert!(session.client_key("browser-a").is_none());

    session.register_client("browser-a", vec![1u8; 32]);
    session.register_client("browser-b", vec![2u8; 32]);
    assert_eq!(session.client_key("browser-a").unwrap(), vec![1u8; 32]);
    assert!(session.list_clients().contains(&"browser-a".to_owned()));
    assert!(session.list_clients().contains(&"browser-b".to_owned()));

    assert!(session.remove_client("browser-a"));
    assert!(!session.remove_client("browser-a"));
    assert_eq!(session.list_clients(), vec!["browser-b".to_owned()]);

    session.close();
    assert!(!session.is_open());
    assert_eq!(session.list_clients(), Vec::<String>::new());
    assert!(session.client_key("browser-b").is_none());
}

#[test]
fn bridge_logins_match_host_and_subdomains_but_not_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let root = ROOT_GROUP_UUID.to_owned();

    let state = session
        .add_entry(&entry_input(
            &root,
            "主站",
            "user@example",
            "pw-1",
            "https://example.com",
        ))
        .unwrap();
    let _ = state;
    session
        .add_entry(&entry_input(
            &root,
            "子域",
            "user@www",
            "pw-2",
            "https://www.example.com",
        ))
        .unwrap();
    let state = session
        .add_entry(&entry_input(
            &root,
            "无关",
            "user@else",
            "pw-3",
            "https://elsewhere.io",
        ))
        .unwrap();
    let www_uuid = state.root.entries[1].uuid.clone();
    let other_uuid = state.root.entries[2].uuid.clone();
    let _ = other_uuid;

    // Exact host, subdomain-of, and superdomain-of all match.
    let logins = session.logins_for("https://example.com/login", None);
    assert_eq!(logins.len(), 2);
    assert!(logins.iter().any(|l| l.login == "user@www"));

    // A request subdomain matches the bare entry host only.
    let logins = session.logins_for("https://sub.example.com", None);
    assert_eq!(logins.len(), 1);
    assert_eq!(logins[0].login, "user@example");

    // Submit URL can match too (elsewhere + example.com + www.example.com).
    let logins = session.logins_for("https://elsewhere.io", Some("https://example.com"));
    assert_eq!(logins.len(), 3);

    // No URL at all matches nothing.
    let logins = session.logins_for("https://example.com", None);
    assert!(logins.iter().all(|l| !l.uuid.is_empty()));
    assert!(session.logins_for("", None).is_empty());
    assert!(session.logins_for("https://nomatch.xyz", None).is_empty());

    // Entries moved to the recycle bin are invisible to the bridge.
    session.delete_entry(&www_uuid).unwrap();
    let logins = session.logins_for("https://www.example.com", None);
    assert!(logins.iter().all(|l| l.uuid != www_uuid));
    let logins = session.logins_for("https://example.com", None);
    assert!(logins.iter().all(|l| l.uuid != www_uuid));
    assert!(logins.iter().any(|l| l.login == "user@example"));
}

#[test]
fn bridge_set_login_updates_entry_fields() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let root = ROOT_GROUP_UUID.to_owned();

    let state = session
        .add_entry(&entry_input(
            &root,
            "站点",
            "old-user",
            "old-pw",
            "https://example.com",
        ))
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    session
        .set_login("new-user", "new-pw", "https://example.com/sso", Some(&uuid))
        .unwrap();
    assert_eq!(session.get_entry_password(&uuid).unwrap(), "new-pw");
    assert_eq!(
        session.autotype_context(&uuid).unwrap().username,
        "new-user"
    );
    assert_eq!(
        session.autotype_context(&uuid).unwrap().url,
        "https://example.com"
    );
    let id = parse_entry_id(&uuid).unwrap();
    let entry = session.require_db().unwrap().entry(id).unwrap();
    let history = entry.history.as_ref().unwrap();
    assert!(history.get_entries().iter().any(|entry| {
        entry.get_username() == Some("old-user") && entry.get_password() == Some("old-pw")
    }));

    let revision = session.state().unwrap().unwrap().revision;
    session
        .set_login(
            "new-user",
            "new-pw",
            "https://unrelated.example/ignored",
            Some(&uuid),
        )
        .unwrap();
    assert_eq!(session.state().unwrap().unwrap().revision, revision);

    let err = session.set_login(
        "u",
        "p",
        "https://example.com",
        Some("00000000-0000-0000-0000-000000000000"),
    );
    assert!(err.is_err());
}

#[test]
fn bridge_create_login_adds_entry_with_host_title() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    session
        .create_login("fresh-user", "fresh-pw", "https://fresh.example.net/x")
        .unwrap();

    let state = session.state().unwrap().unwrap();
    assert_eq!(state.root.entries.len(), 1);
    let entry = &state.root.entries[0];
    assert_eq!(entry.title, "fresh.example.net");
    assert_eq!(entry.username, "fresh-user");

    session.create_login("u2", "p2", "not-a-url").unwrap();
    let state = session.state().unwrap().unwrap();
    assert_eq!(state.root.entries[1].title, "not-a-url");
}

#[test]
fn bridge_db_hash_is_sha1_hex_of_root_and_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    let hash = session.db_hash();
    assert_eq!(hash.len(), 40);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

    // A recycled entry changes the hash (recycle-bin uuid is part of it).
    let state = session
        .add_entry(&entry_input(ROOT_GROUP_UUID, "x", "u", "p", "https://a.b"))
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();
    session.delete_entry(&uuid).unwrap();
    let after = session.db_hash();
    assert_eq!(after.len(), 40);
    assert_ne!(after, hash);

    session.close();
    assert_eq!(session.db_hash(), "");
}

// -- KeePassRPC host ----------------------------------------------------

#[test]
fn rpc_keys_are_session_held_and_wiped_on_close() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    assert!(session.rpc_key("user@browser").is_none());
    session.register_rpc_key("user@browser", vec![7u8; 32]);
    assert_eq!(session.rpc_key("user@browser").unwrap(), vec![7u8; 32]);

    session.close();
    assert!(session.rpc_key("user@browser").is_none());
}

#[test]
fn close_keeping_rpc_session_retains_rpc_keys_but_wipes_bridge_and_secrets() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    session.register_rpc_key("user@browser", vec![7u8; 32]);
    session
        .bridge_keys
        .insert("bridge".to_owned(), vec![9u8; 24]);

    session.close_keeping_rpc_session();

    assert!(session.rpc_key("user@browser").is_some());
    assert!(session.bridge_keys.is_empty());
    assert!(session.password.is_none());
    assert!(session.keyfile.is_none());
    assert!(!session.is_open());
}

#[test]
fn rpc_database_dto_builds_group_tree_and_skips_recycle_bin() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    let state = session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
            icon: None,
            name: "Internet".into(),
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();
    session
        .add_entry(&entry_input(
            &group_uuid,
            "Example",
            "alice",
            "s3cret",
            "https://example.com/login",
        ))
        .unwrap();
    let root_entry = session
        .add_entry(&entry_input(
            ROOT_GROUP_UUID,
            "Trash",
            "ghost",
            "pw-x",
            "https://ghost.example",
        ))
        .unwrap();
    let trash_uuid = root_entry.root.entries[0].uuid.clone();

    let db = session.database().unwrap();
    assert_eq!(db.file_name, "test.kdbx");
    assert!(db.active);
    assert_eq!(db.root.title, "Root");
    assert_eq!(db.root.children.len(), 1);
    assert_eq!(db.root.children[0].title, "Internet");
    assert_eq!(db.root.children[0].path, "Root/Internet");
    assert_eq!(db.root.children[0].children.len(), 0);

    // Moved to recycle bin: gone from the tree and from FindLogins.
    session.delete_entry(&trash_uuid).unwrap();
    let db = session.database().unwrap();
    assert!(db.root.entries.is_empty());
    let logins = session.find_logins(&["https://ghost.example".to_owned()], None, None, None);
    assert!(logins.is_empty());
}

#[test]
fn rpc_find_logins_matches_url_uuid_and_free_text() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let root = ROOT_GROUP_UUID.to_owned();

    session
        .add_entry(&entry_input(
            &root,
            "Example",
            "alice",
            "s3cret",
            "https://example.com/login",
        ))
        .unwrap();
    session
        .add_entry(&entry_input(
            &root,
            "Other",
            "bob",
            "pw-2",
            "https://other.example",
        ))
        .unwrap();

    let by_url = session.find_logins(
        &["https://example.com/dashboard".to_owned()],
        None,
        None,
        None,
    );
    assert_eq!(by_url.len(), 1);
    assert_eq!(by_url[0].username, "alice");
    assert_eq!(by_url[0].password, "s3cret");
    assert_eq!(by_url[0].urls, vec!["https://example.com/login".to_owned()]);
    assert_eq!(by_url[0].parent_group.title, "Root");

    let uuid = by_url[0].uuid.clone();
    let by_uuid = session.find_logins(&[], Some(&uuid), None, None);
    assert_eq!(by_uuid.len(), 1);

    let by_text = session.find_logins(&[], None, Some("Examp"), None);
    assert_eq!(by_text.len(), 1);
    assert_eq!(by_text[0].title, "Example");

    let by_username = session.find_logins(&[], None, None, Some("bob"));
    assert_eq!(by_username.len(), 1);
    assert_eq!(by_username[0].username, "bob");

    session.close();
    assert!(session
        .find_logins(&["https://example.com".to_owned()], None, None, None)
        .is_empty());
    assert!(session.database().is_none());
}

// -- KeePassRPC altURLs (additional custom match URLs) ----------------

const KPRPC_JSON: &str = "KPRPC JSON";

fn entry_with_alt_urls(
    session: &mut VaultSession,
    title: &str,
    primary_url: &str,
    alt_urls: Vec<&str>,
) -> String {
    let json = format!(
        "{{\"version\":1,\"altURLs\":{}}}",
        serde_json::to_string(&alt_urls).unwrap()
    );
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: title.into(),
            username: "u".into(),
            password: "p".into(),
            url: primary_url.into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![CustomField {
                name: KPRPC_JSON.to_owned(),
                value: json,
                protected: false,
            }],
            attachments: vec![],
        })
        .unwrap();
    // Prove the JSON survived the write as a real KDBX custom field.
    let uuid = state.root.entries[0].uuid.clone();
    let id = parse_entry_id(&uuid).unwrap();
    let entry = session.db.as_ref().unwrap().entry(id).unwrap();
    assert!(entry.get("URL").is_some(), "primary URL must exist");
    assert!(
        entry
            .get(KPRPC_JSON)
            .unwrap_or_default()
            .contains("altURLs"),
        "KPRPC JSON must be persisted"
    );
    uuid
}

#[test]
fn entry_match_urls_includes_primary_and_alt_urls() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let uuid = entry_with_alt_urls(
        &mut session,
        "主站",
        "https://example.com",
        vec!["https://alt1.example", "https://alt2.example"],
    );
    let id = parse_entry_id(&uuid).unwrap();
    let entry = session.db.as_ref().unwrap().entry(id).unwrap();
    let urls = super::helpers::entry_match_urls(&entry);
    assert_eq!(
        urls,
        vec![
            "https://example.com".to_owned(),
            "https://alt1.example".to_owned(),
            "https://alt2.example".to_owned()
        ]
    );
}

#[test]
fn entry_without_kprpc_json_matches_primary_url_only() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_entry(&entry_input(
            ROOT_GROUP_UUID,
            "普通",
            "u",
            "p",
            "https://example.com",
        ))
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();
    let id = parse_entry_id(&uuid).unwrap();
    let entry = session.db.as_ref().unwrap().entry(id).unwrap();
    assert_eq!(
        super::helpers::entry_match_urls(&entry),
        vec!["https://example.com".to_owned()]
    );
}

#[test]
fn malformed_kprpc_json_degrades_to_primary_url() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "坏".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![CustomField {
                name: KPRPC_JSON.to_owned(),
                value: "not-json{{".into(),
                protected: false,
            }],
            attachments: vec![],
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();
    let id = parse_entry_id(&uuid).unwrap();
    let entry = session.db.as_ref().unwrap().entry(id).unwrap();
    assert_eq!(
        super::helpers::entry_match_urls(&entry),
        vec!["https://example.com".to_owned()]
    );
}

#[test]
fn bridge_matches_alt_urls_written_by_kee() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    entry_with_alt_urls(
        &mut session,
        "主站",
        "https://example.com",
        vec!["https://alt1.example"],
    );

    // Primary URL still matches.
    let logins = session.logins_for("https://example.com/login", None);
    assert_eq!(logins.len(), 1);
    // The Kee-written alternative URL also matches.
    let logins = session.logins_for("https://alt1.example/app", None);
    assert_eq!(logins.len(), 1);
    // A subdomain of the alternative URL matches too (host-level matching).
    let logins = session.logins_for("https://sub.alt1.example", None);
    assert_eq!(logins.len(), 1);
    // Unrelated host does not match.
    assert!(session.logins_for("https://elsewhere.io", None).is_empty());
}

#[test]
fn rpc_find_logins_matches_alt_urls() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    entry_with_alt_urls(
        &mut session,
        "主站",
        "https://example.com",
        vec!["https://alt1.example"],
    );

    let logins = session.find_logins(&["https://alt1.example".to_owned()], None, None, None);
    assert_eq!(logins.len(), 1);
    // The returned entry exposes both the primary and alternative URLs to Kee.
    assert_eq!(
        logins[0].urls,
        vec![
            "https://example.com".to_owned(),
            "https://alt1.example".to_owned()
        ]
    );
}

#[test]
fn autotype_match_considers_alt_urls() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let uuid = entry_with_alt_urls(
        &mut session,
        "某站",
        "https://example.com",
        vec!["https://alt1.example"],
    );
    let matched = session.autotype_match("Login · alt1.example").unwrap();
    assert_eq!(matched, uuid);
}

// -- OverrideURL ---------------------------------------------------------

#[test]
fn override_url_replaces_url_for_matching() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let uuid = entry_with_alt_urls(
        &mut session,
        "某站",
        "https://stored.example",
        vec!["https://alt1.example"],
    );
    // OverrideURL is set to the real deployment URL; the URL field holds a
    // placeholder that must NOT be used for matching.
    {
        let db = session.require_db_mut().unwrap();
        let mut entry = db
            .entry_mut(parse_entry_id(&uuid).unwrap())
            .expect("entry must exist");
        entry.override_url = Some("https://real.example".into());
    }

    // The override wins over the URL field in the match-URL list.
    let id = parse_entry_id(&uuid).unwrap();
    let entry = session.db.as_ref().unwrap().entry(id).unwrap();
    assert_eq!(
        super::helpers::entry_match_urls(&entry),
        vec![
            "https://real.example".to_owned(),
            "https://alt1.example".to_owned()
        ]
    );
    // logins_for (bridge) matches on the override host.
    assert_eq!(
        session.logins_for("https://real.example/login", None).len(),
        1
    );
    // The stored URL field alone no longer matches.
    assert!(session
        .logins_for("https://stored.example/login", None)
        .is_empty());
    // find_logins matches on the override.
    assert_eq!(
        session
            .find_logins(&["https://real.example".to_owned()], None, None, None)
            .len(),
        1
    );
    // Auto-type window detection uses the override too.
    let matched = session.autotype_match("Login · real.example").unwrap();
    assert_eq!(matched, uuid);
}

#[test]
fn empty_override_url_falls_back_to_url_field() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let uuid = entry_with_alt_urls(&mut session, "某站", "https://stored.example", vec![]);
    {
        let db = session.require_db_mut().unwrap();
        let mut entry = db
            .entry_mut(parse_entry_id(&uuid).unwrap())
            .expect("entry must exist");
        entry.override_url = Some("".into());
    }
    let id = parse_entry_id(&uuid).unwrap();
    let entry = session.db.as_ref().unwrap().entry(id).unwrap();
    assert_eq!(
        super::helpers::entry_match_urls(&entry),
        vec!["https://stored.example".to_owned()]
    );
    assert_eq!(
        session
            .logins_for("https://stored.example/login", None)
            .len(),
        1
    );
}

#[test]
fn update_entry_flags_round_trips_override_url_and_quality_check() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://stored.example".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        })
        .unwrap();
    let uuid = state.root.entries[0].uuid.clone();

    // Set the flags and a foreground color.
    let updated = session
        .update_entry_flags(
            &uuid,
            Some("https://real.example".into()),
            Some(false),
            Some("#FF0000".into()),
        )
        .unwrap();
    let entry = updated
        .root
        .entries
        .iter()
        .find(|e| e.uuid == uuid)
        .unwrap();
    assert_eq!(entry.override_url.as_deref(), Some("https://real.example"));
    assert!(!entry.quality_check);
    assert_eq!(entry.foreground_color.as_deref(), Some("#FF0000"));

    // Absent flags keep the current values.
    let updated = session.update_entry_flags(&uuid, None, None, None).unwrap();
    let entry = updated
        .root
        .entries
        .iter()
        .find(|e| e.uuid == uuid)
        .unwrap();
    assert_eq!(entry.override_url.as_deref(), Some("https://real.example"));
    assert_eq!(entry.foreground_color.as_deref(), Some("#FF0000"));

    // Empty override/color clear them; quality check is restored.
    let updated = session
        .update_entry_flags(&uuid, Some(String::new()), Some(true), Some(String::new()))
        .unwrap();
    let entry = updated
        .root
        .entries
        .iter()
        .find(|e| e.uuid == uuid)
        .unwrap();
    assert!(entry.override_url.is_none());
    assert!(entry.quality_check);
    assert!(entry.foreground_color.is_none());

    // Save + reopen keeps the flags.
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    let state = reopened.open(&path, "master-password", None).unwrap();
    let entry = state.root.entries.iter().find(|e| e.uuid == uuid).unwrap();
    assert!(entry.override_url.is_none());
    assert!(entry.quality_check);
    assert!(entry.foreground_color.is_none());
}

// -- KeePassRPC full KPRPC config (regex / blocked / accuracy) ----------

fn entry_with_kprpc_config(
    session: &mut VaultSession,
    title: &str,
    primary_url: &str,
    config: serde_json::Value,
) -> String {
    let state = session
        .add_entry(&EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: title.into(),
            username: "u".into(),
            password: "p".into(),
            url: primary_url.into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            tags: None,
            custom_fields: vec![CustomField {
                name: KPRPC_JSON.to_owned(),
                value: serde_json::to_string(&config).unwrap(),
                protected: false,
            }],
            attachments: vec![],
        })
        .unwrap();
    state.root.entries[0].uuid.clone()
}

#[test]
fn bridge_matches_regex_match_urls() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    entry_with_kprpc_config(
        &mut session,
        "正则站",
        "",
        serde_json::json!({
            "version": 1,
            "regExURLs": ["https://secure[0-9]+\\.example\\.com"]
        }),
    );
    // Regex triggers a match even without a primary URL or host tier hit.
    let logins = session.logins_for("https://secure42.example.com/page", None);
    assert_eq!(logins.len(), 1);
    // A URL the regex does not cover still misses.
    assert!(session
        .logins_for("https://login.example.com", None)
        .is_empty());
}

#[test]
fn bridge_respects_blocked_urls_and_regex_blocked_urls() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    entry_with_kprpc_config(
        &mut session,
        "站A",
        "https://example.com",
        serde_json::json!({
            "version": 1,
            "blockedURLs": ["https://blog.example.com"],
            "regExBlockedURLs": ["https://secure.*\\.example\\.com"]
        }),
    );
    // Primary URL itself still matches.
    let logins = session.logins_for("https://example.com/login", None);
    assert_eq!(logins.len(), 1);
    // Standard blocked URL vetoes the match.
    assert!(session
        .logins_for("https://blog.example.com", None)
        .is_empty());
    // Regex blocked URL vetoes the match too.
    assert!(session
        .logins_for("https://secure5.example.com", None)
        .is_empty());
    // Unrelated subdomain (not blocked) still matches via domain tier.
    let logins = session.logins_for("https://app.example.com", None);
    assert_eq!(logins.len(), 1);
}

#[test]
fn rpc_and_autotype_honor_blocked_urls() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let uuid = entry_with_kprpc_config(
        &mut session,
        "站B",
        "https://example.com",
        serde_json::json!({
            "version": 1,
            "blockedURLs": ["https://secret.example.com"]
        }),
    );
    // Primary URL matches at domain tier for both surfaces.
    assert_eq!(
        session
            .find_logins(&["https://example.com".to_owned()], None, None, None)
            .len(),
        1
    );
    // The blocked URL is invisible to RPC find_logins.
    assert!(session
        .find_logins(&["https://secret.example.com".to_owned()], None, None, None)
        .is_empty());
    // Auto-type still finds the entry via its primary host.
    assert_eq!(
        session.autotype_match("Dashboard · example.com").unwrap(),
        uuid
    );
}

#[test]
fn domain_accuracy_alt_url_matches_deep_login_path_with_query() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    entry_with_kprpc_config(
        &mut session,
        "阿里云",
        "",
        serde_json::json!({
            "version": 1,
            "altURLs": ["https://account.aliyun.com"]
        }),
    );
    // The real Aliyun login page carries a long oauth_callback query on the
    // same host; Domain accuracy must still hit (host equal).
    let logins = session.logins_for(
        "https://account.aliyun.com/login/login.htm?oauth_callback=https%3A%2F%2Faccount-devops.aliyun.com%2Flogin%3Fnext_url%3Dhttp%253A%252F%252Faccount-devops.aliyun.com%252Flogin%253Fnext_url%253Dhttp%25253A%25252F%25252Fcodeup.aliyun.com%25252F%253FnavKey%25253Dmine",
        None,
    );
    assert_eq!(logins.len(), 1);
    // A sibling host (not a subdomain) does not match, mirroring KeePassRPC's
    // host-tier boundary: `login.aliyun.com` is unrelated to `account.aliyun.com`.
    assert!(session
        .logins_for("https://login.aliyun.com/login", None)
        .is_empty());
    // A true subdomain under the configured host still matches at Domain.
    let logins = session.logins_for("https://intl.account.aliyun.com/login", None);
    assert_eq!(logins.len(), 1);
}

#[test]
fn registrable_domain_match_connects_sibling_hosts_when_enabled() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    entry_with_kprpc_config(
        &mut session,
        "阿里云",
        "", // primary URL empty; match comes from altURLs
        serde_json::json!({
            "version": 1,
            "altURLs": ["https://account.aliyun.com"]
        }),
    );
    // Strict host mode (config off): sibling hosts never connect.
    assert!(session
        .logins_for(
            "https://passport.aliyun.com/havanaone/login/login.htm",
            None
        )
        .is_empty());
    // Registrable-domain mode (KeePassRPC): both share `aliyun.com`.
    session.match_registrable_domain = true;
    let logins = session.logins_for(
        "https://passport.aliyun.com/havanaone/login/login.htm?lang=zh_CN",
        None,
    );
    assert_eq!(logins.len(), 1);
    // The configured host itself still matches, as in strict mode.
    let logins = session.logins_for("https://account.aliyun.com/login/login.htm", None);
    assert_eq!(logins.len(), 1);
    // RPC find_logins. shares the same mode.
    let found = session.find_logins(
        &["https://passport.aliyun.com/login".to_owned()],
        None,
        None,
        None,
    );
    assert_eq!(found.len(), 1);
}

#[test]
fn match_accuracy_exact_blocks_subdomains() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    entry_with_kprpc_config(
        &mut session,
        "精确站",
        "https://example.com/login",
        serde_json::json!({
            "version": 1,
            "blockHostnameOnlyMatch": true
        }),
    );
    // Exact URL matches.
    let logins = session.logins_for("https://example.com/login", None);
    assert_eq!(logins.len(), 1);
    // Same host but a different path no longer matches under Exact.
    assert!(session
        .logins_for("https://example.com/dashboard", None)
        .is_empty());
    // A subdomain misses as well.
    assert!(session
        .logins_for("https://sub.example.com/login", None)
        .is_empty());
}

#[test]
fn malformed_kprpc_config_degrades_to_domain_accuracy() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    entry_with_kprpc_config(
        &mut session,
        "回退站",
        "https://example.com",
        serde_json::json!({}),
    );
    // No flags → Domain accuracy; the primary URL is the only match source.
    let logins = session.logins_for("https://sub.example.com", None);
    assert_eq!(logins.len(), 1);
}

// -- KeePassRPC write path (AddLogin/UpdateLogin) ----------------------

fn rpc_login_write(title: &str, username: &str, password: &str, urls: &[&str]) -> RpcLoginWrite {
    use crate::rpc::RpcFieldWrite;
    RpcLoginWrite {
        title: title.to_owned(),
        urls: urls.iter().map(|u| u.to_string()).collect(),
        http_realm: String::new(),
        icon_image_data: String::new(),
        form_field_list: vec![
            RpcFieldWrite {
                id: "u".to_owned(),
                name: "user".to_owned(),
                display_name: "KeePass username".to_owned(),
                field_type: "FFTusername".to_owned(),
                value: username.to_owned(),
                page: 0,
            },
            RpcFieldWrite {
                id: "p".to_owned(),
                name: "pass".to_owned(),
                display_name: "KeePass password".to_owned(),
                field_type: "FFTpassword".to_owned(),
                value: password.to_owned(),
                page: 0,
            },
            RpcFieldWrite {
                id: "n".to_owned(),
                name: "note".to_owned(),
                display_name: "Custom note".to_owned(),
                field_type: "FFTtext".to_owned(),
                value: "hello".to_owned(),
                page: 0,
            },
        ],
    }
}

#[test]
fn rpc_add_login_creates_entry_with_fields_and_urls() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    let login = rpc_login_write("站点", "alice", "pw-1", &["https://rpc.example.com/login"]);
    let created = session.add_login(&login, "").unwrap();
    assert!(!created.uuid.is_empty());
    assert_eq!(created.title, "站点");
    assert_eq!(created.username, "alice");
    assert_eq!(created.password, "pw-1");
    assert_eq!(
        created.urls,
        vec!["https://rpc.example.com/login".to_owned()]
    );
    assert_eq!(created.parent_group.title, "Root");
    assert_eq!(created.parent_group.path, "Root");

    // Username/password land in the standard fields, the extra form field
    // becomes a custom string, and FindLogins sees the new entry.
    let state = session.state().unwrap().unwrap();
    let entry = &state.root.entries[0];
    assert_eq!(entry.username, "alice");
    assert_eq!(entry.url, "https://rpc.example.com/login");
    assert!(entry
        .custom_fields
        .iter()
        .any(|f| f.name == "Custom note" && f.value == "hello"));

    let by_url = session.find_logins(
        &["https://rpc.example.com/dashboard".to_owned()],
        None,
        None,
        None,
    );
    assert_eq!(by_url.len(), 1);
    assert_eq!(by_url[0].uuid, created.uuid);
}

#[test]
fn rpc_add_login_lands_in_specified_group_and_skips_recycle_bin_parent() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let state = session
        .add_group(&GroupInput {
            parent_uuid: None,
            name: "Internet".to_owned(),
            icon: None,
        })
        .unwrap();
    let group_uuid = state.root.children[0].uuid.clone();

    let login = rpc_login_write("站点", "bob", "pw", &["https://grp.example.com"]);
    let created = session.add_login(&login, &group_uuid).unwrap();
    assert_eq!(created.parent_group.uuid, group_uuid);

    let state = session.state().unwrap().unwrap();
    let group = &state.root.children[0];
    assert_eq!(group.entries.len(), 1);
    assert_eq!(group.entries[0].title, "站点");

    // Unknown or invalid parent uuid falls back to the root group.
    let created = session
        .add_login(&login, "00000000-0000-0000-0000-000000000000")
        .unwrap();
    assert_eq!(created.parent_group.title, "Root");
    assert_eq!(created.parent_group.path, "Root");
}

#[test]
fn rpc_update_login_merges_urls_and_snapshots_history() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);

    let login = rpc_login_write("站点", "alice", "pw-1", &["https://old.example.com"]);
    let created = session.add_login(&login, "").unwrap();

    // Mode 1: old URL kept, new one promoted to primary.
    let update = rpc_login_write("站点", "alice", "pw-2", &["https://new.example.com"]);
    let updated = session.update_login(&update, &created.uuid, 1).unwrap();
    assert_eq!(updated.username, "alice");
    assert_eq!(updated.password, "pw-2");
    assert_eq!(
        updated.urls,
        vec![
            "https://new.example.com".to_owned(),
            "https://old.example.com".to_owned(),
        ]
    );

    // The pre-edit state was snapshotted into the entry history (the
    // plugin's `CreateBackup`): old password is recoverable.
    let id = parse_entry_id(&created.uuid).unwrap();
    let entry = session.db.as_ref().unwrap().entry(id).unwrap();
    let historical = entry.historical(0).unwrap();
    assert_eq!(historical.get_password(), Some("pw-1"));
    assert_eq!(historical.get_url(), Some("https://old.example.com"));

    // Mode 5 replaces the whole list.
    let updated = session.update_login(&update, &created.uuid, 5).unwrap();
    assert_eq!(updated.urls, vec!["https://new.example.com".to_owned()]);
}

#[test]
fn rpc_update_login_rejects_unknown_uuid_recycle_bin_and_locked() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let root = ROOT_GROUP_UUID.to_owned();
    let state = session
        .add_entry(&entry_input(
            &root,
            "Bin",
            "u",
            "p",
            "https://bin.example.com",
        ))
        .unwrap();
    let bin_uuid = state.root.entries[0].uuid.clone();

    let login = rpc_login_write("Bin", "u", "p2", &["https://other.example.com"]);

    // Unknown entry uuid → EntryNotFound.
    assert_eq!(
        session.update_login(&login, "00000000-0000-0000-0000-000000000000", 5),
        Err(RpcError::EntryNotFound)
    );

    // Entries moved to the recycle bin are rejected.
    session.delete_entry(&bin_uuid).unwrap();
    assert_eq!(
        session.update_login(&login, &bin_uuid, 5),
        Err(RpcError::InRecycleBin)
    );

    // A locked vault rejects both write methods.
    session.close();
    assert_eq!(session.add_login(&login, ""), Err(RpcError::Locked));
    assert_eq!(
        session.update_login(&login, &bin_uuid, 5),
        Err(RpcError::Locked)
    );
}

#[test]
fn rpc_write_prepare_does_not_mutate_live_session_before_persist() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let before = session.require_db().unwrap().root().entries().count();
    let login = rpc_login_write("Atomic", "u", "p", &["https://atomic.example"]);

    let _job = session
        .prepare_rpc_write(crate::rpc::RpcWriteRequest::Add {
            login,
            parent_uuid: String::new(),
        })
        .unwrap();

    assert_eq!(
        session.require_db().unwrap().root().entries().count(),
        before
    );
    assert!(!session.state().unwrap().unwrap().dirty);
}

#[test]
fn rpc_write_persist_failure_leaves_live_session_unchanged() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let before = session.state().unwrap().unwrap();
    let login = rpc_login_write("Atomic", "u", "p", &["https://atomic.example"]);
    let job = session
        .prepare_rpc_write(crate::rpc::RpcWriteRequest::Add {
            login,
            parent_uuid: String::new(),
        })
        .unwrap();

    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir_all(dir.path()).unwrap();
    let error = match persist_rpc_write(job) {
        Ok(_) => panic!("persist unexpectedly succeeded"),
        Err(error) => error,
    };
    assert!(error.contains("写入数据库失败"), "{error}");

    let after = session.state().unwrap().unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.dirty, before.dirty);
    assert_eq!(after.root.entries.len(), before.root.entries.len());
}

#[test]
fn rpc_write_completion_replays_on_concurrent_edit_without_overwriting_it() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let login = rpc_login_write("RPC", "rpc-user", "rpc-pass", &["https://rpc.example"]);
    let job = session
        .prepare_rpc_write(crate::rpc::RpcWriteRequest::Add {
            login,
            parent_uuid: String::new(),
        })
        .unwrap();
    let persisted = persist_rpc_write(job).unwrap();

    session
        .add_entry(&entry_input(
            ROOT_GROUP_UUID,
            "ConcurrentUI",
            "ui-user",
            "ui-pass",
            "https://ui.example",
        ))
        .unwrap();
    let (created, _) = session.complete_rpc_write(persisted).unwrap();
    let state = session.state().unwrap().unwrap();
    assert!(state.dirty);
    assert!(state.root.entries.iter().any(|entry| entry.title == "RPC"));
    assert!(state
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "ConcurrentUI"));
    assert_eq!(created.username, "rpc-user");

    // The first persisted snapshot is durable without the concurrent UI edit;
    // the retained dirty state writes both on the next normal save.
    let mut first = VaultSession::default();
    first.open(&path, "master-password", None).unwrap();
    let first_state = first.state().unwrap().unwrap();
    assert!(first_state
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "RPC"));
    assert!(!first_state
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "ConcurrentUI"));
    drop(first);
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    reopened.open(&path, "master-password", None).unwrap();
    let final_state = reopened.state().unwrap().unwrap();
    assert!(final_state
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "RPC"));
    assert!(final_state
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "ConcurrentUI"));
}

#[test]
fn rpc_update_completion_wins_same_entry_fields_and_preserves_ui_history() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    let created = session
        .add_login(
            &rpc_login_write(
                "Initial",
                "initial-user",
                "initial-pass",
                &["https://initial"],
            ),
            "",
        )
        .unwrap();
    let job = session
        .prepare_rpc_write(crate::rpc::RpcWriteRequest::Update {
            login: rpc_login_write("RPC-final", "rpc-user", "rpc-pass", &["https://rpc-final"]),
            old_uuid: created.uuid.clone(),
            url_merge_mode: 5,
        })
        .unwrap();
    let persisted = persist_rpc_write(job).unwrap();

    session
        .update_entry(
            &created.uuid,
            &entry_input(
                ROOT_GROUP_UUID,
                "UI-intermediate",
                "ui-user",
                "ui-pass",
                "https://ui-intermediate",
            ),
        )
        .unwrap();
    let (completed, _) = session.complete_rpc_write(persisted).unwrap();

    assert_eq!(completed.title, "RPC-final");
    assert_eq!(completed.username, "rpc-user");
    assert_eq!(completed.password, "rpc-pass");
    assert_eq!(completed.urls, vec!["https://rpc-final".to_owned()]);
    assert!(session.state().unwrap().unwrap().dirty);

    let id = parse_entry_id(&created.uuid).unwrap();
    let entry = session.require_db().unwrap().entry(id).unwrap();
    let history = entry.history.as_ref().unwrap();
    assert!(history.get_entries().iter().any(|item| {
        item.get_title() == Some("UI-intermediate")
            && item.get_username() == Some("ui-user")
            && item.get_password() == Some("ui-pass")
    }));
}

#[test]
fn rpc_update_completion_reports_persisted_success_after_concurrent_delete() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = create_session(&dir);
    let created = session
        .add_login(
            &rpc_login_write(
                "Initial",
                "initial-user",
                "initial-pass",
                &["https://initial"],
            ),
            "",
        )
        .unwrap();
    let job = session
        .prepare_rpc_write(crate::rpc::RpcWriteRequest::Update {
            login: rpc_login_write("RPC-final", "rpc-user", "rpc-pass", &["https://rpc-final"]),
            old_uuid: created.uuid.clone(),
            url_merge_mode: 5,
        })
        .unwrap();
    let persisted = persist_rpc_write(job).unwrap();

    session.delete_entry(&created.uuid).unwrap();
    let (completed, _) = session.complete_rpc_write(persisted).unwrap();
    assert_eq!(completed.title, "RPC-final");
    assert_eq!(completed.password, "rpc-pass");
    assert!(session.state().unwrap().unwrap().dirty);
    assert!(session
        .state()
        .unwrap()
        .unwrap()
        .root
        .entries
        .iter()
        .all(|entry| entry.uuid != created.uuid));

    let mut durable = VaultSession::default();
    durable.open(&path, "master-password", None).unwrap();
    let persisted_login = durable
        .find_logins(&[], Some(&created.uuid), None, None)
        .pop()
        .unwrap();
    assert_eq!(persisted_login.title, "RPC-final");
    assert_eq!(persisted_login.password, "rpc-pass");
}

// -----------------------------------------------------------------------
// 官方同步·条目级合并: merge_databases (pure) + VaultSession::merge_remote
// -----------------------------------------------------------------------

use super::merge::merge_databases;
use chrono::NaiveDateTime;
use keepass::db::EntryId;

fn merge_ts(minutes: i64) -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-08-01T00:00:00", "%Y-%m-%dT%H:%M:%S").unwrap()
        + chrono::Duration::minutes(minutes)
}

/// Pin every timestamp of an entry to the synthetic timeline so the real
/// "now" that `Times::new()` stamps (creation, last access, location change)
/// cannot leak into merge comparisons.
fn pin_times(db: &mut Database, id: EntryId, modified_at: NaiveDateTime) {
    let mut entry = db.entry_mut(id).unwrap();
    entry.times.creation = Some(merge_ts(0));
    entry.times.last_access = Some(merge_ts(0));
    entry.times.last_modification = Some(modified_at);
    entry.times.location_changed = None;
}

/// Shared base database: one entry "v1" under the root, modified at t10.
/// Both merge sides are clones of this base (same vault lineage).
fn merge_base_db() -> (Database, EntryId) {
    let mut db = Database::new();
    let entry_id = {
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", "v1");
        entry.set_unprotected("UserName", "alice");
        entry.id()
    };
    pin_times(&mut db, entry_id, merge_ts(10));
    (db, entry_id)
}

fn add_titled_entry(db: &mut Database, title: &str, at: NaiveDateTime) -> EntryId {
    let id = {
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected("Title", title);
        entry.id()
    };
    pin_times(db, id, at);
    id
}

/// Edit an entry's title through the history-tracking wrapper, then pin the
/// modification time (the tracking wrapper stamps `now`, which tests cannot
/// order against).
fn edit_title(db: &mut Database, id: EntryId, title: &str, at: NaiveDateTime) {
    {
        let mut entry = db.entry_mut(id).unwrap();
        entry.edit_tracking(|tracked| {
            tracked.as_mut().set_unprotected("Title", title);
        });
    }
    db.entry_mut(id).unwrap().times.last_modification = Some(at);
}

/// Move an entry to the recycle bin exactly like `delete_entries` does
/// (move + LocationChanged stamp).
fn bin_entry(db: &mut Database, id: EntryId, bin_id: keepass::db::GroupId, at: NaiveDateTime) {
    let mut entry = db.entry_mut(id).unwrap();
    entry.move_to(bin_id).unwrap();
    entry.times.location_changed = Some(at);
}

fn add_bin(db: &mut Database) -> keepass::db::GroupId {
    let bin_id = {
        let mut root = db.root_mut();
        let mut bin = root.add_group();
        bin.name = "回收站".to_owned();
        bin.id()
    };
    db.meta.recyclebin_uuid = Some(bin_id.uuid());
    bin_id
}

fn history_titles(entry: &keepass::db::EntryRef<'_>) -> Vec<String> {
    let mut titles = Vec::new();
    let mut index = 0;
    while let Some(historical) = entry.historical(index) {
        titles.push(historical.get_title().unwrap_or("").to_owned());
        index += 1;
    }
    titles
}

/// 同改 (remote newer): the newer remote edit wins and the losing local
/// edit is preserved in the entry history.
#[test]
fn merge_databases_remote_newer_edit_wins_and_preserves_local_history() {
    let (base, entry_id) = merge_base_db();
    let mut local = base.clone();
    let mut remote = base.clone();
    edit_title(&mut local, entry_id, "v1-local", merge_ts(20));
    edit_title(&mut remote, entry_id, "v1-remote", merge_ts(30));

    let merged = merge_databases(&local, &remote).unwrap();
    let entry = merged.entry(entry_id).unwrap();
    assert_eq!(entry.get_title(), Some("v1-remote"));
    let titles = history_titles(&entry);
    assert!(
        titles.contains(&"v1".to_owned()),
        "shared base must stay in history: {titles:?}"
    );
    assert!(
        titles.contains(&"v1-local".to_owned()),
        "losing local edit must land in history: {titles:?}"
    );
}

/// 同改 (local newer): the local edit wins and the remote edit is preserved
/// in the entry history.
#[test]
fn merge_databases_local_newer_edit_wins_and_preserves_remote_history() {
    let (base, entry_id) = merge_base_db();
    let mut local = base.clone();
    let mut remote = base.clone();
    edit_title(&mut local, entry_id, "v1-local", merge_ts(40));
    edit_title(&mut remote, entry_id, "v1-remote", merge_ts(30));

    let merged = merge_databases(&local, &remote).unwrap();
    let entry = merged.entry(entry_id).unwrap();
    assert_eq!(entry.get_title(), Some("v1-local"));
    let titles = history_titles(&entry);
    assert!(
        titles.contains(&"v1-remote".to_owned()),
        "losing remote edit must land in history: {titles:?}"
    );
}

/// 单改: a remote-only edit applies, and entries created on only one side
/// are kept (local-only) or added (remote-only).
#[test]
fn merge_databases_applies_single_side_changes() {
    let (base, entry_id) = merge_base_db();
    let mut local = base.clone();
    let mut remote = base.clone();
    edit_title(&mut remote, entry_id, "v1-remote-only", merge_ts(30));
    let local_only = add_titled_entry(&mut local, "LocalOnly", merge_ts(20));
    let remote_only = add_titled_entry(&mut remote, "RemoteOnly", merge_ts(25));

    let merged = merge_databases(&local, &remote).unwrap();
    assert_eq!(
        merged.entry(entry_id).unwrap().get_title(),
        Some("v1-remote-only")
    );
    assert_eq!(
        merged.entry(local_only).unwrap().get_title(),
        Some("LocalOnly")
    );
    assert_eq!(
        merged.entry(remote_only).unwrap().get_title(),
        Some("RemoteOnly")
    );
}

/// 删除: an entry deleted locally (moved to the bin) stays deleted when the
/// remote side never touched it after the shared base; the deletion is
/// recorded in `deleted_objects` so it propagates.
#[test]
fn merge_databases_local_deletion_wins_over_older_remote_state() {
    let (base, entry_id) = merge_base_db();
    let mut local = base.clone();
    let remote = base.clone();
    let bin_id = add_bin(&mut local);
    bin_entry(&mut local, entry_id, bin_id, merge_ts(40));

    let merged = merge_databases(&local, &remote).unwrap();
    let entry = merged
        .entry(entry_id)
        .expect("deleted entry must survive in the bin");
    assert_eq!(entry.parent().id(), bin_id, "deletion must win the merge");
    assert!(
        merged.deleted_objects.contains_key(&entry_id.uuid()),
        "the winning deletion must be recorded so it propagates"
    );
}

/// 删除 (reverse): a remote edit newer than the local deletion resurrects
/// the entry with the remote content; the stale bin copy is dropped.
#[test]
fn merge_databases_remote_edit_wins_over_older_local_deletion() {
    let (base, entry_id) = merge_base_db();
    let mut local = base.clone();
    let mut remote = base.clone();
    let bin_id = add_bin(&mut local);
    bin_entry(&mut local, entry_id, bin_id, merge_ts(40));
    edit_title(&mut remote, entry_id, "v1-remote-late", merge_ts(50));

    let merged = merge_databases(&local, &remote).unwrap();
    let entry = merged.entry(entry_id).unwrap();
    assert_eq!(entry.get_title(), Some("v1-remote-late"));
    assert_ne!(
        entry.parent().id(),
        bin_id,
        "a newer remote edit resurrects the entry out of the bin"
    );
}

/// 回收站排除: remote bin contents never merge in; the local bin is kept
/// as-is, and no stray remote bin group leaks into the active tree.
#[test]
fn merge_databases_excludes_recycle_bin_contents() {
    let (base, _) = merge_base_db();
    let mut local = base.clone();
    let mut remote = base.clone();
    // Each side created its own bin after the clone (different bin UUIDs).
    let local_bin = add_bin(&mut local);
    let remote_bin = add_bin(&mut remote);

    let local_binned = add_titled_entry(&mut local, "LocalBinned", merge_ts(20));
    bin_entry(&mut local, local_binned, local_bin, merge_ts(30));
    let remote_binned = add_titled_entry(&mut remote, "RemoteBinned", merge_ts(20));
    bin_entry(&mut remote, remote_binned, remote_bin, merge_ts(30));

    let merged = merge_databases(&local, &remote).unwrap();
    assert!(
        merged.entry(remote_binned).is_none(),
        "remote bin contents must never merge in"
    );
    let entry = merged
        .entry(local_binned)
        .expect("local bin entry must survive the merge");
    assert_eq!(entry.parent().id(), local_bin);
    let root = merged.root();
    let bin_groups: Vec<_> = root.groups().filter(|g| g.name == "回收站").collect();
    assert_eq!(
        bin_groups.len(),
        1,
        "the remote bin shell must not leak into the active tree"
    );
}

/// 历史保留: histories from both sides union without duplicates, sorted
/// semantics preserved (the losing side's current state is pushed last).
#[test]
fn merge_databases_unions_history_from_both_sides() {
    let (base, entry_id) = merge_base_db();
    let mut local = base.clone();
    let mut remote = base.clone();
    edit_title(&mut local, entry_id, "v2", merge_ts(15));
    edit_title(&mut local, entry_id, "v2b", merge_ts(20));
    edit_title(&mut remote, entry_id, "v3", merge_ts(30));

    let merged = merge_databases(&local, &remote).unwrap();
    let entry = merged.entry(entry_id).unwrap();
    assert_eq!(entry.get_title(), Some("v3"));
    let titles = history_titles(&entry);
    assert_eq!(
        titles.len(),
        3,
        "history must dedupe the shared base: {titles:?}"
    );
    for expected in ["v1", "v2", "v2b"] {
        assert!(
            titles.contains(&expected.to_owned()),
            "history must contain {expected}: {titles:?}"
        );
    }
}

/// A genuine conflict: same UUID edited on both sides at the same timestamp
/// with different content cannot be auto-merged.
#[test]
fn merge_databases_same_timestamp_divergence_is_an_error() {
    let (base, entry_id) = merge_base_db();
    let mut local = base.clone();
    let mut remote = base.clone();
    edit_title(&mut local, entry_id, "v1-local", merge_ts(20));
    edit_title(&mut remote, entry_id, "v1-remote", merge_ts(20));

    let err = merge_databases(&local, &remote).unwrap_err();
    assert!(err.contains("同一时间戳"), "unexpected: {err}");
}

/// Moving an entry (drag / bin / restore) stamps LocationChanged, which the
/// merge relies on to resolve deletion-vs-edit conflicts.
#[test]
fn move_and_delete_stamp_location_changed() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = create_session(&dir);
    session.add_entry(&merge_test_input("ToMove")).unwrap();
    let uuid = session.state().unwrap().unwrap().root.entries[0]
        .uuid
        .clone();

    session.delete_entry(&uuid).unwrap();
    {
        let db = session.require_db().unwrap();
        let id = parse_entry_id(&uuid).unwrap();
        let entry = db.entry(id).unwrap();
        assert!(
            entry.times.location_changed.is_some(),
            "move-to-bin must stamp LocationChanged"
        );
    }

    session.restore_entry(&uuid).unwrap();
    {
        let db = session.require_db().unwrap();
        let id = parse_entry_id(&uuid).unwrap();
        let entry = db.entry(id).unwrap();
        assert!(
            entry.times.location_changed.is_some(),
            "restore must keep the LocationChanged stamp"
        );
    }
}

fn merge_test_input(title: &str) -> EntryInput {
    EntryInput {
        group_uuid: ROOT_GROUP_UUID.to_owned(),
        title: title.into(),
        username: "u".into(),
        password: "pw".into(),
        url: String::new(),
        notes: String::new(),
        totp: None,
        expires: None,
        icon: Some(None),
        color: None,
        tags: None,
        custom_fields: vec![],
        attachments: vec![],
    }
}

/// Session-level: merge combines the local unsaved edit with another
/// device's uploaded edit, persists the merged database back, and advances
/// the base hash so the next save succeeds without a conflict.
#[test]
fn merge_remote_vault_merges_both_sides_and_advances_base_hash() {
    let dir = TempDir::new().unwrap();
    let (storage, seed_path) = seed_remote_storage(&dir);
    let local = dir.path().join("local");

    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();

    // Another device: open the same seed, add an entry, save, upload.
    let mut other = VaultSession::default();
    other.open(&seed_path, "pw", None).unwrap();
    other.add_entry(&merge_test_input("RemoteAdded")).unwrap();
    other.save().unwrap();
    storage
        .put("vaults/seed.kdbx", &std::fs::read(&seed_path).unwrap())
        .unwrap();

    // Local unsaved edit.
    session.add_entry(&merge_test_input("LocalAdded")).unwrap();

    let job = session.prepare_remote_merge().unwrap();
    let revision = job.revision;
    let merged = session
        .complete_remote_merge(revision, persist_remote_merge(job).unwrap())
        .unwrap();
    assert!(!merged.dirty);
    let titles: Vec<&str> = merged
        .root
        .entries
        .iter()
        .map(|e| e.title.as_str())
        .collect();
    assert!(
        titles.contains(&"LocalAdded"),
        "local edit must survive: {titles:?}"
    );
    assert!(
        titles.contains(&"RemoteAdded"),
        "remote edit must merge in: {titles:?}"
    );

    // The merged bytes were uploaded: another device opening the remote file
    // sees both entries.
    let key = crate::vault::helpers::build_database_key("pw", None).unwrap();
    let uploaded = Database::parse(&storage.get("vaults/seed.kdbx").unwrap(), key).unwrap();
    let uploaded_titles: Vec<String> = uploaded
        .root()
        .entries()
        .filter_map(|e| e.get_title().map(|t| t.to_owned()))
        .collect();
    assert!(uploaded_titles.contains(&"LocalAdded".to_owned()));
    assert!(uploaded_titles.contains(&"RemoteAdded".to_owned()));

    // Base hash advanced: a later save no longer reports REMOTE_CHANGED.
    session.add_entry(&merge_test_input("PostMerge")).unwrap();
    session.save().unwrap();
}

#[test]
fn remote_merge_completion_rejects_newer_local_edits() {
    let dir = TempDir::new().unwrap();
    let (storage, seed_path) = seed_remote_storage(&dir);
    let local = dir.path().join("local");
    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    session
        .add_entry(&merge_test_input("LocalBeforeMerge"))
        .unwrap();

    let mut other = VaultSession::default();
    other.open(&seed_path, "pw", None).unwrap();
    other.add_entry(&merge_test_input("RemoteAdded")).unwrap();
    other.save().unwrap();
    storage
        .put("vaults/seed.kdbx", &std::fs::read(&seed_path).unwrap())
        .unwrap();

    let job = session.prepare_remote_merge().unwrap();
    let revision = job.revision;
    let merged = persist_remote_merge(job).unwrap();
    session
        .add_entry(&merge_test_input("EditDuringMerge"))
        .unwrap();

    let err = session.complete_remote_merge(revision, merged).unwrap_err();
    assert!(err.contains("已发生修改"));
    let state = session.state().unwrap().unwrap();
    assert!(state.dirty);
    assert!(state
        .root
        .entries
        .iter()
        .any(|entry| entry.title == "EditDuringMerge"));

    // The remote upload did succeed and contains the snapshot merge, while
    // the newer local edit remains only in memory for the next sync.
    let key = crate::vault::helpers::build_database_key("pw", None).unwrap();
    let uploaded = Database::parse(&storage.get("vaults/seed.kdbx").unwrap(), key).unwrap();
    let titles: Vec<String> = uploaded
        .root()
        .entries()
        .filter_map(|entry| entry.get_title().map(ToOwned::to_owned))
        .collect();
    assert!(titles.contains(&"LocalBeforeMerge".to_owned()));
    assert!(titles.contains(&"RemoteAdded".to_owned()));
    assert!(!titles.contains(&"EditDuringMerge".to_owned()));
}

#[test]
fn remote_merge_pre_persist_errors_do_not_trigger_read_only() {
    let dir = TempDir::new().unwrap();
    let (storage, _) = seed_remote_storage(&dir);
    let local = dir.path().join("local");
    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "vaults/seed.kdbx",
            "pw",
            None,
            RemoteMode::InMemory,
            &local,
            3,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();

    // A malformed download fails before the upload/persistence phase. Three
    // retries must not degrade the session to read-only.
    storage.put("vaults/seed.kdbx", &[7u8; 32]).unwrap();
    for _ in 0..3 {
        let err = match persist_remote_merge(session.prepare_remote_merge().unwrap()) {
            Ok(_) => panic!("malformed remote bytes unexpectedly merged"),
            Err(err) => err,
        };
        if err.persist_failure {
            session.note_save_failure();
        }
        assert!(!err.persist_failure);
        assert!(
            err.message.contains("无法打开数据库") || err.message.contains("密码或密钥文件错误")
        );
    }
    assert!(!session.is_read_only());
}
