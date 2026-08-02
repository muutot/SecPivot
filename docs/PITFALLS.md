# Pitfalls

Recurring traps discovered while developing KeyVault. Read before touching the relevant subsystem.

## Frontend

- **`$derived(get(store))` is frozen in Svelte 5.** `get()` subscribes inside `untrack`, so a `$derived` whose only inputs come from `get(appSettings)` evaluates once and never re-runs — inputs fed from it show stale values after the store changes (reported as "远程配置改完又变回老数据"). Fix pattern (as in `RemoteSettingsPanel.svelte`): mirror the store into `$state` and re-subscribe in an `$effect`, then derive from the mirror. The same applies to `$effect(() => { ... get(appSettings) ... })` with no other reactive reads — it never re-runs.
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
- **Bridge error envelopes never include decrypted plaintext.** `bridge.rs`/`bridge_server.rs` return fixed Chinese error strings (e.g. `关联校验失败`); never format an error with a decrypted field or the bridge key. `eprintln!("bridge: {e}")` in `lib.rs::sync_bridge` only logs server lifecycle errors (bind/thread), which carry no secrets.
- **The associate approval holds the `VaultSession` mutex.** `handle_request` runs under the session lock and `request_approval` blocks up to 120 s on `recv_timeout`; a pending browser approval therefore stalls other vault commands. Accepted for Phase 1 (single-window desktop app); do not "fix" it by dropping the lock before dispatch without first adding per-request session leasing.
- **Never use `WDA_MONITOR` (0x1) for the screen-capture guard.** `SetWindowDisplayAffinity(WDA_MONITOR)` renders the window as a solid **black box on the physical display** — after unlocking a vault the whole window appears black. Use `WDA_EXCLUDEFROMCAPTURE` (0x11, Windows 10 2004+) instead: the window stays fully visible while screenshots/recordings/sharing omit it (see `shield.rs`).
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
- **No live S3 evidence in this environment.** No docker/minio/aws CLI — `S3Storage` transport is exercised by a local mock HTTP S3 server test (`remote::tests::s3_transport_round_trips_against_local_mock`); real-provider behavior stays code-reviewed. Keep the limitation noted in `data-contracts.md` and `TODO.md`.
- **rust-s3 has no overall request timeout** — only a TCP connect timeout (default 60 s), and even that is bypassed by DNS hangs or servers that accept then stay silent, which previously left the remote UI on "正在加载…" forever. `S3Storage` now pins a 15 s connect timeout (`with_request_timeout`) and wraps list/get/put in `tokio::time::timeout` (30 s / 120 s). Never remove those bounds.
- **Never call `Runtime::block_on` on an async runtime worker.** Tauri `async fn` commands run on tokio worker threads; `S3Storage`'s sync methods (`list`/`get`/`put`) `block_on` their own shared runtime, and tokio panics ("Cannot block the current thread from within a runtime") when that happens inside a runtime — the command future aborts, the invoke promise never resolves, and the UI hangs on "正在加载…" with a disabled refresh button and no error text. This silently broke the S3 browser until the commands hopped to the blocking pool first. Contract: sync S3 commands are only safe from the blocking pool / sync contexts; async commands must wrap them in `spawn_blocking` (`s3_list_objects` → `remote::list_objects_async`; open/create wrap `prepare_remote_open`/`prepare_remote_create`). Regression test: `remote::tests::s3_list_objects_async_works_from_runtime_worker_thread`.
- **`block_on` inside async Tauri commands is fine** here because each `S3Storage` owns its own tokio `Runtime`; commands stay async so they run off the main thread.

## KeePassRPC write path (keepass crate)

- **Group/entry lookups must be downward-only recursion.** `GroupRef` borrows the `Database`, so walking `parent()` chains back toward the root produces E0597/E0515 lifetime errors. Find groups/entries by descending the tree (see `find_rpc_group_id` / `find_rpc_entry_urls` in `vault.rs`); the root group itself is reachable via `db.root()` / `root_mut()`.
- **`Database::entry_mut(id)` / `group_mut(id)` are flat lookups** — they resolve any group id at any depth, so no recursive walk is needed for the write itself, only for the read-side checks (recycle-bin containment etc.).
- **History snapshots come from `EntryTrack`, not manual clones.** `entry.edit_tracking(|tracked| { let mut e = tracked.as_mut(); /* edits */ })` snapshots the pre-edit entry into `entry.history` on drop (the plugin's `CreateBackup` equivalent). The historical clone strips its own history, so nested edits do not grow history exponentially. Do not hand-roll a clone-and-insert approach.
- **Do not delete stale custom fields on `UpdateLogin`.** The plugin's update path overwrites fields but never removes custom fields the extension no longer sends; KeyVault mirrors that (deviation documented in `data-contracts.md`), otherwise app-managed fields would be destroyed.
