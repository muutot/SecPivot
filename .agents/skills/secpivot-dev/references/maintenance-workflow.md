# Maintenance Workflow

Credential fixtures follow the local-open triple `(Database, PreparedKeyfile,
[u8;32])` and complete_save_as's PersistOutcome argument. Their observed owned
credential payloads and production bodies stay intact; baseline hashes/pending
reload fields are nonsecret stubs. Reload controller/service and mounted activity
tests use synthetic IPC/flags only. Native reload tests use disposable KDBX,
MemoryStorage and a real abandoned Tokio worker permit, without user files,
provider or shared clipboard access. Browser draft checks are isolated previews,
not desktop IPC evidence; do not close platform/interop TODOs from these checks.

Read this before any repository change. It is the operating contract for every edit, audit, and commit.

## Workspace discipline

- Work in `B:\Program\Project\SecPivot` unless told otherwise. The sibling projects (`clipboard`, `MuuPass`) are read-only references for style and conventions; never edit them during SecPivot work.
- Treat `clipboard` as the style source of truth for the approved visual language. `MuuPass` is the reference for KeePass domain behavior (TOTP, KDF choices, auto-type). Borrow patterns, do not copy implementation wholesale.
- Preserve unrelated user changes. Inspect `git status --short --branch` before starting and re-inspect before committing.
- Generated npm/pnpm lockfiles are excluded from Prettier; preserve locally
  generated lockfiles instead of reformatting them as part of a UI fix.
- OpenIdea's local `.openidea/` metadata stays out of Git and Prettier. If local editor metadata appears during a check, preserve its contents and verify the ignore rule rather than formatting or deleting the user's files.

## Task loop

1. Read `SKILL.md` and this workflow.
2. Inspect status, `git log --oneline -10`, `TODO.md`, and the exact files in scope.
3. Read the routing-table references fully before editing a subsystem.
4. Make one minimal change with an independent verification path.
5. Verify with the narrowest relevant command, then the gate for the change's scope.
6. Run `npm run format:check`; a formatting warning is a blocker and any reformatting must be staged.
7. Run the documentation currency gate.
8. Commit one unit with the gitmoji message format; recheck status.

## TODO audit rule

A TODO item is checked only when direct implementation evidence and proportionate verification cover its **full wording**. Evidence is code that executes the described behavior plus a passing check that exercises it. A stub, an unintegrated module, a UI shell, a similar symbol, or a compiled-but-unwired platform adapter is not completion.

When a TODO cannot be verified in the current environment, leave it unchecked and record the missing evidence in the commit report.

## Verification mapping

| Change scope           | Minimum check                                                              | Full gate           |
| ---------------------- | -------------------------------------------------------------------------- | ------------------- |
| Frontend-only          | relevant `npm run test:frontend` tests + `npm run check` + `npm run build` | `npm run verify`    |
| Backend-only           | `cargo check` / `cargo test` for the module                                | `npm run verify`    |
| Cross frontend/backend | contract-adjacent tests + `npm run check` + `cargo check`                  | `npm run verify`    |
| Visual / theme         | structural comparison + rendered dark/light/narrow check                   | add `npm run build` |
| Settings markup / CSS  | `npm run check` + style gate + narrow-window check                         | add `npm run build` |
| Security behavior      | targeted tests for lock/clear/session semantics                            | full backend gate   |

`npm run verify` runs format checks, svelte-check, vite build, Node frontend behavior tests (`tests/*.test.mjs`), Rust tests, and clippy. If an environment prevents a required runtime/visual/platform check, report the missing evidence and keep the corresponding TODO unverified.

It also runs the unchanged tldts7.4.2 JS/MIT-license hash gate
(`verify:tldts`); `node scripts/verify-tldts.mjs --staged` checks staged vendor
bytes before committing. Vendor files are exempt from formatting/EOL changes.
It runs the KeePass and QuickXML patch provenance gates: 60/43 package
hashes, nine/one explicitly patched Rust files and Cargo wiring. Read
`src-tauri/vendor/KEEPASS_PATCH.md` and `QUICK_XML_PATCH.md` before patch edits.
Preserve untouched upstream bytes/licenses and update only reviewed patched
hashes after verification; do not blanket-format either published package.
When formatting an owned module entry point, pass rustfmt `--config skip_children=true`
to avoid rewriting untouched upstream child modules; compare all package hashes.
QuickXML's package and byte-preserving patch are exempt from Git text/EOL
normalization because its upstream files use CRLF. Check staged byte hashes,
not only working files, before committing a package/provenance change.

