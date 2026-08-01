# Project Structure and Runtime Surfaces

This reference is a source map, not a substitute for reading the current files. Update this map when ownership or entry points change.

## Runtime surfaces

| Surface             | Entry point                                                                      | Responsibility                                                                                       |
| ------------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Main desktop window | `src/routes/+page.svelte`                                                        | Welcome/unlock, group tree, entry list, detail, search, editor dialogs, status                       |
| Settings window     | `src/routes/settings/+page.svelte` → `SettingsDialog.svelte`                     | Standalone settings page, theme/font application, settings navigation/panels                         |
| Frontend services   | `src/lib/services/*`                                                             | Settings store + bootstrap, vault IPC wrapper + browser fallback                                     |
| GUI backend         | `src-tauri/src/lib.rs`                                                           | Tauri setup, managed config + vault session, commands                                                |
| Backend modules     | `src-tauri/src/config.rs`, `src-tauri/src/vault.rs`, `src-tauri/src/autotype.rs` | Config persistence; KeePass open/create/edit/save session; auto-type sequence parsing + enigo replay |
| Release automation  | `scripts/version.mjs`, `changelog.mjs`, `release.mjs`                            | Atomic version bump, gitmoji changelog, two-pass release orchestration                               |
| CI / CD             | `.github/workflows/ci.yml`, `release.yml`                                        | `npm run verify` on push/PR; tagged multi-platform build + draft release                             |
| Windows installer   | `src-tauri/windows/installer.nsi`                                                | Custom NSIS template wired via `bundle.windows.nsis.template`                                        |

SvelteKit runs as a static SPA: `src/routes/+layout.ts` disables SSR and awaits `appSettings.initialize()` before route load. `+layout.svelte` imports global CSS and applies settings to the document.

## Frontend ownership

| Path                                       | Ownership                                                                                            |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| `src/app.css`                              | Global reset, theme defaults, font variables, accessibility media rules; imports shared settings CSS |
| `src/lib/styles/settings-shared.css`       | Reusable settings-panel primitives (header/cards/toggles/slider/feedback)                            |
| `src/lib/types/theme.ts`                   | `ThemeColors`, `DARK_THEME_COLORS`, `LIGHT_THEME_COLORS`                                             |
| `src/lib/types/settings.ts`                | `AppSettings`, `GeneralSettings`, `SecuritySettings`, `DatabaseDefaults`                             |
| `src/lib/types/vault.ts`                   | `VaultState`, `VaultGroup`, `VaultEntry`, request/input shapes                                       |
| `src/lib/utils/theme.ts`                   | `ThemeColors` → CSS-variable mapping                                                                 |
| `src/lib/utils/password.ts`                | Password generator, entropy estimate, strength label                                                 |
| `src/lib/utils/totp.ts`                    | Browser-fallback TOTP (RFC 6238, WebCrypto); desktop uses backend `totp_code`                        |
| `src/lib/utils/clipboard.ts`               | Clipboard copy + scheduled clear (`clipboardClearSeconds`)                                           |
| `src/lib/services/settings.ts`             | `appSettings` store, defaults, normalization, debounced persistence                                  |
| `src/lib/services/settings-bootstrap.ts`   | Apply settings to document (theme colors, font vars, window effect)                                  |
| `src/lib/services/security.ts`             | `lockVault`/`copySensitive`/`armIdleLock`/`installAutoLock` (lock lifecycle, clipboard-clear gating) |
| `src/lib/services/vault.ts`                | `vault` store: open/create/close/save + entry/group CRUD + `remembered` path (Tauri + browser)       |
| `src/lib/data/demo-vault.ts`               | Browser-preview fallback data; not proof of desktop KDBX behavior                                    |
| `src/lib/components/AppIcon.svelte`        | Hand-written inline SVG icon set (stroke `currentColor`)                                             |
| `src/lib/components/ContextMenu.svelte`    | Viewport-fixed right-click menu (items, destructive style, click-outside/Escape close)               |
| `src/lib/components/SettingsDialog.svelte` | Settings shell: sidebar nav + content pane + panels                                                  |
| `src/lib/components/settings/*`            | General / Security / Database / About panels                                                         |
| `src/lib/components/VaultWelcome.svelte`   | Welcome/unlock + open/create database modal flows                                                    |
| `src/lib/components/LockScreen.svelte`     | Lock screen: reopen remembered path with password, or switch to another database                     |
| `src/lib/components/TotpWidget.svelte`     | TOTP code readout with countdown bar; refetches per period                                           |

## Backend ownership

| Path                        | Ownership                                                                                                                        |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/lib.rs`      | Tauri builder, managed state (`AppState`), command registration, setup, global auto-type hotkey (register/re-register + handler), TCATO overlay window + commands |
| `src-tauri/src/config.rs`   | `config.json` schema, defaults, normalization, atomic persistence                                                                |
| `src-tauri/src/vault.rs`    | KeePass session: open/create/close/get_state/save, entry & group CRUD, serialization, auto-type match scoring                    |
| `src-tauri/src/autotype.rs` | KeePass-style auto-type sequence parser + `enigo` keystroke replay; `{REF:...}` field-reference expansion |
| `src-tauri/src/focus.rs`    | Windows-only foreground-window title reader (Win32) for global auto-type matching; TCATO `WM_CHAR` channel injection |

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
- `src-tauri/src/config.rs`
- `src-tauri/src/vault.rs`
- `TODO.md`
- `SKILL.md` and shared references

## Structure update rule

When adding, removing, renaming, or moving a route/module/component/service, update this map and the focused reference that describes its contract. Describe stable ownership; do not add transient implementation notes or raw file listings that will immediately go stale.
