use super::settings::{col, normalize_file_extension};
use super::RECENT_FILES_MAX;
use super::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn defaults_round_trip_and_persist() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let defaults = store.get().unwrap();
    assert_eq!(defaults.general.theme, "dark");
    assert_eq!(defaults.database.kdf, "Argon2id");
    assert_eq!(defaults.general.font_sizes.base, 14);
    assert_eq!(defaults.general.density.group_gap, 2);
    assert_eq!(defaults.general.density.group_radius, 6);

    let saved = store.set(defaults.clone()).unwrap();
    assert_eq!(saved.general.theme, "dark");

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert_eq!(again.general.theme, "dark");
    assert_eq!(again.general.theme_colors.accent, "#ff5050");
    assert_eq!(again.general.density.group_gap, 2);
}

#[test]
fn old_config_without_density_loads_with_defaults() {
    let dir = TempDir::new().unwrap();
    let conf = dir.path().join("conf");
    fs::create_dir_all(&conf).unwrap();
    let text = r#"{
          "general": {
            "language": "zh-CN",
            "theme": "light",
            "themeColors": {},
            "customPresets": [],
            "compactMode": true,
            "showDescriptions": true,
            "fontSizes": { "base": 14, "secondary": 11, "cardTitle": 13, "cardPreview": 11 },
            "windowEffect": "off",
            "windowOpacity": 100,
            "rememberLastDatabase": true
          },
          "security": { "autoLockMinutes": 5, "clipboardClearSeconds": 20, "minimizeToTray": true, "clearOnLock": true, "lockAfterAction": false },
          "database": { "kdf": "Argon2id", "cipher": "Aes256", "compression": "Gzip", "generator": { "length": 20, "includeUpper": true, "includeLower": true, "includeDigits": true, "includeSymbols": true, "excludeSimilar": false, "excludeAmbiguous": false } }
        }"#;
    fs::write(conf.join("config.json"), text).unwrap();

    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let config = store.get().unwrap();
    assert_eq!(config.general.theme, "light");
    assert!(config.general.compact_mode);
    assert_eq!(config.general.density.group_gap, 2);
    assert!(config.general.density.show_group_icon);
}

#[test]
fn remote_profiles_survive_deserialize_write_reload() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.remote_profiles.push(RemoteProfile {
        name: "Bitiful".into(),
        settings: RemoteSettings {
            kind: "s3".into(),
            endpoint: "http://127.0.0.1:9000".into(),
            region: "cn-east-1".into(),
            bucket: "my-vaults".into(),
            access_key: "AKIA-test".into(),
            secret_key: "s3cret".into(),
            prefix: "vaults/".into(),
            backup_count: 5,
            backup_template: "{name}.{timestamp}.{ext}.bak".into(),
        },
    });
    config.active_remote = 1;
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"backupCount\": 5"));
    assert!(text.contains("\"remoteProfiles\""));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert_eq!(again.remote_profiles.len(), 2);
    assert_eq!(again.remote_profiles[1].name, "Bitiful");
    assert_eq!(
        again.remote_profiles[1].settings.endpoint,
        "http://127.0.0.1:9000"
    );
    assert_eq!(again.remote_profiles[1].settings.bucket, "my-vaults");
    assert_eq!(again.remote_profiles[1].settings.secret_key, "s3cret");
    assert_eq!(again.remote_profiles[1].settings.backup_count, 5);
    // active index is clamped to the last valid profile
    assert_eq!(again.active_remote, 1);
    // the legacy field must not be re-serialized
    assert!(!text.contains("\"remote\":"));
}

#[test]
fn legacy_single_remote_migrates_into_first_profile() {
    let dir = TempDir::new().unwrap();
    let conf = dir.path().join("conf");
    fs::create_dir_all(&conf).unwrap();
    let legacy = r#"{"remote": {
            "endpoint": "https://s3.bitiful.net", "region": "cn-east-1",
            "bucket": "muuyo", "accessKey": "AK", "secretKey": "SK",
            "prefix": "vaults/", "localDir": "remote", "backupCount": 3
        }}"#;
    fs::write(conf.join("config.json"), legacy).unwrap();

    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let config = store.get().unwrap();
    assert_eq!(config.remote_profiles.len(), 1);
    assert_eq!(config.remote_profiles[0].name, "默认");
    assert_eq!(
        config.remote_profiles[0].settings.endpoint,
        "https://s3.bitiful.net"
    );
    assert_eq!(config.remote_profiles[0].settings.bucket, "muuyo");
    assert_eq!(config.active_remote, 0);
    // re-saving writes the new shape only
    store.set(config.clone()).unwrap();
    let text = std::fs::read_to_string(conf.join("config.json")).unwrap();
    assert!(text.contains("\"remoteProfiles\""));
    assert!(!text.contains("\"accessKey\": \"AK\""));
}

