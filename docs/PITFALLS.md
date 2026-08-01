# Pitfalls

Recurring traps discovered while developing KeyVault. Read before touching the relevant subsystem.

## Frontend

- **Do not persist passwords in config or localStorage.** The browser fallback stores only demo data (`demo://vault.kdbx`). Never store a real vault password outside the backend session.
- **`.settings-scroll` relies on flex-shrink.** It scrolls only because its parent `.settings-content` is a fixed-height flex column and the scroll div has `min-height: 0; overflow: auto`. Do not add `height: 100%` or wrap it in an unconstrained parent.
- **Theme updates must go through `applySettingsToDocument`.** Changing `appSettings.general.themeColors` does not re-theme the page until bootstrap applies it. Panels that edit theme colors should subscribe to the store and reapply.
- **Nested store state must be replaced, not mutated.** `appSettings.updateGeneral("fontSizes", {...})` with a new object. Mutating `s.general.fontSizes.base` in place is invisible to persistence.
- **Number inputs in settings are textfields with hidden spin buttons** (`.settings-input`). Reuse that class; do not hand-roll another appearance reset.
- **`--slider-pct` must be initialized on first render.** Derive it from value/min/max in the template so the filled track is correct before any input event.
- **The edit tool matches LF only.** Files written by PowerShell tools may carry CRLF (e.g. `VaultWelcome.svelte` did); large `edit` oldStrings then fail with "could not find". Normalize the file to LF with the .NET API (UTF-8, no BOM) first — git's `core.autocrlf` keeps the diff clean — then use small edits.
- **Remote (`s3://`) paths must stay out of recent files and the lock-screen reopen.** `rememberRecent` skips `s3://` paths and the remote open/create methods never set `vault.remembered`, because the local open flow cannot reopen a remote vault.

## Backend / vault

- **KDBX passwords never enter `config.json` or error strings.** Keep key material inside the session state; error messages must be generic on auth failure.
- **Keyfile bytes live in the session, never the frontend or config.** `open_vault`/`create_vault` take a user-picked path; the backend reads and holds the raw bytes for save and clears them on `close_vault`. A passwordless vault is valid only with a keyfile (KeePass-compatible).
- **The root group is virtual.** Frontend `VaultState.root` maps to the DB's root group; keep `children`/`entries` split consistent or save will be wrong.
- **UUIDs must be stable across the session.** Render KDBX uuid bytes as hex and reuse them as the frontend `uuid`; regenerating on each mutation breaks selection/editing.
- **Save must re-open with the stored key, not re-derive from a lost password.** Hold `DatabaseKey` or the password (and keyfile bytes) in the session for the duration; zeroize on close.
- **Browser demo behavior is not desktop evidence.** Do not check desktop KDBX TODOs from `npm run dev` screenshots or localStorage behavior.
- **The entry-list TOTP badge computes client-side** (`computeTotp` in `src/lib/utils/totp.ts`) so the list stays IPC-free, while the detail-panel `TotpWidget` uses the backend `totp_code` command. Keep both implementations in sync (same period/digits/SHA defaults, otpauth URI support); the badge is display/copy-only and never an authority.

## Settings / style

- **Never re-define shared primitives in a panel.** Before adding CSS, grep `settings-shared.css` and `SettingsDialog.svelte` for the same primitive.
- **Do not use a parent scoped selector to style a child component.** Pass props/classes or move the rule into the shared stylesheet.
- **Segmented controls (theme/KDF/charset)** follow one pattern: bordered chip, `--settings-control-radius`, active = selection tint. Do not introduce a second segmented style.
- **Keep the sidebar width fixed (168px).** Narrow-window behavior hides the sidebar below ~560px; preserve the single-column fallback.

## Remote (S3)

- **The crate is `rust-s3`, not `s3`.** `s3 = "0.34"` fails dependency selection. Depends on `tokio` (`rt-multi-thread`) for the internal runtime; default features give the tokio + native-tls backend, which works on Windows via schannel.
- **rust-s3 0.34 API traps** (verified in `rust-s3-0.34.0/src/bucket.rs`, `request_trait.rs`): `Credentials::new` takes `Option<&str>` (pass `Some(cfg.access_key.trim())`); `get_object` returns a single `ResponseData` with `status_code()`/`to_vec()` (not a tuple); `put_object(path, data)` takes **2** args — the 3-arg variant is `put_object_with_content_type(path, data, content_type)`; list page contents have `size: u64` and `last_modified: String`.
- **S3 keys are plaintext in `config.json` by design** (secondary credentials). Never log them; error paths in `S3Storage` must not echo the secret (they don't — they only format the crate error).
- **No live S3 evidence in this environment.** No docker/minio/aws CLI — `S3Storage` is only code-reviewed; the offline suite uses `MemoryStorage`. Do not claim real-S3 behavior is verified; keep the limitation noted in `data-contracts.md` and `TODO.md`.
- **`block_on` inside async Tauri commands is fine** here because each `S3Storage` owns its own tokio `Runtime`; commands stay async so they run off the main thread.
