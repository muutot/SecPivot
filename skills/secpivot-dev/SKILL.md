# Skill: secpivot-dev

# SecPivot Desktop Development

SecPivot is a professional, compact, high-information-density, minimalist modern KeePass client built with Svelte 5 + Tauri 2 + Rust. Its visual language and settings architecture are derived from the Clipboard Desktop project (see `css-theming.md`).

## Start every task

1. Read [references/maintenance-workflow.md](references/maintenance-workflow.md) completely before changing the repository.
2. Inspect `git status --short --branch`, recent commits, `TODO.md`, and the exact source and tests in scope. Preserve unrelated user changes.
3. Select the relevant references from the routing table below and read each selected file completely before editing its subsystem.
4. Treat current source, tests, configuration, and rendered/runtime behavior as authoritative. References are navigation aids and must be corrected when evidence disagrees.
5. Define one independently verifiable feature, fix, audit, style pass, or documentation update for the next commit.

## Non-negotiable principles

- Check a TODO only when direct implementation evidence and proportionate verification cover its full wording. A stub, setting, UI shell, similarly named symbol, or untested platform file is not completion.
- Preserve the approved compact visual language. Do not redesign it unless the task explicitly requests a redesign.
- Apply the theme-token gate before any CSS or markup change: reuse the semantic theme variables and shared settings primitives; never introduce parallel spellings of an existing token.
- Protect local vault data first. Be conservative around migrations, cleanup, lock/close boundaries, clipboard clearing, and password handling.
- Keep parallel ownership non-overlapping. The primary agent owns shared integration files and final verification.
- Commit one verified minimal unit at a time; never mix unrelated cleanup into the same commit.
- Never log, persist, or transmit passwords or vault master keys beyond the in-memory session; zeroize on lock.

## Mandatory documentation currency gate — every commit

Before **every** commit, inspect the staged diff and decide whether `SKILL.md` or any file under `references/` must change. This gate is required even when the answer is “no documentation update needed.”

Update the matching reference in the same commit when the change affects any of:

- routes, components, services, utilities, backend modules, or ownership boundaries;
- Tauri commands/events, TypeScript/Rust types, database/config schemas, defaults, or serialization;
- lock lifecycle, clipboard-clearing, vault-session, or security invariants;
- theme variables, project-wide styles, settings primitives, layout hierarchy, or an approved pattern;
- a recurring pitfall, workflow rule, or stable project preference.

Keep stable workflow and routing rules in `SKILL.md`. Put module-specific facts, signatures, examples, and style details in the relevant reference. Do not edit documentation merely to create churn, but never commit a known-stale skill or reference.

## Reference router

| Task                                                              | Read before editing                                                                                     |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Any repository change, TODO audit, verification, or commit        | [maintenance-workflow.md](references/maintenance-workflow.md)                                           |
| Repository orientation, routes, directories, runtime surfaces     | [project-structure.md](references/project-structure.md)                                                 |
| Any UI or CSS change                                              | [css-theming.md](references/css-theming.md)                                                             |
| Settings shell, panel markup, controls, feedback, or settings CSS | [settings-panels.md](references/settings-panels.md) and [css-theming.md](references/css-theming.md)     |
| Vault open/create/edit/save flows, IPC, or backend session        | [data-contracts.md](references/data-contracts.md) and [security-model.md](references/security-model.md) |
| Config defaults, normalization, persistence                       | [data-contracts.md](references/data-contracts.md)                                                       |
| Password generation, entropy, clipboard clearing, auto-lock       | [security-model.md](references/security-model.md)                                                       |
| Version bump, release, or regenerate                              | `../version-release/SKILL.md` (sibling skill)                                                           |

Also read `docs/PITFALLS.md` when relevant.

## Cross-cutting change gates

- **Tauri contract:** keep command name, Rust arguments/result, frontend `invoke`, serde casing, error handling, and tests aligned.
- **Settings contract:** update TypeScript type, defaults, normalization/ranges, Rust config serde/defaults, UI, persistence, cross-window application, and references as applicable. Rust serde drops unknown fields silently — a field missing from the Rust config struct survives the session but resets on restart; keep both sides in lockstep and cover the round-trip in a test. Settings structs use container-level `#[serde(default)]` so older `config.json` files load without crashing (`ConfigStore::load` errors would panic the setup hook); keep `Default` impls current and add a load test for each new field (see `data-contracts.md`).
- **Vault contract:** update frontend `VaultState`/`VaultEntry`/`VaultGroup` mapping, Rust domain serialization, repository behavior, session handling, save/lock semantics, and tests.
- **Security contract:** password fields never enter the log or config; clipboard clearing follows `clipboardClearSeconds`; lock clears in-memory session; the demo browser fallback is explicitly not proof of desktop persistence or KDBX behavior.
- **Visual contract:** type/build checks do not prove appearance. Use structural comparison and, when available, rendered/runtime inspection at the target window size and theme.

