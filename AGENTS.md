# SecPivot repository instructions

SecPivot is a professional, compact, high-information-density KeePass client built with Svelte 5 + Tauri 2 + Rust. Before changing this repository, read `skills/secpivot-dev/SKILL.md` completely and follow its linked maintenance workflow.

In particular:

- Audit TODO completion from direct evidence before checking an item.
- Preserve the approved compact visual language and apply the theme-token gate before any CSS or markup change.
- Keep parallel agent ownership non-overlapping.
- Commit one verified minimal feature or fix at a time.
- Never log, persist, or transmit passwords or vault master keys beyond the in-memory session.

## Commit message format

Follow the gitmoji convention:

```
<gitmoji> <type>[<scope>]: <message>
```

- **gitmoji**: single emoji indicating the change category.
- **type**: lowercase change type matching the emoji.
- **scope**: optional, lowercase, in **square brackets**.
- **message**: concise imperative description, English only. Never use Chinese (or any other language) in any part of the commit message.

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
| 🔒    | feat     | privacy, security, permissions, defaults enforcement |
| 🔌    | feat     | exposing commands / connecting frontend to backend   |
| 💾    | feat     | storage / persistence                                |
| ⚙️    | feat     | configuration / settings                             |
| 📦    | feat     | packaging / dependencies                             |

## Verification

```powershell
npm run check
npm run build
npm run test:rust
npm run lint:rust
npm run format:check
npm run verify
```

Run the narrowest relevant checks during implementation and the full set at integration milestones. Local `dev`/`release`/`ci` Rust profiles are tuned for build speed; the extreme runtime optimization (fat LTO, opt3, cgu1) is applied only by the GitHub Actions release workflow via `CARGO_PROFILE_RELEASE_*` env overrides (see `skills/secpivot-dev/SKILL.md`).

## Reference projects

- `clipboard` (sibling): source of truth for the approved visual language and settings primitives.
- `MuuPass` (sibling): reference for KeePass domain behavior (TOTP, KDF, auto-type). Do not edit either during SecPivot work.
