# Data Contracts

This reference defines the frontend ↔ backend contract. Update it together with any change to IPC commands, serde types, config schema, or vault serialization.

## Config contract

`config.rs` persists `conf/config.json` beside the executable/project. The Rust `AppConfig` serde shape mirrors the frontend `AppSettings` (`src/lib/types/settings.ts`):

| Rust field | TypeScript         | Notes                                                                                                                                                                                                                                                                                        |
| ---------- | ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `general`  | `GeneralSettings`  | `language`, `theme`, `themeColors`, `customPresets`, `compactMode`, `density{groupGap,groupPaddingY,groupIndent,groupRadius,showGroupIcon,showGroupChevron}`, `showDescriptions`, `fontSizes{base,secondary,cardTitle,cardPreview}`, `windowEffect`, `windowOpacity`, `rememberLastDatabase` |
| `security` | `SecuritySettings` | `autoLockMinutes`, `clipboardClearSeconds`, `minimizeToTray`, `clearOnLock`, `lockAfterAction`                                                                                                                                                                                               |
| `database` | `DatabaseDefaults` | `kdf` (`Argon2id`/`Argon2`/`Aes`), `cipher` (`Aes256`/`ChaCha20`), `compression` (`None`/`Gzip`), `generator{length,includeUpper,includeLower,includeDigits,includeSymbols,excludeSimilar,excludeAmbiguous}`                                                                                 |

- `get_config() -> AppConfig`: read + normalize; returns defaults on first run.
- `set_config(config: AppConfig) -> AppConfig`: normalize + atomic save; returns the normalized value.

Frontend normalization lives in `services/settings.ts::normalizeSettings`; the backend must apply the same range/default rules so the round-trip is idempotent. New settings fields require both sides plus tests.

> **Pitfall — serde silently drops unknown fields.** The Rust `AppConfig` structs have no `deny_unknown_fields`, so a frontend field missing from the Rust serde shape is quietly discarded on `set_config`, written to disk, and returned as absent — the UI keeps it in memory via the fallback, but it **resets on the next launch**. Symptom: "my settings don't stick after restart", especially per-sub-item values while a parent toggle (e.g. `compactMode`) persists. Every new settings field must be mirrored in: TypeScript type + defaults + `normalizeSettings` (frontend), Rust serde struct + `Default` + `normalize_config` clamp (backend), and this table. The regression test `density_survives_deserialize_write_reload` guards the round-trip.

> **Pitfall — the read side: missing fields must not crash load.** Deserialization is strict by default: a required field absent from an existing `config.json` (old version, manual edit) makes `ConfigStore::load` fail, which propagates from the setup hook and **panics the app on startup**. All settings structs therefore use container-level `#[serde(default)]` (every struct also impls `Default`), so older files load with defaults for missing fields and `normalize_config` heals them. When adding a field, keep the `Default` impl updated — no per-field `#[serde(default)]` is needed, but a test loading a config without the new field is required. Guarded by `old_config_without_density_loads_with_defaults` and `empty_config_object_loads_with_defaults`.

## Vault IPC contract

The vault session is held in backend managed state and returned to the frontend as serialized `VaultState`.

### Shared types (Rust serde ↔ `src/lib/types/vault.ts`)

| Type         | Fields                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `VaultEntry` | `uuid`, `groupUuid`, `title`, `username`, `password`, `url`, `notes`, `totp?`, `icon?`, `created?`, `modified?`, `tags?`, `favorite: bool` |
| `VaultGroup` | `uuid`, `parentUuid` (null for root), `name`, `icon?`, `children: VaultGroup[]`, `entries: VaultEntry[]`                                   |
| `VaultState` | `path`, `fileName`, `password`, `root: VaultGroup`, `dirty: bool`, `modifiedAt`                                                            |

The root group is the virtual top-level (uuid `"root"`); it is not persisted as a KeePass group — its `children` map to top-level groups and its `entries` map to the DB root group entries.

### Commands

| Command           | Args                                       | Result               | Notes                             |
| ----------------- | ------------------------------------------ | -------------------- | --------------------------------- |
| `open_vault`      | `path: String, password: String`           | `VaultState`         | Opens KDBX with password key      |
| `create_vault`    | `path, password, kdf, cipher, compression` | `VaultState`         | Creates empty vault and saves     |
| `close_vault`     | –                                          | `()`                 | Clears session; zeroizes password |
| `get_vault_state` | –                                          | `Option<VaultState>` | null when no session              |
| `save_vault`      | –                                          | `VaultState`         | Persists session DB; resets dirty |
| `add_entry`       | `input: EntryInput`                        | `VaultState`         |                                   |
| `update_entry`    | `uuid, input: EntryInput`                  | `VaultState`         |                                   |
| `delete_entry`    | `uuid`                                     | `VaultState`         |                                   |
| `toggle_favorite` | `uuid`                                     | `VaultState`         | Flips the pinned marker           |
| `totp_code`       | `uuid`                                     | `TotpCode`           | `{ code, validFor, period }`      |
| `add_group`       | `input: GroupInput`                        | `VaultState`         | parentUuid null → root            |
| `rename_group`    | `uuid, name`                               | `VaultState`         |                                   |
| `delete_group`    | `uuid`                                     | `VaultState`         | Entries/children bubble to root   |

Every mutating command returns the refreshed `VaultState` so the frontend `vault` store stays in sync with the backend session. `favorite` is always present on `VaultEntry` and is persisted as a custom field `KeyVault.Favorite = "true"` (absent when not pinned); the browser fallback mirrors the same boolean on the demo state. `totp_code` is read-only: it computes the current one-time code via `keepass::db::TOTP` (`TotpCode { code, validFor, period }`), accepting either an `otpauth://` URI or a raw Base32 key (wrapped with SHA-1 / 6 digits / 30s defaults). The frontend `TotpWidget` counts down locally and refetches at each period boundary.

### Browser fallback

Outside Tauri, `services/vault.ts` simulates the same commands on `demo-vault.ts` data persisted to `localStorage`. It is a UI-development surface only and must never be used as evidence for desktop KDBX behavior.

## Serialization rules

- KDBX entry/group uuids are rendered as hex strings; group UUIDs are stable for the session.
- `password` round-trips only inside the vault session; it never enters config, logs, or the browser fallback except as demo data.
- `modifiedAt` is refreshed on every mutation.

## Cross-cutting gates

- Command names, arg names, serde casing (camelCase), error strings, and tests must stay aligned across `invoke` calls and Rust handlers.
- A new mutation requires: Rust handler + test, frontend `vault.ts` method, UI wiring, and this reference update.