`npm run test:keepass-patch` and `npm run test:quickxml-patch` run the 36 KeePass
specialized tests and QuickXML's serialization-enabled library unit tests in
disposable source copies, preserving the published standalone Cargo locks.
Both run in `npm run verify`. The helper validates its created temporary root
before recursive cleanup and uses the repository target cache; source copies
contain no user vaults. Upstream ignored tests do not count as executed coverage.

`npm run test:vault-credentials` executes twenty-five synthetic credential-control-flow
tests in a disposable Cargo fixture, with a private copy of the repository lock
and offline dependencies. It extracts the four current open/create handler
bodies plus change_master_key; only the password parameter is replaced by an
observed payload. The actual guarded-password implementation is included unchanged. Tauri state,
configuration, persistence and adoption remain stubs; real Tokio workers/abort
and JSON string parsing execute. The master-change fixture observes successfully
read new keyfile bytes too; completion follows the current owned/borrowed API
and keeps adopted values until simulated close. These checks prove temporary
owned-command cleanup paths,
not native IPC, provider/KDBX behavior or complete process-memory erasure.
The ordinary full gate runs current sources. `--source-ref <commit>` is an
explicit read-only audit comparison, never the ordinary gate's source.

The stored-keyfile fixture also extracts five session methods and the actual
wipe helpers. It binds Vec to a live-owned observed buffer without changing
the method bodies; database/storage/UUID/snapshot/outcome boundaries remain
stubs. Four checks cover25 replacement/close paths and25 old/new buffers. This
proves omitted stored-value clearing and adoption controls, not raw-memory
disclosure or native IPC/provider behavior.

The new-keyfile fixture extracts four complete preparation bodies; only the
local return payload type is replaced by an observed live-owned buffer. Its
read/KDF/parser/transport/mirror boundaries remain stubs. Local creation's new
settings argument and triple outcome use nonsecret config/backup/outcome stubs;
save failure and credential/adoption cleanup remain observed. The command fixture
also supplies observed preparation outputs and extracts actual registry open,
including its current backup-preference propagation.
The stub session has explicit close and no implicit Drop; registry poison is injected
after adoption. Registered values remain valid until simulated close, while fresh
values are closed by the production Unregistered guard on refusal. Six added checks
cover62 paths/52 buffers with real Tokio panic/worker/abort. Type aliases and
guarded result shapes follow current source; ordinary verification never uses
source-ref. These checks are ownership evidence, not actual IPC/provider or
whole-process memory-erasure proof.

The persistence-job fixture extracts three job declarations, five complete
session factories and four complete consumers. Credential payload types and wipe
helper parameter types are observed; production bodies remain unchanged. Five
added groups cover80 paths/120 owned values, including ordinary Err controls,
direct job drop, panic, real Tokio abort before worker start and abort after start.
Database/KDF/parser/storage/settings are stubs. The injected KDF panic proves an
unwind path, not a native input trigger. Observers record while still owning the
value, then wipe synthetic test payloads without altering the recorded result.
The fixture uses one queued blocker and tiny values; it never reads released memory,
user vaults, native providers or the shared clipboard. Source-ref remains an
explicit audit comparison; ordinary verification always extracts current sources.

## Browser vs desktop truth

Native logging is scoped by lib.rs's lexical eprintln macro before all modules.
Keep new diagnostics inside that gate; do not bypass it with std::eprintln,
print/println/dbg or a separate first-party logger. Record static templates only;
dynamic payloads are never formatted. Typed config parse categories/offsets have
their dedicated safe API. OS/vendor/framework logs remain outside this gate.
Tests that intentionally capture process output use synthetic data only.

`npm run test:android-tls` builds a disposable host Cargo fixture using locked
reqwest/rustls/webpki versions and actual net.rs bodies. Only Android cfg becomes
a fixture feature; no networking API is stubbed. Public DER fixtures verify the
removed emSign ECC G3 root and all eight distrusted SPKIs. Both real async and
blocking clients reject the synthetic localhost CA during TLS, while explicit
fixture-only trust succeeds. Tests bypass proxies and never contact a provider,
user vault, device or shared clipboard. The copied lock and synthetic private
TLS key remain confined to the disposable test; the checked-in key is expressly
non-production. This is host execution of Android configuration, not an Android
target/APK/OS verifier or real provider result.

