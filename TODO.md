# KeyVault Roadmap

Status legend: `[ ]` pending · `[x]` delivered (with direct evidence) · `[~]` partial/blocked.

## Stage 1 — Project scaffold (delivered)

- [x] Svelte 5 + SvelteKit (adapter-static SPA) + Tauri 2 project skeleton
- [x] Rust backend crate (config + vault session module wiring)
- [x] Theme token system (20 semantic colors, dark/light presets, custom mapping)
- [x] Shared settings primitives (`settings-shared.css`)
- [x] Settings shell + General/Security/Database/About panels
- [x] Welcome/unlock flow with open + create database modals
- [x] Three-pane main layout: group tree / entry list / detail
- [x] Entry editor (create/edit/delete), group create/rename/delete
- [x] Search across title/username/url/notes, group filter
- [x] Password generator with entropy readout; copy with scheduled clipboard clear
- [x] Skill + references + repository documentation

## Stage 2 — Backend vault engine

- [x] `open_vault` / `create_vault` / `save_vault` / `close_vault` via `keepass` crate (KDBX 4.0)
- [x] Entry and group CRUD commands with in-memory session
- [x] Rust tests: round-trip save, CRUD, wrong-password rejection, session clear
- [x] Wire `src/lib/services/vault.ts` to real commands behind `isTauriRuntime()`

## Stage 3 — Lock & clipboard security (delivered)

- [x] Idle auto-lock timer driven by `autoLockMinutes` (`armIdleLock`/`installAutoLock`, `src/lib/services/security.ts`)
- [x] Lock clears clipboard when `clearOnLock` (`lockVault`)
- [x] `lockAfterAction` after password copy (`copySensitive`, wired for password copies only)
- [x] Frontend lock screen + reopen with remembered path only (`LockScreen.svelte`, `vault.remembered`)

## Stage 4 — Search & productivity

- [x] TOTP display with countdown for `otp`/`totp` fields (`totp_code` command + `TotpWidget`)
- [ ] Password strength meter in entry editor
- [x] URL quick-open via `@tauri-apps/plugin-opener` (detail + list rows)
- [ ] Autotype sequence runner (roadmap reserved)
- [x] Favorite/pin entries with `--warning-color` accent (`toggle_favorite` + `KeyVault.Favorite` field)

## Stage 5 — Packaging & release

- [ ] App icons, window branding, NSIS installer template
- [ ] GitHub Actions CI mirroring `npm run verify`
- [ ] Release workflow via version-release skill (like sibling projects)
