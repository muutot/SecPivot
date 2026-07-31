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

- [ ] `open_vault` / `create_vault` / `save_vault` / `close_vault` via `keepass` crate (KDBX 4.0)
- [ ] Entry and group CRUD commands with in-memory session
- [ ] Rust tests: round-trip save, CRUD, wrong-password rejection, session clear
- [ ] Wire `src/lib/services/vault.ts` to real commands behind `isTauriRuntime()`

## Stage 3 — Lock & clipboard security

- [ ] Idle auto-lock timer driven by `autoLockMinutes`
- [ ] Lock clears clipboard when `clearOnLock`
- [ ] `lockAfterAction` after password copy
- [ ] Frontend lock screen + reopen with remembered path only

## Stage 4 — Search & productivity

- [ ] TOTP display with countdown for `otp`/`totp` fields
- [ ] Password strength meter in entry editor
- [ ] URL quick-open, autotype sequence runner (roadmap reserved)
- [ ] Favorite/pin entries with `--warning-color` accent

## Stage 5 — Packaging & release

- [ ] App icons, window branding, NSIS installer template
- [ ] GitHub Actions CI mirroring `npm run verify`
- [ ] Release workflow via version-release skill (like sibling projects)
