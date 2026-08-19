---
name: version-release
description: Release workflow for SecPivot. Bump, release, or regenerate versions via scripts/release.mjs with the two-pass RELEASE.md flow, semantic versioning, and CI/CD release triggers.
---

# Skill: version-release

# Version Release

## Trigger patterns

Start this skill when the user says any of:

- "升级版本到 x.x.x" / "bump version to x.x.x"
- "发布版本 x.x.x" / "release version x.x.x"
- "重新发布版本 x.x.x" / "regenerate release x.x.x"
- "升级 patch/minor/major 版本"
- "release" combined with a version number or bump type

## Release workflow (single script, two passes for RELEASE.md)

### Prerequisite: only version files in the release commit

Before running the release script, ensure that **every other change** has already been committed separately.

The release commit (`🔖 chore[release]: bump version to x.x.x`) **must only contain**:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `CHANGELOG.md`
- `RELEASE.md`

Any change to scripts, skills, references, tests, or other source files **must be committed before** the release. The release script's `git diff --name-only` may pick up unrelated dirty files — verify the staged diff before allowing the commit.

The script enforces this prerequisite before changing versions and again before committing: unrelated tracked or staged files abort the release, and the commit stages only the six files above. Untracked local tooling directories are not staged and do not broaden the release commit.

Run:

```
node scripts/release.mjs <version>
```

The script does the following:

| Step | What                                                                 |
| ---- | -------------------------------------------------------------------- |
| 1    | Bump version in `package.json`, `tauri.conf.json`, `Cargo.toml`      |
| 2    | Generate `CHANGELOG.md` from commits since last tag                  |
| 3    | Check `RELEASE.md` — if stale, prints instructions and exits cleanly |
| 4    | Commit version files + CHANGELOG.md + RELEASE.md                     |
| 5    | Create git tag `vx.x.x`                                              |
| 6    | Push to origin (triggers GitHub Actions release workflow)            |

The script is **idempotent**: re-running with the same version skips already-done steps.

### Pass 1 — Script bumps + generates changelog

```
node scripts/release.mjs <version>
```

Steps 1–2 run, then Step 3 detects stale `RELEASE.md` and exits.

### Between passes — LLM generates RELEASE.md

Read `CHANGELOG.md` and use `skills/version-release/release_template.md` as a format reference:

1. Group related commits into feature areas
2. Attach commit hash links (use the actual repository URL once a remote is configured):
   ```
   - **Feature description** — detail | [`hash`](https://github.com/<owner>/<repo>/commit/hash)
   ```
3. Write the curated body to `RELEASE.md`
4. Run `npx prettier --write RELEASE.md` (matches the Prettier config in the repo root). `release.mjs` gates `RELEASE.md` only on the version heading, and CI `verify` runs `format:check` — a trailing-newline or line-length violation in the release body would block the release job.

**Do NOT commit** — Pass 2 will include `RELEASE.md` in the release commit automatically.

### Pass 2 — Script commits + tags + pushes

Re-run the **same** command — already-bumped steps skip, `RELEASE.md` check passes:

```
node scripts/release.mjs <version>
```

Steps 3–6 run: check, commit, tag, push to origin.

### Semantic bump

```
node scripts/release.mjs patch    # 0.1.0 → 0.1.1
node scripts/release.mjs minor    # 0.1.0 → 0.2.0
node scripts/release.mjs major    # 0.1.0 → 1.0.0
```

Same two-pass flow applies.

For a semantic bump, the target is calculated from the version committed at `HEAD`. On the second pass, the script recognizes that exact uncommitted target and reuses it instead of bumping again; any other working version is rejected as an inconsistent partial release.

### Regenerate mode

Re-releases the current version by dropping the old release commit + tag from history first, then re-running the normal flow:

```
node scripts/release.mjs --regenerate <version>
```

