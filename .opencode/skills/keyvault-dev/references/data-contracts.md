# Data Contracts

This reference defines the frontend ↔ backend contract. Update it together with any change to IPC commands, serde types, config schema, or vault serialization.

## Config contract

`config.rs` persists `conf/config.json` beside the executable/project. The Rust `AppConfig` serde shape mirrors the frontend `AppSettings` (`src/lib/types/settings.ts`):

| Rust field | TypeScript         | Notes                                                                                                                                                                                                        |
| ---------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `general`  | `GeneralSettings`  | `language`, `theme`, `themeColors`, `customPresets`, `compactMode`, `showDescriptions`, `fontSizes{base,secondary,cardTitle,cardPreview}`, `windowEffect`, `windowOpacity`, `rememberLastDatabase`           |
| `security` | `SecuritySettings` | `autoLockMinutes`, `clipboardClearSeconds`, `minimizeToTray`, `clearOnLock`, `lockAfterAction`                                                                                                               |
| `database` | `DatabaseDefaults` | `kdf` (`Argon2id`/`Argon2`/`Aes`), `cipher` (`Aes256`/`ChaCha20`), `compression` (`None`/`Gzip`), `generator{length,includeUpper,includeLower,includeDigits,includeSymbols,excludeSimilar,excludeAmbiguous}` |

- `get_config() -> AppConfig`: read + normalize; returns defaults on first run.
- `set_config(config: AppConfig) -> AppConfig`: normalize + atomic save; returns the normalized value.

Frontend normalization lives in `services/settings.ts::normalizeSettings`; the backend must apply the same range/default rules so the round-trip is idempotent. New settings fields require both sides plus tests.

## Vault IPC contract

The vault session is held in backend managed state and returned to the frontend as serialized `VaultState`.

### Shared types (Rust serde ↔ `src/lib/types/vault.ts`)

| Type         | Fields                                                                                                                   |
| ------------ | ------------------------------------------------------------------------------------------------------------------------ |
| `VaultEntry` | `uuid`, `groupUuid`, `title`, `username`, `password`, `url`, `notes`, `totp?`, `icon?`, `created?`, `modified?`, `tags?` |
| `VaultGroup` | `uuid`, `parentUuid` (null for root), `name`, `icon?`, `children: VaultGroup[]`, `entries: VaultEntry[]`                 |
| `VaultState` | `path`, `fileName`, `password`, `root: VaultGroup`, `dirty: bool`, `modifiedAt`                                          |

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
| `totp_code`       | `uuid`                                     | `TotpCode`           | `{ code, validFor, period }`      |
| `add_group`       | `input: GroupInput`                        | `VaultState`         | parentUuid null → root            |
| `rename_group`    | `uuid, name`                               | `VaultState`         |                                   |
| `delete_group`    | `uuid`                                     | `VaultState`         | Entries/children bubble to root   |

Every mutating command returns the refreshed `VaultState` so the frontend `vault` store stays in sync with the backend session. `totp_code` is read-only: it computes the current one-time code via `keepass::db::TOTP` (`TotpCode { code, validFor, period }`), accepting either an `otpauth://` URI or a raw Base32 key (wrapped with SHA-1 / 6 digits / 30s defaults). The frontend `TotpWidget` counts down locally and refetches at each period boundary.

### Browser fallback

Outside Tauri, `services/vault.ts` simulates the same commands on `demo-vault.ts` data persisted to `localStorage`. It is a UI-development surface only and must never be used as evidence for desktop KDBX behavior.

## Serialization rules

- KDBX entry/group uuids are rendered as hex strings; group UUIDs are stable for the session.
- `password` round-trips only inside the vault session; it never enters config, logs, or the browser fallback except as demo data.
- `modifiedAt` is refreshed on every mutation.

## Cross-cutting gates

- Command names, arg names, serde casing (camelCase), error strings, and tests must stay aligned across `invoke` calls and Rust handlers.
- A new mutation requires: Rust handler + test, frontend `vault.ts` method, UI wiring, and this reference update.