`npm run test:clipboard-write` runs fourteen independent text/binary write checks
in a disposable offline Cargo project with the repository's pinned zeroize.
It extracts the complete platform clipboard module, replacing only the Win32
import root and platform cfg selector with a synthetic API. Real byte copies and
zeroize execute on still-owned aligned storage at most128 bytes; the fake API
records clearing before releasing that storage. No OS clipboard or released
memory is accessed. The ordinary gate uses current source; explicit source-ref
is a read-only comparison. A separate native Rust test only creates/destroys the
invisible message-only owner window. Neither check proves live clipboard/client
interoperability or external identical-copy ownership. Temporary cleanup verifies
the resolved created directory stays inside the system temp root before deletion.

- Audit evidence paths become immutable once their hashes are recorded. Use
  separate red/green artifact names and archive or remove temporary emitters
  before running ordinary tests again. Verify earlier hashes before sealing the
  next stage. If an artifact was overwritten, record its availability gap and
  retain the original hash; never regenerate a sample and label it original.

- Every test that accesses the actual OS clipboard must check
  `SECPIVOT_CLIPBOARD_LIVE=1` before any native read, write or usability probe.
  A crate mutex does not isolate the user's clipboard from other processes.
  Default verification leaves that opt-in unset; a returned skip is not evidence
  of native clipboard interoperability. Live checks need an explicitly selected,
  disposable environment rather than the developer's shared clipboard.

- Windows accepted sockets inherit a nonblocking listener's mode. A blocking
  HTTP fixture must explicitly call `set_nonblocking(false)` before reading;
  setting a read timeout alone still permits immediate `WouldBlock`. Verify
  this with delayed request bytes, not only clients that send immediately.

- Offline S3/WebDAV HTTP tests disable environment/system proxies on their
  shared client. Otherwise a Windows system proxy can relay loopback fixtures
  and return HTTP 502 despite the local peer always replying 200. This is a
  test-only setting; production respects the user's proxy configuration.

- Machine-readable audit metadata under `.claude/audits/*.json` and shared test
  fixtures under `tests/fixtures/*.json` stay strict JSON.
  The Prettier override disables trailing commas there; verify it with a JSON
  parser because the repository's general `jsonc` formatting accepts them.

- The browser (`npm run dev` outside Tauri) runs a demo-vault fallback stored in `localStorage`. It is a UI-development surface only.
- Desktop KDBX read/write behavior is defined only by the Rust backend (`keepass` crate) and its tests. Never check a desktop TODO from browser-only evidence.
- When verifying a Tauri contract, keep command names, Rust argument/result types, frontend `invoke` arguments, serde casing, and error strings aligned.

## Documentation currency gate

Before every commit, inspect the staged diff and update `SKILL.md`/`references/*` if the change affects routes, components, services, backend modules, IPC/serialization, config defaults, security invariants, theme tokens, settings primitives, or any approved pattern or stable rule. State the outcome explicitly in the commit report ("docs: no update needed" or what changed).

## Version bump and release

Use the sibling `version-release` skill (`.agents/skills/version-release/SKILL.md`) when bumping the version, releasing, or regenerating a release. It drives `scripts/release.mjs` through a two-pass flow (script bumps + changelog, LLM curates `RELEASE.md`, script commits/tags/pushes). Before any release, run `npm run verify` and commit all non-release changes separately so the release commit contains only version files.

## Packaging verification

`npm run tauri -- build` produces the NSIS installer (`src-tauri/target/release/bundle/nsis/SecPivot_<version>_x64-setup.exe`) and exercises the custom `src-tauri/windows/installer.nsi` template. GitHub Actions workflows (`.github/workflows/*`) cannot execute locally; inspect the real run/job conclusions plus draft-release assets before changing TODO status. A locally valid YAML diff or a partially successful job is not release evidence: CI remains partial until the current HEAD passes, and release remains partial until every required Windows/portable/Android artifact exists and its job succeeded.