#[cfg(target_os = "windows")]
#[test]
fn remote_secrets_never_persist_in_plaintext() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.remote_profiles[0].settings.access_key = "AKIA-secret-access".into();
    config.remote_profiles[0].settings.secret_key = "plaintext-s3cret".into();
    store.set(config).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(!text.contains("AKIA-secret-access"));
    assert!(!text.contains("plaintext-s3cret"));
    assert!(text.contains("\"accessKey\": \"dpapi1:"));
    assert!(text.contains("\"secretKey\": \"dpapi1:"));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert_eq!(
        again.remote_profiles[0].settings.access_key,
        "AKIA-secret-access"
    );
    assert_eq!(
        again.remote_profiles[0].settings.secret_key,
        "plaintext-s3cret"
    );
}

#[test]
fn remote_defaults_and_normalization_rules() {
    let mut config = AppConfig::default();
    assert_eq!(
        config.remote_profiles[0].settings.endpoint,
        "https://s3.amazonaws.com"
    );
    assert_eq!(config.remote_profiles[0].settings.backup_count, 3);
    assert_eq!(config.remote_profiles[0].settings.kind, "s3");

    config.remote_profiles[0].settings.endpoint = "  https://s3.example.com  ".into();
    config.remote_profiles[0].settings.backup_count = 99;
    config.active_remote = 42;
    let normalized = normalize_config(config);
    assert_eq!(
        normalized.remote_profiles[0].settings.endpoint,
        "https://s3.example.com"
    );
    assert_eq!(normalized.remote_profiles[0].settings.backup_count, 3);
    assert_eq!(normalized.active_remote, 0);
}

#[test]
fn remote_profile_names_are_unique_and_suffixed_on_normalize() {
    let mut config = AppConfig::default();
    config.remote_profiles.push(RemoteProfile {
        name: "Bitiful".into(),
        settings: RemoteSettings::default(),
    });
    config.remote_profiles.push(RemoteProfile {
        name: " 默认 ".into(),
        settings: RemoteSettings::default(),
    });
    config.remote_profiles.push(RemoteProfile {
        name: "Bitiful".into(),
        settings: RemoteSettings::default(),
    });
    let normalized = normalize_config(config);
    let names: Vec<&str> = normalized
        .remote_profiles
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    // Empty → 默认 (deduped to 默认 (2)), duplicate → suffixed.
    assert_eq!(names, vec!["默认", "Bitiful", "默认 (2)", "Bitiful (2)"]);
}

#[test]
fn remote_kind_normalizes_to_s3_or_webdav() {
    let mut config = AppConfig::default();
    config.remote_profiles[0].settings.kind = "  WebDAV ".into();
    assert_eq!(
        normalize_config(config).remote_profiles[0].settings.kind,
        "webdav"
    );

    let mut config = AppConfig::default();
    config.remote_profiles[0].settings.kind = "ftp".into();
    assert_eq!(
        normalize_config(config).remote_profiles[0].settings.kind,
        "s3"
    );

    let mut config = AppConfig::default();
    config.remote_profiles[0].settings.kind = "".into();
    assert_eq!(
        normalize_config(config).remote_profiles[0].settings.kind,
        "s3"
    );
}