Uses `git rebase --onto` to surgically remove the previous release commit (preserving other commits' content and timestamps) and deletes the old tag. The normal flow then creates a fresh changelog, commit, and tag.

Only this explicit `--regenerate` path force-pushes (`--force-with-lease` for the branch and `--force` for the replaced tag). Normal releases always use a regular push and stop if the remote has diverged; ahead/behind counts never authorize an implicit history rewrite.

`--regenerate` requires a strict explicit semantic version and an existing tag that points to a release commit. All version arguments are validated before any tag/rebase command; malformed or shell-like input is rejected.

The tagged commit subject must exactly equal `🔖 chore[release]: bump version to <version>`, and that commit must be an ancestor of the current `HEAD`. Regeneration refuses tags on unrelated branches before invoking rebase, preventing an unrelated upstream from replaying and force-pushing the entire current branch.

All child processes receive argument arrays rather than interpolated shell strings. This is required even for branch names because Git permits shell metacharacters in refs; changelog revision arguments are separated from Git options with `--end-of-options`.

Changelog Git failures are fatal. An invalid `--from` revision or a failed tag query must exit non-zero rather than being reported as an empty commit range, which could otherwise leave stale release notes in a release commit.

`RELEASE.md` is considered current only when its first heading is exactly `# SecPivot Desktop v<target version>`. A version mention elsewhere in stale release notes does not satisfy the second-pass gate.

### Dry run

```
node scripts/release.mjs --dry-run <version>
```

Previews the resolved target version, generated changelog, and `RELEASE.md` status without modifying version files, `Cargo.lock`, `CHANGELOG.md`, commits, tags, or remotes.

## Standalone tools

These can be run independently:

```sh
node scripts/version.mjs <version>        # bump version only
node scripts/version.mjs patch|minor|major  # semantic bump
node scripts/version.mjs --current         # show current version

node scripts/changelog.mjs                # generate changelog since last tag
node scripts/changelog.mjs --all           # full history changelog
node scripts/changelog.mjs --from v0.1.0   # from specific tag
node scripts/changelog.mjs --preview       # preview without writing
node scripts/changelog.mjs --preview --version 0.2.0  # preview for a target version
```

## Release body (`RELEASE.md`)

`RELEASE.md` is the canonical release body for GitHub Releases. It is **manually curated** by the LLM during each release, following the format in `skills/version-release/release_template.md`.

Pushing the tag triggers CI/CD which reads `RELEASE.md` automatically as the GitHub Release body.

## Post-release

After a successful release, report:

1. New version number
2. Tag created (`vx.x.x`)
3. Release has been pushed to origin (GitHub Actions will build artifacts automatically)

## CI/CD

Pushing a semantic-version tag (`v<major>.<minor>.<patch>`, with an optional prerelease suffix) triggers `.github/workflows/release.yml` which (desktop is Windows-first and bundles only the NSIS installer; the Android APK is a secondary artifact built on Linux):

- Fails before verification/build if the release tag does not exactly match the identical versions in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`; manual dispatch therefore cannot publish an arbitrary version from unrelated source
- Runs `npm run verify` on `windows-latest`, plus an **extreme-release build** validation that verifies the exact GitHub-release configuration (fat LTO, opt-level 3, codegen-units 1, `target-cpu=x86-64-v3`) compiles and links
- Builds the Windows x64 bundle with extent runtime optimization via `CARGO_PROFILE_RELEASE_*` + `RUSTFLAGS` (`tauri-action` → NSIS `SecPivot_<version>_x64-setup.exe`)
- Packages a portable (no-install) ZIP (`scripts/package-portable.ps1 -SkipBuild -ReleaseExe <exe> -Version <ver>`) and uploads it to the same release as `SecPivot-<version>-portable.zip`; a missing exe/ZIP or failed upload fails the job
- Builds a four-ABI **universal release Android APK** in parallel on `ubuntu-latest` (`android` job, `needs: verify`) — the same `CARGO_PROFILE_RELEASE_*` extreme-optimization overrides as the Windows build apply (`target-cpu=x86-64-v3` is x86-64 only and is not set here), avoiding a needlessly large unoptimized native library. The Rust build requires vendored `openssl-sys`, and the workflow exports the selected NDK's `llvm-ranlib` as `TARGET_RANLIB` because `openssl-src` otherwise falls back to a removed `<target>-ranlib`; all signing secrets are mandatory and `apksigner verify` gates the exact universal APK. The verified APK crosses jobs as a one-day Actions artifact, and `publish-android` waits for both builders before uploading it to the draft release, so a slow Windows LTO build cannot trip a fixed polling timeout; a missing APK/release or failed upload still fails closed
- Creates a draft GitHub Release with these artifacts using `RELEASE.md` as the release body

Local `cargo build`/`tauri build` uses the fast build-speed `release` profile (see `skills/secpivot-dev/SKILL.md`); only this GitHub workflow applies the slow, runtime-maximizing release overrides.

The workflow also supports `workflow_dispatch` with a `version` input (e.g. `0.2.0`): the `RELEASE_TAG` env (`github.ref_type == 'tag' ? github.ref_name : v<version>`) drives `tauri-action`'s tag name. The input must equal the checked-out repository version; use the ref selector in GitHub Actions to dispatch the matching release commit.

## Version source files

| File                        | Key        |
| :-------------------------- | :--------- |
| `package.json`              | `.version` |
| `src-tauri/tauri.conf.json` | `.version` |
| `src-tauri/Cargo.toml`      | `version`  |

All three are updated atomically by `scripts/version.mjs`.

## Error recovery

If the release script fails mid-way:

- If version was already bumped: run `git checkout -- .` to revert config files
- If commit was created but tag failed: `git reset --soft HEAD~1` then re-run

## Commit message format

Release commits use the gitmoji convention:

```
🔖 chore[release]: bump version to x.x.x
```