## Verification commands

```powershell
npm run check
npm run build
npm run test:rust
npm run lint:rust
npm run format:check
npm run verify
```

Run the narrowest relevant checks during implementation. Run `npm run verify` at integration milestones or before a commit whose scope crosses frontend and backend. If an environment prevents a required runtime, visual, platform, packaging, or performance check, report the missing evidence and leave the corresponding TODO unverified.

The pre-release gate additionally runs the extreme-release build (see `skills/version-release/SKILL.md`); the local build profile itself stays fast.

## Local build and run model

Build profiles in `src-tauri/Cargo.toml` are split so local builds are fast and storage-light, and only the GitHub Actions release build applies extreme runtime optimization:

- `npm run tauri dev` — local dev run. `[profile.dev]` keeps dependencies unoptimized (opt-level 0, no debug symbols) with `incremental` + `split-debuginfo = "unpacked"`, so the cached target is reused and rebuilds are as fast as possible.
- `npm run tauri build` — local packaging. `[profile.release]` is likewise tuned for build speed and minimal cache (opt-level 0, `codegen-units = 256`, no debug info, `incremental = false`).
- GitHub Actions release — the only place extreme runtime optimization is enabled. `.github/workflows/release.yml` sets `CARGO_PROFILE_RELEASE_LTO=true`, `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1`, `CARGO_PROFILE_RELEASE_OPT_LEVEL=3` plus `RUSTFLAGS` (`target-cpu=x86-64-v3`). Local builds are unaffected.
- CI checks (`ci.yml`, release `verify`) build with `--profile ci` (deps at opt-level 0) and use `sccache` and `cargo-nextest` for tests.

## Commit message format

Follow the gitmoji convention established by the Clipboard repository and shared across sibling projects:

```
<gitmoji> <type>[<scope>]: <message>
```

- **gitmoji**: single emoji indicating the change category.
- **type**: lowercase change type matching the emoji.
- **scope**: optional, lowercase, in **square brackets**.
- **message**: concise imperative description, Chinese or English.

### Gitmoji mapping

| Emoji | Type     | Use when                                             |
| ----- | -------- | ---------------------------------------------------- |
| ✨    | feat     | new feature or capability                            |
| 🐛    | fix      | bug fix or correction                                |
| 📝    | docs     | documentation, roadmap, skill, or reference update   |
| ♻️    | refactor | code restructuring without behavior change           |
| 🎨    | style    | formatting, CSS, visual polish                       |
| 🚀    | perf     | performance improvement                              |
| ✅    | test     | adding or updating tests                             |
| 🔧    | chore    | tooling, dependencies, build scripts, CI             |
| 🎉    | chore    | initial commit / project bootstrap                   |
| 🗃️    | feat     | database / schema changes                            |
| 🔒    | feat     | privacy, security, permissions, defaults enforcement |
| 🔌    | feat     | exposing commands / connecting frontend to backend   |
| 🔄    | feat     | synchronization, reload, outbox                      |
| ⚙️    | feat     | configuration / settings                             |
| 💾    | feat     | storage / persistence                                |
| 📦    | feat     | packaging / dependencies                             |
| 💬    | fix      | messaging / empty-state / user-facing text           |
| 📁    | feat     | file / directory organization                        |

### Examples

```
🎉 chore: scaffold SecPivot Svelte 5 + Tauri 2 project
📝 docs[skill]: document theme-token gate and settings primitives
✨ feat[vault]: add open_vault/create_vault backend session
⚙️ feat[settings]: add general/security/database settings panels
🎨 style[settings]: align custom theme color grid spacing
```

## Commit discipline

1. Review `git diff` and `git diff --cached`; stage only the intended unit.
2. Run the relevant verification and the mandatory documentation currency gate.
3. Update matching TODO evidence in the same commit only when completion is directly proven.
4. Write the commit message in the gitmoji format described above.
5. Recheck `git status` after committing and report verification plus any evidence gaps.

Base directory for this skill: B:\Program\Project\KeyVault\skills\secpivot-dev
Relative paths in this skill (e.g., scripts/, reference/) are relative to this base directory.