#[test]
fn file_extension_and_backup_template_normalize() {
    let mut config = AppConfig::default();
    assert_eq!(config.database.file_extension, "kdbx");
    assert_eq!(
        config.remote_profiles[0].settings.backup_template,
        "{name}.{timestamp}.{ext}.bak"
    );

    config.database.file_extension = "  .kdb!x  ".into();
    config.remote_profiles[0].settings.backup_template = "   ".into();
    let normalized = normalize_config(config);
    assert_eq!(normalized.database.file_extension, "kdbx");

    let mut config = AppConfig::default();
    config.database.file_extension = "txt".into();
    config.remote_profiles[0].settings.backup_template = "  {name}-{timestamp}.{ext}.old  ".into();
    let normalized = normalize_config(config);
    assert_eq!(normalized.database.file_extension, "txt");
    assert_eq!(
        normalized.remote_profiles[0].settings.backup_template,
        "{name}-{timestamp}.{ext}.old"
    );
    assert_eq!(normalize_file_extension(""), "kdbx");
    assert_eq!(normalize_file_extension("..."), "kdbx");
    assert_eq!(normalize_file_extension("KDBX"), "KDBX");
}

#[test]
fn remote_settings_resolves_profile_and_clamps_index() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.remote_profiles.push(RemoteProfile {
        name: "Bitiful".into(),
        settings: RemoteSettings {
            endpoint: "http://127.0.0.1:9000".into(),
            ..RemoteSettings::default()
        },
    });
    config.active_remote = 1;
    store.set(config).unwrap();

    let first = store.remote_settings(0).unwrap();
    assert_eq!(first.endpoint, "https://s3.amazonaws.com");
    let second = store.remote_settings(1).unwrap();
    assert_eq!(second.endpoint, "http://127.0.0.1:9000");
    // out-of-range indices clamp to the last valid profile
    let clamped = store.remote_settings(9).unwrap();
    assert_eq!(clamped.endpoint, "http://127.0.0.1:9000");

    // remote_profile returns the same profile's name + settings
    let (name, settings) = store.remote_profile(1).unwrap();
    assert_eq!(name, "Bitiful");
    assert_eq!(settings.endpoint, "http://127.0.0.1:9000");
    let (clamped_name, _) = store.remote_profile(9).unwrap();
    assert_eq!(clamped_name, "Bitiful");
}

#[test]
fn empty_config_object_loads_with_defaults() {
    let dir = TempDir::new().unwrap();
    let conf = dir.path().join("conf");
    fs::create_dir_all(&conf).unwrap();
    fs::write(conf.join("config.json"), "{}").unwrap();

    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let config = store.get().unwrap();
    assert_eq!(config.general.theme, "dark");
    assert_eq!(config.general.density.group_gap, 2);
    assert_eq!(config.security.clipboard_clear_seconds, 20);
    assert_eq!(config.database.generator.length, 20);
    assert!(!config.security.lock_on_focus_loss);
    assert!(!config.security.remember_password);
}

#[test]
fn remember_password_survives_deserialize_write_reload() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.security.remember_password = true;
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"rememberPassword\": true"));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert!(again.security.remember_password);
}

#[test]
fn lock_on_focus_loss_survives_deserialize_write_reload() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.security.lock_on_focus_loss = true;
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"lockOnFocusLoss\": true"));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert!(again.security.lock_on_focus_loss);
}

#[test]
fn entry_columns_roundtrip_keeps_ids_and_clamps_widths() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.general.entry_columns = vec![
        col("title", true, 0),
        col("username", true, 120),
        col("custom:手机号", true, 999),
        col("notes", true, 10),
    ];
    store.set(config.clone()).unwrap();
    let normalized = store.get().unwrap();
    let by_id: std::collections::HashMap<&str, &EntryColumnState> = normalized
        .general
        .entry_columns
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect();
    // title keeps its 0 flex sentinel
    assert_eq!(by_id["title"].width, 0);
    assert!(by_id["title"].visible);
    // normal widths pass through
    assert_eq!(by_id["username"].width, 120);
    // custom-field ids survive the round-trip
    assert_eq!(by_id["custom:手机号"].width, 400);
    // out-of-range widths clamp
    assert_eq!(by_id["notes"].width, 30);
    assert_eq!(normalized.general.entry_columns.len(), 4);
}

#[test]
fn empty_config_object_gets_default_entry_columns() {
    let dir = TempDir::new().unwrap();
    let conf = dir.path().join("conf");
    fs::create_dir_all(&conf).unwrap();
    fs::write(conf.join("config.json"), "{}").unwrap();

    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let config = store.get().unwrap();
    assert_eq!(
        config.general.entry_columns.len(),
        default_entry_columns().len()
    );
    assert_eq!(config.general.entry_columns[0].id, "title");
    assert_eq!(config.general.entry_columns[0].width, 0);
}

