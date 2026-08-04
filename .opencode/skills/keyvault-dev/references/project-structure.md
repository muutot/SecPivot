# Project Structure and Runtime Surfaces

This reference is a source map, not a substitute for reading the current files. Update this map when ownership or entry points change.

## Runtime surfaces

| Surface             | Entry point                                                                                              | Responsibility                                                                                                                                                   |
| ------------------- | -------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Main desktop window | `src/routes/+page.svelte`                                                                                | Welcome/unlock, group tree, entry list, detail, search, editor dialogs, status                                                                                   |
| Settings window     | `src/routes/settings/+page.svelte` → `SettingsDialog.svelte`                                             | Standalone settings page, theme/font application, settings navigation/panels                                                                                     |
| Frontend services   | `src/lib/services/*`                                                                                     | Settings store + bootstrap, vault IPC wrapper + browser fallback                                                                                                 |
| GUI backend         | `src-tauri/src/lib.rs`                                                                                   | Tauri setup, managed config + vault session, commands                                                                                                            |
| Backend modules     | `src-tauri/src/config.rs`, `src-tauri/src/vault.rs`, `src-tauri/src/autotype.rs`, `src-tauri/src/otp.rs` | Config persistence; KeePass open/create/edit/save session; auto-type sequence parsing + enigo replay; OTP primitives (RFC 6238 TOTP, RFC 4226 HOTP, Steam Guard) |
| Release automation  | `scripts/version.mjs`, `changelog.mjs`, `release.mjs`                                                    | Atomic version bump, gitmoji changelog, two-pass release orchestration                                                                                           |
| CI / CD             | `.github/workflows/ci.yml`, `release.yml`                                                                | `npm run verify` on push/PR; tagged multi-platform build + draft release                                                                                         |
| Windows installer   | `src-tauri/windows/installer.nsi`                                                                        | Custom NSIS template wired via `bundle.windows.nsis.template`                                                                                                    |

SvelteKit runs as a static SPA: `src/routes/+layout.ts` disables SSR and awaits `appSettings.initialize()` before route load. `+layout.svelte` imports global CSS and applies settings to the document.

## Frontend ownership

