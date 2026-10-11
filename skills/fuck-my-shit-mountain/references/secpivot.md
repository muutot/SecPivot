# SecPivot audit profile

Read this profile when auditing SecPivot. Paths below are relative to the repository root; verify them against the current tree. If this skill is packaged for another project, use that project's instructions instead.

## Repository workflow

Read `AGENTS.md`, `skills/secpivot-dev/SKILL.md` and its maintenance workflow before repository changes. Route focused inspection through the development skill's references; do not load all subsystem references for every audit.

- Inspect Git status and preserve unrelated work, including nested worktrees.
- TODO completion requires direct implementation evidence plus proportionate verification. An audit report alone does not authorize checking roadmap items.
- Preserve the approved compact visual language. For requested UI remediation, read `css-theming.md` in the dev skill; settings changes also require `settings-panels.md` and the theme-token gate.
- For authorized fixes, use one verified minimal commit at a time, inspect documentation currency, and use `<emoji> <type>[<scope>]: <English imperative message>`.
- Never echo, log, or export vault passwords, master keys, or decrypted entry secrets in commands, logs, reports, or test fixtures. Redact credentials in evidence.

## Risk-driven source map

| Concern                | Starting points and checks                                                                                                                                                               |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UI and state           | `src/routes/`, `src/lib/`: initialization gates, out-of-order invoke results, subscription cleanup, focus management, compact-layout regressions                                         |
| IPC and local API      | `src-tauri/src/lib.rs`, `commands/`: registered commands, invoke names, argument/result casing, validation before effects, error surfaces                                                |
| Vault and persistence  | `src-tauri/src/`: KDBX open/create/save flows, lock/close boundaries, in-memory session lifetime, zeroization, migrations and recovery expectations                                      |
| Security and privacy   | password generation, clipboard clearing (`clipboardClearSeconds`), auto-lock, config/logging paths: no password or master key outside the in-memory session                              |
| Settings and contracts | `data-contracts.md`, `security-model.md` in the dev skill: TS type ↔ Rust serde lockstep, `#[serde(default)]` round-trips, normalization/ranges, cross-window application                |
| Release                | `.github/workflows/`, lockfiles, and `skills/version-release/SKILL.md`: tag bound to the release commit only, six-file release commit rule, pinned actions, extreme profile Actions-only |

Documentation is a navigation aid. Re-check a historical finding against current code before reporting it again.

## Verification limits

Use configured commands and record their actual results:

- Frontend: `npm run check`, `npm run test:frontend`.
- Rust: focused Cargo tests, `npm run test:rust`, `npm run lint:rust`.
- Before a fix commit: `npm run format:check` and the applicable maintenance gates.
- Release readiness: `npm run format:check`, `npm run check`, `npm run lint:rust`. Release building occurs in GitHub Actions on tag push. Never run or locally enable the extreme-release profile overrides.

Windows checks do not compile other-platform `cfg` branches. Inspect matching CI or runtime evidence before claiming those platforms verified. Browser preview and static checks do not prove desktop KDBX behavior, clipboard clearing, auto-lock, or visual correctness. Record unavailable evidence as a limit, not an automatic defect or a passing result.

Audit tests use synthetic vaults and temporary directories. Never run migration, cleanup, or vault-mutating experiments against the user's real `.kdbx` files or `config.json`.