#[test]
fn screen_capture_guard_defaults_off_and_survives_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    assert!(!store.get().unwrap().security.screen_capture_guard);

    let mut config = AppConfig::default();
    config.security.screen_capture_guard = true;
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"screenCaptureGuard\": true"));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    assert!(reloaded.get().unwrap().security.screen_capture_guard);
}

#[test]
fn recent_files_survive_deserialize_write_reload() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.general.recent_files = vec!["C:/vault-a.kdbx".into(), "C:/vault-b.kdbx".into()];
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"recentFiles\""));
    assert!(text.contains("C:/vault-a.kdbx"));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert_eq!(
        again.general.recent_files,
        vec!["C:/vault-a.kdbx".to_owned(), "C:/vault-b.kdbx".to_owned()]
    );
}

#[test]
fn recent_files_normalize_trims_dedups_and_caps() {
    let mut config = AppConfig::default();
    config.general.recent_files = vec![
        "  dup.kdbx ".into(),
        "dup.kdbx".into(),
        "".into(),
        "  ".into(),
        "keep.kdbx".into(),
    ];
    (0..RECENT_FILES_MAX).for_each(|i| config.general.recent_files.push(format!("f{i}.kdbx")));

    let normalized = normalize_config(config);
    assert_eq!(
        normalized.general.recent_files,
        vec![
            "dup.kdbx".to_owned(),
            "keep.kdbx".to_owned(),
            "f0.kdbx".to_owned(),
            "f1.kdbx".to_owned(),
            "f2.kdbx".to_owned(),
            "f3.kdbx".to_owned(),
            "f4.kdbx".to_owned(),
            "f5.kdbx".to_owned(),
        ]
    );
}

#[test]
fn window_and_panel_sizes_survive_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.general.window_width = 1024;
    config.general.window_height = 768;
    config.general.panel_widths.group = 220;
    config.general.panel_widths.detail = 360;
    config.general.panel_widths.url_col = 260;
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"panelWidths\""));
    assert!(text.contains("\"urlCol\": 260"));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert_eq!(again.general.window_width, 1024);
    assert_eq!(again.general.window_height, 768);
    assert_eq!(again.general.panel_widths.group, 220);
    assert_eq!(again.general.panel_widths.detail, 360);
    assert_eq!(again.general.panel_widths.url_col, 260);
}

#[test]
fn window_and_panel_sizes_are_clamped() {
    let mut config = AppConfig::default();
    config.general.window_width = 300;
    config.general.window_height = 99_999;
    config.general.panel_widths.group = 100;
    config.general.panel_widths.detail = 999;
    config.general.panel_widths.url_col = 1;

    let normalized = normalize_config(config);
    assert_eq!(normalized.general.window_width, 1100);
    assert_eq!(normalized.general.window_height, 720);
    assert_eq!(normalized.general.panel_widths.group, 200);
    assert_eq!(normalized.general.panel_widths.detail, 300);
    assert_eq!(normalized.general.panel_widths.url_col, 200);
}

#[test]
fn density_survives_deserialize_write_reload() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let mut config = AppConfig::default();
    config.general.density.group_gap = 8;
    config.general.density.group_padding_y = 6;
    config.general.density.group_indent = 20;
    config.general.density.group_radius = 4;
    config.general.density.show_group_icon = false;
    config.general.density.show_group_chevron = false;
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"density\""));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert_eq!(again.general.density.group_gap, 8);
    assert_eq!(again.general.density.group_indent, 20);
    assert!(!again.general.density.show_group_chevron);
}

#[test]
fn keyboard_auto_type_global_round_trips_and_defaults_empty() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    assert_eq!(store.get().unwrap().keyboard.auto_type_global, "");

    let mut config = AppConfig::default();
    config.keyboard.auto_type_global = "Ctrl+Shift+A".into();
    config
        .keyboard
        .shortcuts
        .insert("save".into(), "Ctrl+S".into());
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"autoTypeGlobal\": \"Ctrl+Shift+A\""));
    assert!(text.contains("\"save\": \"Ctrl+S\""));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let again = reloaded.get().unwrap();
    assert_eq!(again.keyboard.auto_type_global, "Ctrl+Shift+A");
    assert_eq!(
        again.keyboard.shortcuts.get("save"),
        Some(&"Ctrl+S".to_string())
    );
}