| Path                                             | Ownership                                                                                                                                                                                       |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/app.css`                                    | Global reset, theme defaults, font variables, accessibility media rules; imports shared settings CSS                                                                                            |
| `src/lib/styles/settings-shared.css`             | Reusable settings-panel primitives (header/cards/toggles/slider/feedback)                                                                                                                       |
| `src/lib/types/theme.ts`                         | `ThemeColors`, `DARK_THEME_COLORS`, `LIGHT_THEME_COLORS`                                                                                                                                        |
| `src/lib/types/settings.ts`                      | `AppSettings`, `GeneralSettings`, `SecuritySettings`, `DatabaseDefaults`                                                                                                                        |
| `src/lib/types/vault.ts`                         | `VaultState`, `VaultGroup`, `VaultEntry`, request/input shapes                                                                                                                                  |
| `src/lib/utils/theme.ts`                         | `ThemeColors` → CSS-variable mapping                                                                                                                                                            |
| `src/lib/utils/password.ts`                      | Password generator, entropy estimate, strength label                                                                                                                                            |
| `src/lib/utils/csv.ts`                           | RFC 4180 CSV build/parse (import + export)                                                                                                                                                      |
| `src/lib/utils/tree.ts`                          | Vault-group tree primitives (`walkGroups`/`collectGroups`/`collectEntries`/`findGroup`/`findEntry`/`findBinGroup`/`countEntries`)                                                               |
| `src/lib/utils/format.ts`                        | Byte-size display formatting (`formatBytes`)                                                                                                                                                    |
| `src/lib/utils/date.ts`                          | Date formatting (column `YYYY-MM-DD`, `datetime-local` input value, localized long date)                                                                                                        |
| `src/lib/utils/kdbx-xml.ts`                      | KeePass 2.x XML import parser (DOMParser → `XmlImportEntry[]`, groups as `A / B` paths, Protected base64 decode, custom fields)                                                                 |
| `src/lib/utils/totp.ts`                          | Browser-fallback TOTP (RFC 6238, WebCrypto); desktop uses backend `totp_code`                                                                                                                   |
| `src-tauri/src/otp.rs`                           | Pure one-time-password primitives: TOTP (RFC 6238), HOTP (RFC 4226), Steam Guard; seed parsing + field contract                                                                                 |
| `src/lib/utils/clipboard.ts`                     | Clipboard copy + scheduled clear (`clipboardClearSeconds`)                                                                                                                                      |
| `src/lib/services/settings.ts`                   | `appSettings` store, defaults, normalization, debounced persistence                                                                                                                             |
| `src/lib/services/settings-bootstrap.ts`         | Apply settings to document (theme colors, font vars, window effect)                                                                                                                             |
| `src/lib/services/security.ts`                   | `lockVault`/`copySensitive`/`armIdleLock`/`installAutoLock` (lock lifecycle, clipboard-clear gating)                                                                                            |
| `src/lib/composables/useTotpCode.svelte.ts`      | Shared reactive OTP poller (fetch loop, countdown, HOTP static code, copy+flash); the OTP skins (`TotpWidget`, `EntryTotpBadge`) are pure renderers over it                                     |
| `src/lib/services/vault.ts`                      | `vault` store: open/create/close/save + entry/group CRUD + `remembered` path (Tauri + browser)                                                                                                  |
| `src/lib/data/demo-vault.ts`                     | Browser-preview fallback data; not proof of desktop KDBX behavior                                                                                                                               |
| `src/lib/components/AppIcon.svelte`              | Hand-written inline SVG icon set (stroke `currentColor`)                                                                                                                                        |
| `src/lib/components/ContextMenu.svelte`          | Viewport-fixed right-click menu (items, destructive style, click-outside/Escape close)                                                                                                          |
| `src/lib/components/SettingsDialog.svelte`       | Settings shell: sidebar nav + content pane + panels                                                                                                                                             |
| `src/lib/components/settings/*`                  | General / Security / Database / Remote / Integrations (KeePassHttp `BridgeSettingsPanel` + KeePassRPC `RpcSettingsPanel`) / About panels                                                        |
| `src/lib/components/BridgeApprovalPrompt.svelte` | Global browser-association approval modal; listens for `bridge-associate-request` and answers via `bridge_approve` (mounted in `+layout.svelte`, skipped on the TCATO overlay)                  |
| `src/lib/components/RpcSideChannelPrompt.svelte` | Global KeePassRPC side-channel modal; listens for `rpc-side-channel-request`, shows the one-time SRP password with a countdown/copy (mounted in `+layout.svelte`, skipped on the TCATO overlay) |
| `src/lib/components/VaultWelcome.svelte`         | Welcome/unlock + open/create database modal flows                                                                                                                                               |
| `src/lib/components/LockScreen.svelte`           | Lock screen: reopen remembered path with password, or switch to another database                                                                                                                |
| `src/lib/components/TotpWidget.svelte`           | OTP code readout with countdown bar; TOTP/Steam refetch per period, HOTP shows a static code + counter                                                                                          |
| `src/lib/components/EntryTotpBadge.svelte`       | List-row OTP badge (TOTP/Steam countdown bar; HOTP static, no bar); copy on click                                                                                                               |

## Backend ownership

| Path                               | Ownership                                                                                                                                                                                                                                                                                                           |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/lib.rs`             | Tauri builder, managed state (`AppState`), command registration, setup, system tray (Show/Lock/Quit + minimize-to-tray close handling), global auto-type hotkey (register/re-register + handler), window close handling                                                                                             |
| `src-tauri/src/commands.rs`        | All Tauri IPC command handlers, grouped by domain: config, bridge/RPC status + approval, credential store, vault open/create/save/change-key, entry & group CRUD, favicon download, TCATO overlay, clipboard, S3 remote (extracted from lib.rs)                                                                     |
| `src-tauri/src/config/mod.rs` | Config module root: `pub mod settings`/`store`, re-exports (`AppConfig`, `normalize_config`, `ConfigStore`, `RECENT_FILES_MAX`, `DEFAULT_BACKUP_TEMPLATE`)                                                                    |
| `src-tauri/src/config/settings.rs` | `config.json` serde shapes (mirror `AppSettings`), defaults, normalization incl. remote backup template + file extension (extracted from config.rs)                                                                    |
| `src-tauri/src/config/store.rs` | Atomic persistence to `<project_dir>/conf/config.json` (`read_config`/`write_config`) + managed `ConfigStore` (extracted from config.rs)                                                                    |
| `src-tauri/src/config/tests.rs` | Config schema/defaults/normalization/persistence test suite (extracted from config.rs)                                                                    |
| `src-tauri/src/vault/mod.rs` | Vault module root: `VaultSession` struct + `RemoteMode`/`RemoteTarget`, shared field constants, `vault::*` re-exports (`dto` types, `helpers`/`persist` free functions)                                                                                                                                       |
| `src-tauri/src/vault/session.rs` | `VaultSession` lifecycle impl: open/create/close/state/save/change_master_key/save-as, adopt/replace, snapshot + prepare_save/change/complete_save/change internals (extracted from mod.rs)                                                                                                                    |
| `src-tauri/src/vault/entries.rs` | `VaultSession` entry & group CRUD impl: add/update/move/delete, history + restore, group add/rename/delete/restore, recycle-bin empty (extracted from mod.rs)                                                                                                                                                |
| `src-tauri/src/vault/security.rs` | `VaultSession` OTP/favorites/favicons/auto-type impl: totp_code, toggle_favorite, favicon jobs + apply, autotype context/expand/match, get_entry_password/totp, security_report, export_csv (extracted from mod.rs)                                                                                            |
| `src-tauri/src/vault/persist.rs` | Lock-free save primitives: `SaveTarget`/`SaveJob`, `prepare_local/remote_open/create`, `persist_snapshot/save/change`, `read_keyfile`, attachment/CSV writers (extracted from mod.rs)                                                                                                                         |
| `src-tauri/src/vault/helpers.rs` | Free functions shared by the session: `parse_entry_id`/`parse_group_id`/`recycle_bin_id`, auto-type match walkers, OTP field resolution, `build_database_key`, `save_database`/`write_database_bytes`, KDF/cipher/compression application, secret wipe helpers (extracted from mod.rs)                          |
| `src-tauri/src/vault/tests.rs` | Session/entry/group/OTP/favicon/security/remote/autotype/RPC/bridge test suite (extracted from mod.rs)                                                                                                                                                                                                     |
| `src-tauri/src/vault/hosts.rs` | Host adapters over `VaultSession`: `BridgeHost` impl (association keys, logins, `db_hash`) + `RpcHost` impl (SRP session keys, RPC DTOs, `AddLogin`/`UpdateLogin` write path with history snapshots) + pure helpers (RPC/bridge tree building, URL matching, write-path field application; extracted from vault.rs) |
| `src-tauri/src/bridge/mod.rs` | KeePassHttp protocol core: request/response types, AES-256-CBC per-field crypto, verifier/HMAC, request dispatch (pure, no sockets)                                                                                                                                                                                 |
| `src-tauri/src/bridge/server.rs` | Loopback HTTP server (127.0.0.1:19455), `BridgeState` lifecycle, `ApprovalBoard`, HTTP framing, server tests                                                                                                                                                                                                        |
| `src-tauri/src/crypto.rs`          | Shared loopback crypto: AES-256-CBC + PKCS7 (raw/b64), HMAC-SHA256, SHA-1/SHA-256, hex/base64, CSPRNG, constant-time MAC compare (single source for bridge/rpc/vault)                                                                                                                                               |
| `src-tauri/src/rpc/mod.rs` | KeePassRPC protocol core: SRP-6a server math, key-auth challenge/response, AES-256-CBC + SHA-1 MAC frames, v1 JSON-RPC dispatch (pure, no sockets), write-path DTOs + `merge_urls` (KeePassRPC `MergeInNewURLs` semantics)                                                                                          |
| `src-tauri/src/rpc/server.rs` | Loopback WebSocket server (127.0.0.1:12546), `RpcState` lifecycle, per-connection handshake state machine, side-channel event emission, WS transport tests                                                                                                                                                          |
| `src-tauri/src/autotype.rs`        | KeePass-style auto-type sequence parser + `enigo` keystroke replay; `{REF:...}` field-reference expansion                                                                                                                                                                                                           |
| `src-tauri/src/focus.rs`           | Windows-only foreground-window title reader (Win32) for global auto-type matching; TCATO `WM_CHAR` channel injection                                                                                                                                                                                                |
| `src-tauri/src/remote/backup.rs` | Local mirror write + backup rotation for remote vaults: key validation/basename, backup-template expand/match, write + prune retention (pure std + chrono; extracted from vault.rs)                                                                                                                                 |
| `src-tauri/src/util.rs`            | Small cross-module helpers: URL host extraction (single source for bridge/rpc/vault URL matching), atomic tmp+rename file write                                                                                                                                                                                     |
| `src-tauri/src/vault/dto.rs`       | IPC serde DTOs (camelCase) shared with the frontend: entries/groups/state/inputs/patch/OTP/security/favicon shapes + tri-state icon deserializer (re-exported via `vault::*`)                                                                                                                                       |
| `src-tauri/src/vault/serialize.rs` | Snapshot building + entry mutation for the vault session: favicon host collection, `VaultState` tree/entry build, CSV/entropy/history helpers, `EntryInput`/`EntryPatch` field writes, custom-fields/attachment sync (extracted from vault.rs)                                                                      |

## Persistent layout

```text
<project>/
├─ conf/
│  └─ config.json          # AppConfig; remains beside the executable/project
└─ vaults/                 # user databases live where the user chooses (.kdbx)
```

The vault session is in-memory only. The database file path is user-selected; nothing is stored outside the chosen `.kdbx` file except the optional last-path setting.

## High-coupling files

Treat these as integration points and avoid concurrent edits:

- `src/routes/+page.svelte`
- `src/lib/components/SettingsDialog.svelte`
- `src/lib/types/settings.ts` + `src/lib/types/vault.ts`
- `src/lib/services/settings.ts` + `src/lib/services/vault.ts`
- `src-tauri/src/lib.rs`
- `src-tauri/src/config/` (mod + settings + store + tests)
- `src-tauri/src/vault/mod.rs`
- `src-tauri/src/bridge/mod.rs` + `src-tauri/src/bridge/server.rs` + `src-tauri/src/rpc/mod.rs` + `src-tauri/src/rpc/server.rs` + `src-tauri/src/vault/hosts.rs` (protocol ↔ server ↔ session boundaries)
- `TODO.md`
- `SKILL.md` and shared references

## Structure update rule

When adding, removing, renaming, or moving a route/module/component/service, update this map and the focused reference that describes its contract. Describe stable ownership; do not add transient implementation notes or raw file listings that will immediately go stale.
