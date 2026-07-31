# Maintenance Workflow

Read this before any repository change. It is the operating contract for every edit, audit, and commit.

## Workspace discipline

- Work in `B:\Program\Project\KeyVault` unless told otherwise. The sibling projects (`clipboard`, `MuuPass`) are read-only references for style and conventions; never edit them during KeyVault work.
- Treat `clipboard` as the style source of truth for the approved visual language. `MuuPass` is the reference for KeePass domain behavior (TOTP, KDF choices, auto-type). Borrow patterns, do not copy implementation wholesale.
- Preserve unrelated user changes. Inspect `git status --short --branch` before starting and re-inspect before committing.

## Task loop

1. Read `SKILL.md` and this workflow.
2. Inspect status, `git log --oneline -10`, `TODO.md`, and the exact files in scope.
3. Read the routing-table references fully before editing a subsystem.
4. Make one minimal change with an independent verification path.
5. Verify with the narrowest relevant command, then the gate for the change's scope.
6. Run the documentation currency gate.
7. Commit one unit with the gitmoji message format; recheck status.

## TODO audit rule

A TODO item is checked only when direct implementation evidence and proportionate verification cover its **full wording**. Evidence is code that executes the described behavior plus a passing check that exercises it. A stub, an unintegrated module, a UI shell, a similar symbol, or a compiled-but-unwired platform adapter is not completion.

When a TODO cannot be verified in the current environment, leave it unchecked and record the missing evidence in the commit report.

## Verification mapping

| Change scope           | Minimum check                                             | Full gate           |
| ---------------------- | --------------------------------------------------------- | ------------------- |
| Frontend-only          | `npm run check` + `npm run build`                         | `npm run verify`    |
| Backend-only           | `cargo check` / `cargo test` for the module               | `npm run verify`    |
| Cross frontend/backend | contract-adjacent tests + `npm run check` + `cargo check` | `npm run verify`    |
| Visual / theme         | structural comparison + rendered dark/light/narrow check  | add `npm run build` |
| Settings markup / CSS  | `npm run check` + style gate + narrow-window check        | add `npm run build` |
| Security behavior      | targeted tests for lock/clear/session semantics           | full backend gate   |

`npm run verify` runs format checks, svelte-check, vite build, Rust tests, and clippy. If an environment prevents a required runtime/visual/platform check, report the missing evidence and keep the corresponding TODO unverified.

## Browser vs desktop truth

- The browser (`npm run dev` outside Tauri) runs a demo-vault fallback stored in `localStorage`. It is a UI-development surface only.
- Desktop KDBX read/write behavior is defined only by the Rust backend (`keepass` crate) and its tests. Never check a desktop TODO from browser-only evidence.
- When verifying a Tauri contract, keep command names, Rust argument/result types, frontend `invoke` arguments, serde casing, and error strings aligned.

## Documentation currency gate

Before every commit, inspect the staged diff and update `SKILL.md`/`references/*` if the change affects routes, components, services, backend modules, IPC/serialization, config defaults, security invariants, theme tokens, settings primitives, or any approved pattern or stable rule. State the outcome explicitly in the commit report ("docs: no update needed" or what changed).

## Version bump and release

Use the sibling `version-release` skill (`.opencode/skills/version-release/SKILL.md`) when bumping the version, releasing, or regenerating a release. It drives `scripts/release.mjs` through a two-pass flow (script bumps + changelog, LLM curates `RELEASE.md`, script commits/tags/pushes). Before any release, run `npm run verify` and commit all non-release changes separately so the release commit contains only version files.

## Packaging verification

`npm run tauri -- build` produces the NSIS installer (`src-tauri/target/release/bundle/nsis/KeyVault_<version>_x64-setup.exe`) and exercises the custom `src-tauri/windows/installer.nsi` template. GitHub Actions workflows (`.github/workflows/*`) cannot execute locally; they require a configured `origin` remote and GitHub to provide runtime evidence, so their TODO items stay unchecked until a real CI run passes.