#[test]
fn old_config_without_hotkey_field_loads_with_defaults() {
    let dir = TempDir::new().unwrap();
    let conf = dir.path().join("conf");
    std::fs::create_dir_all(&conf).unwrap();
    std::fs::write(conf.join("config.json"), r#"{"general":{"theme":"light"}}"#).unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    assert_eq!(store.get().unwrap().keyboard.auto_type_global, "");
}

#[test]
fn legacy_global_auto_type_shortcut_migrates_into_keyboard_section() {
    let dir = TempDir::new().unwrap();
    let conf = dir.path().join("conf");
    std::fs::create_dir_all(&conf).unwrap();
    std::fs::write(
        conf.join("config.json"),
        r#"{"general":{"globalAutoTypeShortcut":"Ctrl+Shift+A"}}"#,
    )
    .unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    let loaded = store.get().unwrap();
    assert_eq!(loaded.keyboard.auto_type_global, "Ctrl+Shift+A");
    assert_eq!(loaded.general.global_auto_type_shortcut, "");

    // A re-save writes it under `keyboard`, never the legacy field.
    store.set(loaded).unwrap();
    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"keyboard\""));
    assert!(text.contains("\"autoTypeGlobal\": \"Ctrl+Shift+A\""));
    assert!(!text.contains("globalAutoTypeShortcut"));
}

#[test]
fn normalization_clamps_and_fixes_enums() {
    let mut config = AppConfig::default();
    config.general.theme = "neon".into();
    config.general.window_effect = "glass".into();
    config.general.window_opacity = 500;
    config.security.auto_lock_minutes = 9999;
    config.database.kdf = "scrypt".into();
    config.database.cipher = "des".into();
    config.database.compression = "zip".into();
    config.database.generator.length = 3;
    config.general.font_sizes.base = 5;
    config.general.theme_colors.accent = "not-a-color".into();
    config.general.density.group_gap = 99;
    config.general.density.group_indent = 0;

    let normalized = normalize_config(config);
    assert_eq!(normalized.general.theme, "dark");
    assert_eq!(normalized.general.window_effect, "off");
    assert_eq!(normalized.general.window_opacity, 100);
    assert_eq!(normalized.security.auto_lock_minutes, 5);
    assert_eq!(normalized.database.kdf, "Argon2id");
    assert_eq!(normalized.database.cipher, "Aes256");
    assert_eq!(normalized.database.compression, "Gzip");
    assert_eq!(normalized.database.generator.length, 20);
    assert_eq!(normalized.general.font_sizes.base, 14);
    assert_eq!(normalized.general.theme_colors.accent, "#ff5050");
    assert_eq!(normalized.general.density.group_gap, 2);
    assert_eq!(normalized.general.density.group_indent, 12);
}

#[test]
fn bridge_defaults_off_and_survives_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    assert!(!store.get().unwrap().bridge.enabled);

    let mut config = AppConfig::default();
    config.bridge.enabled = true;
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"bridge\": {"));
    assert!(text.contains("\"enabled\": true"));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    assert!(reloaded.get().unwrap().bridge.enabled);
}

#[test]
fn rpc_defaults_off_keep_session_on_and_survive_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    assert!(!store.get().unwrap().rpc.enabled);
    assert!(store.get().unwrap().rpc.keep_session_after_lock);

    let mut config = AppConfig::default();
    config.rpc.enabled = true;
    config.rpc.keep_session_after_lock = false;
    store.set(config.clone()).unwrap();

    let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
    assert!(text.contains("\"rpc\": {"));
    assert!(text.contains("\"enabled\": true"));
    assert!(text.contains("\"keepSessionAfterLock\": false"));

    let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
    assert!(reloaded.get().unwrap().rpc.enabled);
    assert!(!reloaded.get().unwrap().rpc.keep_session_after_lock);
}

#[test]
fn normalization_is_idempotent() {
    let mut config = AppConfig::default();
    config.general.compact_mode = true;
    let once = normalize_config(config);
    let twice = normalize_config(once.clone());
    assert_eq!(
        serde_json::to_string(&once).unwrap(),
        serde_json::to_string(&twice).unwrap()
    );
}
