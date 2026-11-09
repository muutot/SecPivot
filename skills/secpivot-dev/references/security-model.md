# Security Model

SecPivot stores credentials in standard KDBX 4.0 files. This reference documents the security invariants the frontend and backend must preserve.

Both CSV writers prepend the existing spreadsheet formula guard before quoting
the cell, keeping commas/quotes/newlines inside it. Do not move that apostrophe
outside RFC quotes or remove literal input apostrophes on import. Guarding alters
the exported field value; the wire regressions prove column and guarded-value
fidelity, not native spreadsheet execution behavior or lossless migration.

CSV import rejects malformed quoting for the whole document before submitting
entries. Parser errors carry no field contents; IO translates them and the page
reports only in the initiating view. It never repairs ambiguous malformed CSV by
discarding quotes or concatenating records, and never strips literal apostrophes.

`util::atomic_write` creates a unique random sibling per write with `create_new`
and owner-only permissions on Unix, flushes the same writable handle, then
renames it onto the target. Cleanup only removes that operation's own file;
concurrent writes cannot truncate/remove another pending writer's sibling. Old
fixed `.tmp` files are not deleted or reused. This protects complete-payload
publication, not higher-level revision order; vault persistence still needs its
existing admission gate. Windows host tests cover concurrent replacement and
legacy sibling preservation; other native platforms remain unverified.

Native plaintext export and attachment-save pickers share `io.pickSaveFile`.
Android selects a writable SAF document with the export MIME; desktop keeps the
save dialog. Both attachment entry points and CSV/XML/HTML preserve the original
view epoch before resolving backend secrets or writing. CSV/XML confirmation and
HTML's captured password opt-in remain required. The shared named writer uses
atomic local replacement or verified URI rollback; temporary command payloads,
rollback/readback buffers and failed capped reads are zeroized on return. Export
URIs never enter recent-vault settings. Device/provider runtime remains pending.

## Core invariants

- **Desktop development CSP**: HTTP and HMR WebSocket connections permit only
  the configured loopback Vite port 14200 (`localhost` and `127.0.0.1`), plus the
  existing same-origin/IPC sources. Vite may advertise the numeric address when
  the page uses localhost. Production CSP has no additional development origins.

- **Local-only**: the vault master password and the derived key never leave the machine. There is no telemetry or automatic upload. The one exception is the opt-in remote vault feature (S3 or WebDAV): opening or creating a remote vault sends the KDBX payload (encrypted with the master key) to the configured endpoint, and `save()` uploads it back. Local vaults never touch the network. Master passwords still never leave the machine 鈥?they cross the IPC to the backend only, and never to the remote storage.
- **In-memory session**: the decrypted database and the master password live only in backend managed state while a vault is open. `close_vault` and lock paths clear the session and zeroize the password buffer.
- **No vault-secret persistence**: config (`config.json`) never stores vault master passwords, entry passwords, or vault content. Opt-in remote-storage credentials are the narrow exception: on Windows they are DPAPI-encrypted before persistence; desktop non-Windows uses OS-keyring references; mobile currently retains legacy passthrough. The browser demo fallback stores fake demo data only.
- **Clipboard hygiene**: copying a password schedules an automatic clear after `clipboardClearSeconds` (0 disables). The timer is reset on each copy and skipped when the user copies other content afterward. Lock clears the clipboard when `clearOnLock` is enabled. A backend safety net (`clipboard_schedule_wipe` in `commands/clipboard.rs`) zeroizes the incoming text before scheduling one shared worker that retains only the latest zeroizing SHA-256 fingerprint, and clears after the same interval when the clipboard content matches that fingerprint — surviving webview death before the renderer timer fires. The renderer retains only an ephemeral SHA-256 fingerprint, never a module-global password, and keeps no ownership when the timer is disabled. Independent copy identity prevents an old browser read from clearing or releasing a newer identical copy. Zero-second schedules cancel prior generations. Schedules are generation-guarded: a newer copy or an explicit cancel (`clipboard_cancel_scheduled_wipe`, invoked by the lock path) supersedes pending wipes so they never destroy unrelated content.
- **Whole-entry clipboard transfer (KeePass "Data Exchange")**: the entry-exchange payload is a gzip-compressed KDBX XML document holding plaintext field values (protected fields are flagged, not encrypted), so _both_ variants must be treated as secret-bearing: "Copy Entry (Unencrypted)" is readable by anything that can read the clipboard, and "Copy Entry (Encrypted)" only adds a Windows DPAPI wrapper (`CryptProtectData`, current-user scope, KeePass' `ClipDomainSep` optional entropy) around the same plaintext document — DPAPI protects it at rest in the clipboard, not from the current user. Neither variant goes through the text wipe path (`clipboard_schedule_wipe` compares text), so the entry-exchange copy schedules its own wipe of that clipboard format (`clipboard_schedule_exchange_wipe`, sharing the same generation counter, so either schedule supersedes the other). A payload whose flag claims DPAPI protection must never fall back to being read as unencrypted. The clipboard is attacker-writable (any local app can set the format), so every read is size-bounded before allocation and bounded again after inflate. The payload is built, framed and written entirely in the backend and never crosses the IPC, so the renderer cannot read or leak it; the browser demo has no binary clipboard bridge and therefore disables the whole command group instead of silently degrading to a text copy.
- **No secret logging**: password and vault content must never be written to logs, debug output, or error strings.
- **Read-only degradation**: after 3 consecutive failed saves the session reports `readOnly` and refuses the write path (`save`, master-key change, storage re-encrypt) with guidance to use save-as; `save_vault_as` stays available and a successful persist resets the counter. The UI disables the save button and shows a "鍙" badge so users never assume changes were persisted.
- **Remote overwrite guard**: remote saves never blindly overwrite 鈥?the session records the SHA-256 of the remote bytes at open/last save and refuses to `put` when the current remote hash differs (`REMOTE_CHANGED` error with remote/local sizes). This prevents a concurrent device's changes from being silently lost; conflicts are resolvable (merge by entry/group UUID + last-modified via `merge_remote_vault`, force overwrite via `save_vault(force=true)`, or download-and-replace via `refresh_remote_vault`, the latter discarding local unsaved edits with an explicit confirm) and never trigger read-only degradation.
- **HIBP k-anonymity**: the opt-in breach check (`check_hibp`) sends only the first 5 hex chars of each password's SHA-1 to `api.pwnedpasswords.com`; passwords and full hashes never leave the session, the check only runs on an explicit user action (privacy screen + explicit start), and results carry no secrets. Network failures surface as errors without falling back to anything that leaks password material.
- **Plaintext export guard**: CSV and emergency-sheet exports resolve passwords server-side only on explicit user action. The HTML emergency sheet embeds plaintext passwords only when the user ticks the opt-in checkbox (the dialog shows a warning first), and the sheet itself carries a visible "contains plaintext passwords" banner; that opt-in value and the visible-session epoch are captured before opening the native save picker, so a tab switch cannot make an old export read a new dialog's checkbox or export another tab. `export_csv`/`export_xml`/`export_emergency_sheet` are the only write paths that ever persist secrets outside the KDBX; XML export always embeds passwords (KeePass PlainXml: escaped plaintext with `ProtectInMemory="True"`) and therefore requires the same explicit plaintext confirm as CSV.
- **Controlled attachment extraction**: external viewing writes attachment bytes only under `<system temp>/secpivot-attachments/<random token>/` with sanitized single-segment file names (no path escape), each registered in the `AttachmentTempStore` together with its originating `sessionId`. The frontend confirms explicitly before opening; import-back must present both that token and the same addressed session, and the backend rejects a cross-session replay before reading or mutating any attachment (never an arbitrary path, 64 MiB cap), even after a tab switch. Token validation, metadata and file reading occur before the vault-session lock; the short locked phase only replaces the attachment bytes and snapshots the database. External-open registration revalidates the originating session after writing the token, so a tab closed during extraction cannot leave a newly orphaned file. `cleanup_attachment_temp` discards a token's directory; single-tab close cleans only that session and the all-tabs lock retries every remaining token, with all filesystem work running after the vault lock is released. A failed removal stays registered in both backend and frontend bookkeeping for a later cleanup attempt instead of being reported as already discarded. The attachment dialog invokes cleanup for explicit close, Escape/unmount, resource replacement, discard and replacement by a newly opened extraction, while a failed import retains the token so the user can retry. Its preview, native save picker, external open and import completion share one session+entry+attachment view token, so a stale preview/open result cannot publish data or open a path, a stale picker result cannot start an export, and a stale import completion cannot invoke its parent callback. Temp content is never logged and never written anywhere else; previews (`preview_attachment`) stay fully in memory. Explicit 鈥渟ave attachment鈥?exports from both the detail row and preview dialog bind the native destination picker to the originating view and refuse to start the backend write when that view is stale.
- **Passwords never reach the frontend**: `VaultState`/`VaultEntry` carry no entry passwords in the Tauri runtime. Reveal/copy, editor prefill, CSV export, and the security report resolve secrets server-side (`get_entry_password`, `export_csv`, `security_report`) so plaintext crosses the IPC only on explicit user action. The browser demo fallback keeps fake passwords in `VaultEntry.password` for its local simulation only. **Protected custom fields follow the same rule**: their value is never part of `VaultEntry` (only the `protected` flag is); reveal/copy/editor-prefill resolve them via `get_custom_field_value`. The detail-panel inline editor opens only after a protected value is revealed, and writes back through `update_custom_field` with the same protected flag, so editing never downgrades protection. Reveal state changes only after the requested secret is successfully loaded for the still-current detail-view epoch; session/entry changes and even a same-entry revisit (u1鈫抲2鈫抲1) invalidate the first request, leaving the new view concealed. Password copies apply the same visible-view epoch immediately before the clipboard write, so an A鈫払鈫扐 late response from the first A can never copy its secret into the second A view. `EntryDetail` uses one session+UUID token for both the password/protected-field read and clipboard consumer, and invalidates that token when the component is destroyed, so a detached detail pane cannot copy a late secret. CSV export resolves protected fields server-side like passwords. The entry-list password column shows a masked placeholder driven by the `hasPassword` flag (never the value); clicking a masked cell resolves that one secret via `get_entry_password` bound to the current session view (`revealEntryPassword` in `+page.svelte` 鈥?staleness/failure keeps it masked) and `EntryTable` re-masks on second click, mouse leave, 10 s timeout, and unmount.
  **Browser bridge (KeePassHttp, opt-in)**: when `bridge.enabled` is set, a loopback HTTP server listens on `127.0.0.1:19455` only 鈥?never a non-loopback interface 鈥?and serves the KeePassHttp protocol (`associate`/`test-associate`/`get-logins`/`set-login`/`get-logins-count`/`generate-password`). Association keys live in the vault session (like keyfile bytes) and are wiped on lock/close; nothing bridge-related is persisted to `config.json`. Each response carries a fresh random `Nonce` used as the AES-CBC IV; sensitive fields (`Url`/`SubmitUrl`/`Login`/`Password`/`Uuid`/`Realm`/`Names[]`) are encrypted per-field with AES-256-CBC + PKCS7 and the whole response is authenticated by HMAC-SHA256. A first-time `associate` validates the key/verifier and captures the stable backend-active session under the vault mutex, then waits up to 120 s for explicit user approval through a frontend event (`bridge-associate-request`, payload `{ token, id }` 鈥?never keys) answered by the `bridge_approve` command **without retaining that mutex**. Unanswered requests are rejected on timeout; allow/deny/timeout all invalidate the one-shot token. Completion registers the key only if the same session is still backend-active, so a tab switch/close cannot redirect an approval to another vault, and the pending key is zeroized on every abandoned path. A locked vault answers with a plain error envelope, never triggers auto-unlock, and never opens the approval prompt. Server-side, no decrypted plaintext is ever logged; errors in `bridge/server.rs` are HTTP-status-agnostic envelopes. Protocol dispatch runs under `std::panic::catch_unwind` with any `MutexGuard` living outside the unwind closure, so a handler panic (e.g. a hypothetical OS RNG failure in `random_bytes`) drops the guard normally and never poisons the `VaultSession` mutex 鈥?later requests keep working.
- **KeePassRPC (Phase 2, opt-in)**: when the KeePassRPC server is enabled, a loopback WebSocket server listens on `127.0.0.1:12546` only and speaks the KeePassRPC protocol (SRP-6a setup 閳?AES-256-CBC + protocol SHA-1 MAC JSON-RPC frames). SRP-derived session keys are held in the vault session per client username (like `bridge_keys`/keyfile bytes). Closing a database tab always wipes its keys. During the all-tabs lock path, `rpc.keepSessionAfterLock` (default) retains only the backend-active session's keys in the locked active slot and transfers them to the next successfully unlocked vault, so Kee can reuse the fresh vault without another side-channel password (official KeePassRPC behavior); parked-session keys are wiped because RPC never serves parked tabs. Locking always wipes the master password, keyfile bytes, and the KeePassHttp `bridge_keys` regardless. Nothing RPC-related persists to `config.json`; a fresh SRP handshake shows the randomly generated side-channel password only in the desktop UI (user types it into the browser extension), never logged, expiring with the pending handshake (120 s), and key-auth challenges (`cr`/`sr`) are SHA-256 over the session key. A locked vault answers with an error envelope and never auto-unlocks; a failed auth tells the client `AUTH_FAILED` so it drops its stored key and falls back to SRP. Plain HTTP availability probes receive 404 with CORS before the WebSocket handshake; valid WS handshakes to a path other than `/` get 404 (availability probe). Server-side diagnostics log only protocol stages/field presence and never decrypted URLs, client usernames, credentials, session keys, or side-channel passwords. `rpc.sessionTimeoutSecs` (default `0` = never) caps the retained SRP keys' lifetime: the countdown restarts on every vault unlock and at each new key registration, and a lapsed key set is wiped lazily at lookup (forcing re-authorization). The server also tracks live connections in `RpcState` (id/username/peer/connect time/auth flag only 鈥?no secrets); the settings UI lists them via `rpc_sessions` and can force-close one via `rpc_close_session` (socket shutdown).

## Lock lifecycle

The idle activity watcher observes keyboard events in window capture, before
menus and child controls consume navigation. Discrete keys reset the full idle
deadline even when their bubble is stopped; cleanup removes the same capture
listener. This does not change lock duration, disabled mode or focus-loss lock.
The deterministic menu regression exercises the actual timer functions with a
fake clock and verifies both the refreshed deadline and listener cleanup.

The RPC side-channel renderer uses a wall-clock deadline, rechecked by interval,
focus/visibility resume and before copying. Expiry dismisses the modal and drops
its pending password reference; unmount and manual close do the same. Timer and
copy completions are bound to the current request object, so old work cannot
alter a replacement prompt. `rpc-side-channel-ended` carries the attempt's
process-monotonic `requestId` on proof/replacement and connection exit, including
worker exit after service stop. The renderer ignores old/duplicate shows and cannot
reopen an ended attempt from delayed delivery. Backend event payloads borrow the
temporary `Zeroizing` password for synchronous serialization; begin failure and
callback unwind also wipe that buffer. `VaultSessions` ends registered active
lifetime signals on successful open/create adoption, active close, lock or tab
switch. RPC prompt leases subscribe while the active vault mutex is held, so an
idle connection's prompt ends without another setup message. Failed/idempotent
transitions and parked-tab close leave the current prompt alone. An ended lease
also rejects proof after A→B→A; matching the old session id again cannot revive
that attempt. Observer callbacks run after registry/observer guards are released,
retain no secret, and cannot poison the active mutex on unwind. This does not
prove JavaScript heap zeroization or native event delivery on every target.

RPC connection keys prove frame integrity, but do not outlive host authorization.
Read dispatch and write preparation revalidate the connection's username and
32-byte key against the current host under the vault mutex; `rpc_key` applies
lazy expiry here. Revoked, rotated, expired or absent keys close the data path,
even when the socket still holds a cached copy. Write validation runs after
the persistence gate, so a queued request cannot bypass an intervening expiry.
Already admitted durable writes retain their normal completion/adoption boundary.
Stored-key proofs also revalidate their captured challenge key and identity;
retained keys may still authenticate while locked, but data requests answer
locked until unlock. Fresh SRP proof completion requires an open vault and the
same stable `VaultSessions` id that owned its identify step. A tab switch or
lock/reopen cannot create authorization on a different vault. Temporary key
comparison copies and hex encodings are zeroized after use.

Browser integration admits connections through `loopback.rs` before spawning
workers. Each opt-in service has a 32-worker cap (64 total if both are enabled),
including unauthenticated HTTP/WebSocket requests and approval waits. Excess
connections are closed without a queued socket or new thread. Managed service
state retains permits across listener restarts; worker return, spawn failure
and unwind release them. Each listener start owns a fresh `ListenerLifetime`:
stop invalidates that generation, shuts down its registered sockets and rejects
pending association waits. Requests queued on the vault mutex revalidate the
generation before dispatch; a restarted listener cannot revive old requests or
register their keys/sessions. Short dispatch is linearized with stop under the
lifetime mutex, acquired after the vault mutex; socket I/O, storage work and
approval waits never run under that lifetime guard. RPC response writes also
run outside the vault mutex. An admitted durable RPC write still finishes and
adopts its committed result, retaining its permit until the worker returns.
Windows clone shutdown can leave an existing recv blocked despite succeeding;
`loopback/stream.rs` polls native reads/writes at 100 ms, checks cancellation
and preserves the protocol's longer logical read timeout. Already rendered
association prompts have no cancellation event yet, although backend approval
tokens expire; SRP attempts end through their connection lease on worker exit.
This bounds worker amplification, not all resource use or
availability against a local process deliberately occupying every slot.

RPC upgrade detection peeks a fresh bounded snapshot of the unread TCP bytes;
it never appends repeated peeks or consumes the head/first frame. Classification
waits for the real header terminator, bounds only the header to 64 KiB, parses
the Upgrade field inside that header, and yields briefly while a partial head
awaits more bytes. Partial-head observation has a 30-second deadline. HTTP
availability probes keep their 404/CORS response. These transport tests use
isolated loopback sockets, not a live browser extension or Tauri runtime.

Lock happens on:

- explicit lock action (`close_all_vaults`; ordinary tab close uses `close_vault` and does not enter the retained-RPC lock lifecycle);
- idle timeout when `autoLockMinutes > 0` (timer refreshed on `pointerdown`/`keydown`/`mousemove`/`wheel`/`scroll`, re-armed at most once per 15 s to avoid churn, reset on vault open, skipped when 0 or no vault open; the settings subscription re-arms only when `autoLockMinutes` itself changes, never on unrelated settings writes);
- `lockAfterAction` immediately after a password copy;
- focus loss when `lockOnFocusLoss` is enabled (installed from `+layout.svelte` via `installFocusLock`; locks only while a vault is open). TCATO suppresses this path while an open attempt is pending and while the backend-confirmed overlay exists; pending attempts hold independent leases, so one late failure cannot clear another attempt's confirmed overlay state and accidentally re-enable focus-loss locking.

The frontend lock path (`lockVault` in `src/lib/services/security.ts`) zeroizes every session by calling `vault.closeAll()` (which closes all open tabs and discards extracted temp attachments), and starts clipboard clearing when `clearOnLock` is enabled. Ownership invalidation is synchronous; vault closure and clipboard clearing run in parallel, so a pending Clipboard API write cannot delay session closure. The explicit clear waits behind already-started writes and cancels queued old copies, preventing a delayed write from restoring content after clearing. Password copies go through `copySensitive` so `lockAfterAction` applies only to the password, not usernames/URLs/notes. The entry right-click menu's "Copy entry" action copies the whole entry (title, username, password, URL, notes, unprotected custom fields) as one labeled text block, so it resolves the password server-side through `get_entry_password`, consumes it under the same visible-view epoch guard as a plain password copy, and passes `sensitive = true` to `copyValue` — the clipboard wipe and `lockAfterAction` apply exactly as they do for the password alone. Protected custom-field values are never part of the text (the snapshot carries an empty value), matching the rest of the renderer surface.

After lock, the frontend `vault` store is cleared and the UI shows the lock screen (`LockScreen.svelte`) when a last-database path is remembered (`rememberLastDatabase`); otherwise it returns to the welcome screen. The remembered path (`vault.remembered`) is set on open/create and cleared only via "use another database"; the password is never persisted or remembered.

With `minimizeToTray` on (default), closing the main window does **not** lock 鈥?it merely hides to the system tray, keeping the in-memory session alive. Use the tray "閿佸畾鏁版嵁搴? menu (or any lock path above) to drop the session; "閫€鍑? quits and fully ends the process.

## KeePass strengths

The `keepass` crate handles KDF (Argon2id/Argon2/AES-KDF), cipher (AES-256/Twofish/ChaCha20), and compression. Defaults favor Argon2id + AES-256 + Gzip for newly created databases. Twofish is retained for read/write compatibility with an already-Twofish database, but SecPivot does not offer it as a new creation or storage-migration target: the database-settings write contract accepts only AES-256 or ChaCha20 and rejects a forged Twofish patch at deserialization. A KDF/cipher/compression change applies the complete settings patch to a clone and re-encrypts it with the existing master key before adoption, so failed validation or persistence cannot partially alter live metadata; persistence tests verify supported changes survive reopen.

## Password generation

`src/lib/utils/password.ts` and `bridge::generate_password_with` enforce the same generator policy. A non-empty `customCharset` replaces built-in classes; otherwise every enabled upper/lower/digit/symbol class contributes at least one character. Exclusions apply before generation, unique `requiredChars` must fit the result, and pattern slots (`u`/`l`/`d`/`s`/`a`; other characters are literals) are never overwritten with an incompatible required character. Both engines use rejection sampling over OS randomness (`crypto.getRandomValues` / `getrandom`) so character and shuffle indices have no modulo bias. Empty pools, insufficient length, and unsatisfiable pattern/required combinations return an error; the editor displays it and KeePassHttp/KeePassRPC propagate it instead of silently generating with defaults. Entropy is estimated as `length 脳 log2(poolSize)`; strength buckets are `<40` weak, `<72` fair, else strong. Similar/ambiguous exclusion filters `Il1O0` and `{}[]()/\\'"`~,;:.<>` respectively.

## Threat notes / accepted trade-offs

- Keyfile unlock is supported as a second factor: the keyfile path is user-picked, its bytes are read and kept in the session for save, and cleared on close. Keyfile contents never enter config, logs, or the frontend.
- Windows Hello quick unlock: when `rememberPassword` is enabled, the master password is stored in the OS credential store (Windows Credential Manager via `keyring`, DPAPI-protected per user) keyed by vault path. The lock screen probes `get_saved_credential` and offers a one-click unlock; the credential is removed on lock whenever the setting is disabled. This is the standard KeePass trade-off: the credential manager protects the password at rest, but anyone who can read the logged-in user's credential store can also unlock the vault. Hardware challenge-response (e.g. YubiKey) remains on the roadmap.
- **One-time passwords (opt-in per entry)**: seeds live only inside the vault session; `totp_code` returns codes, never seeds. TOTP (`otp`/`TimeOtp`), HOTP (`HmacOtp`) and Steam Guard (`SteamOtp`/`steam`) seeds are recognized; HOTP advances its counter on every code request and rewrites the `HmacOtp` field server-side _without_ a history snapshot (showing a code must not pollute history), leaving the vault dirty so the next explicit save persists the counter 鈥?a code request therefore mutates the entry. The browser fallback computes TOTP only (UI surface, not desktop evidence).
- **Multi-session isolation**: every renderer-originated secret read, mutation, export, maintenance command, attachment action and explicit auto-type call carries the tab's captured `sessionId`; the backend resolves that id through `VaultSessions`. Async results update only the originating session cache, and TOTP caches are keyed by `sessionId + uuid`, so identical copies of the same KDBX cannot leak/cross-write through shared UUIDs. Page-level completion UI (selection, dialog state, success/error notices) additionally requires the exact originating visible-view epoch, so A鈫払鈫扐 cannot let the first A callback overwrite the second A view merely because the session id matches again. Bridge/RPC/global-hotkey behavior is intentionally scoped to the backend-active session.
- CSV export writes plaintext credentials (including passwords and TOTP seeds) to a user-chosen file via the save dialog and the `export_csv` command; the user must keep that file as secure as the vault itself. The security report analyzes empty/weak/duplicate passwords entirely server-side 鈥?only counts, uuids, and entropy bits cross the IPC.
- Auto-type replays entry fields as keystrokes via `enigo`; fields are resolved server-side from the vault session (the frontend never sends the password in the `auto_type` payload), a 300 ms grace period lets the user focus the target window, and execution runs on a background thread. The sequence itself may still be observed by other applications on the same machine 閳?an accepted trade-off of the feature.
- **Remote vault credentials (opt-in)**: each remote profile belongs to exactly one transport and is addressed by canonical path (`s3/config_1`, `webdav/config_1`). S3 access keys / WebDAV username+password are encrypted at rest with Windows DPAPI (`CryptProtectData`, current-user scope) and persisted as `dpapi1:<base64>` in `config.json`. They are a secondary credential, never a vault master password 鈥?anyone who can execute as the same user can still use the storage, but offline readers cannot recover readable text from the file. Desktop non-Windows stores OS-keyring references; mobile has legacy passthrough until a native secret store is wired. Credentials never appear in logs; remote commands send only the non-secret profile path over IPC and resolve decrypted credentials from `ConfigStore`. Stored credentials also never cross the IPC toward the renderer: `get_config`/`set_config` exchange masked snapshots (`accessKey`/`secretKey` blanked), `remote_secret_present` reports only presence flags for the "saved — leave blank to keep" placeholders, and empty submitted credentials keep the stored ones (clearing happens by deleting the profile); transport uses HTTPS (or the user's configured MinIO/WebDAV endpoint), and the uploaded payload is already encrypted KDBX. Local mirror copies live under `<data_dir>/Storage/remote/<kind>/<sanitized profile name>/` (portable: beside the executable; installed: app-data dir) with the same KDBX encryption. DPAPI protects against offline reads by other users/accounts, not a process already running in the same user session.
- Screen-capture guard (opt-in, default off): when `security.screenCaptureGuard` is enabled, opening or creating a vault (local or remote) applies `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` (Windows 10 2004+; Windows only) to the main window so screenshots, recordings and screen sharing omit it, while the window stays fully visible on screen. The toggle lives on the welcome page and persists in config; the backend reads the setting at open/create time. `WDA_MONITOR` must never be used here 閳?it renders the window as a solid black box on the physical display (black-screen bug) and, on some WebView2 builds, caused the main window to vanish entirely after unlock; the guard defaults off precisely to keep this class of issues away from users who did not ask for it. The guard is released on lock/close. TCATO overlay windows and the lock screen are not guarded (no secrets rendered there).
- The WebView clipboard write relies on platform clipboard; scheduled clearing uses a renderer timer plus the backend wipe thread. The remaining gap is a full process crash/exit before either fires — tray-quit clears explicitly (`RunEvent::Exit`), but a hard kill cannot be covered.

## Verification for security changes

XML exports preserve CR/CRLF through numeric CR references and reject characters
that XML 1.0 cannot represent, including invalid JS surrogate halves. Rejection
never includes a field value and occurs before destination writes; stored KDBX
credentials stay unchanged. The Rust intermediate document is Zeroizing even
when validation fails. This is a plaintext export fidelity boundary, not proof
of native client interoperability or JavaScript heap zeroization.

Android release keystore/passwords are scoped only to the inline signing step
of the separate `sign-android` job. Build jobs receive no signing secrets;
signing/publishing jobs never check out or run build sources. The keystore
lives in a private runner temp directory with exit/signal cleanup. Password
arguments use `env:` references; final staged APK signatures and exact native
ABI contents are verified before checksums/upload. Synthetic shell tests prove
control flow and cleanup, not genuine APK signatures or CI execution.

Windows conditional clipboard wipes use `clipboard_clear_if_matches` with a
32-byte fingerprint (no repeated plaintext IPC) and the
platform `clear_clipboard_text_if` / `clear_clipboard_bytes_if` helpers. Each
holds a single `OpenClipboard` guard across read, ownership/generation checks
and `EmptyClipboard`, so another process cannot replace content between them.
Text reads stop at the first UTF-16 NUL and reject missing terminators, invalid
UTF-16 or allocations over 4 MiB; custom-format reads retain the 64 MiB cap.
Read copies are zeroized after comparison. A non-Windows conditional IPC result
is null; the renderer uses a readable Clipboard API and never blind-clears on
read failure. Browser read/write still cannot provide a native atomic compare.
Scheduled text and entry-exchange wipes share one worker and one pending
deadline in `commands/clipboard/scheduler.rs`. Replacing/cancelling drops the
pending 32-byte fingerprint immediately; an in-flight task rechecks generation
before acquiring the clipboard and inside its native predicate. Each command
reserves its generation before fingerprinting, native reads or lazy worker
creation, then installs only if that reservation is still current. Cancellation
creates only scheduler state, never a worker, so first-worker initialization
cannot hide a cancel and late preparation cannot supersede a newer request.
This orders backend reservations, not renderer writes or IPC arrival across
processes. No clipboard I/O holds the scheduler mutex. Unsupported platforms
never start a native worker. Renderer hashing results are rejected after explicit clear, and
Web Crypto unavailability never falls back to retaining plaintext. Native
explicit clear returns false on unsupported platforms, allowing renderer clear.
Legacy execCommand failure is reported and its textarea is emptied/removed in
finally. External identical-copy ownership, cross-renderer write/IPC order and binary/text
copy ordering remain pending; the entire clipboard lifecycle is not resolved.
All four native live clipboard tests require `SECPIVOT_CLIPBOARD_LIVE=1` before
any OS read or probe, including vault entry exchange. Default verification leaves
the flag unset and returns a content-free skip for these live bodies; their
harness success does not prove actual clipboard interoperability. The clipboard
mutex coordinates crate tests only, and text restoration cannot restore every
foreign clipboard format. Live tests belong in an explicit disposable environment.

HOTP counter advancement preserves the existing `HmacOtp` protected flag.
A code request must not turn a protected seed into a plaintext custom-field
snapshot. The regression verifies masking and the next counter after save/reopen.

Vault and keyfile reads are bounded while streaming (512 MiB and 10 MiB),
including document URIs. SAF saves must read the previous bytes successfully
before any truncation, and both save and rollback verify the bytes read back.
Windows tests exercise the shared transaction helper; native provider behavior
still requires Android device verification.

Text-import reads share the bounded URI/filesystem reader with an 8 MiB ceiling.
Android validates the selected provider display name; desktop validates the actual
path extension. These are format checks, not filesystem authorization. Invalid
UTF-8 errors never include file content. Native picker completion is checked before
reading plaintext, and the read result is checked again before parsing/importing;
an obsolete A view cannot read a file selected after A→B→A. SAF import selections
are not added to recent vaults. Native provider access remains unverified.

An authoritative remote upload is committed before its optional local mirror.
Mirror failures retain the new session key and remote hash and surface a warning;
they never count as repeated upload failures or leave the session using an old key.

- Targeted tests for lock/clear/session semantics (Rust).
- Confirm no password reaches `config.json`, logs, or the `VaultState`/`VaultEntry` payload by construction and test. Same for protected custom-field values (only the `protected` flag is serialized; `get_custom_field_value` resolves them on demand).
- Browser fallback changes are UI-only; do not claim desktop security behavior from them.

1PIF import validates JSON-line records before publishing any rows. Concealed,
password-type and guarded custom values stay protected through the shared import
DTO and KDBX write. A concealed username stays a hidden custom field, never a
public UserName snapshot. Unmapped secure fields/password history use protected
JSON fields; duplicates/reserved names cannot overwrite or drop their values.
Binary attachments and folder hierarchy are rejected until supported. Parser
errors disclose line numbers only, never the record, and incoming command text
is zeroized after parsing. Imported secrets cross IPC only for the explicit
import flow; this does not prove JavaScript heap zeroization or native exports.

Bitwarden's plaintext import rejects account/password-encrypted exports before
mapping items. Encryption markers must be boolean when present; absent markers
remain compatible with legacy plaintext. The command's temporary input string
is zeroized on return, as with 1PIF. This protects the format boundary, not a new
decryptor, and does not prove imported data's full fidelity or JS heap erasure.
External Bitwarden custom labels are data: a `Bitwarden` prefix and unique
suffixes prevent reserved-name drops, duplicate-value overwrites and accidental
native OTP/KPRPC configuration. Hidden flags remain protected in KDBX and masked
in snapshots. `import/fields.rs` shares the allocator with 1PIF; native XML OTP
keys still retain their native semantics through their separate import path.

The renderer clipboard utility serializes its text writes, conditional native
wipes and explicit clears through one promise queue. Idle writes still start
synchronously after the caller view guard; queued outdated copies are skipped.
A new copy releases old ownership immediately, and rejected writes cannot poison
the queue. Browser read/digest waits stay outside it, revalidating ownership
before their final queued clear. This establishes one renderer's operation order,
not OS clipboard sequence ownership or cross-window atomicity. A stalled write
may still delay clipboard clearing; lock closes vaults independently. Tests use
synthetic API promises and no shared OS clipboard.

Sensitive-copy lock follow-up is admitted by `copyText(text, afterCopy)` only
for a successfully completed, still-current copy generation. `copySensitive`
watches the published active id and vault path through `KeyedViewGuard`; A→B→A,
close/reopen and the browser demo's fixed-id path change end the old lifetime.
It also checks the internal active id and current lock-after-action setting.
`VaultStore.closeAll(whenCurrent?)` rechecks the nonsecret predicate inside the
topology queue before any backend close, so an earlier queued open cannot be
closed by an old copy. Browser close rechecks after attachment cleanup. Manual
lock omits the predicate. Observers are released on success/failure; aborted
copies never lock, and failed renderer hashing retains native safety-net behavior.
This is renderer admission evidence, not multi-window native authorization.

Bitwarden typed-item import protects financial/identity/private-key values as
unique custom fields, masked in ordinary snapshots and preserved in KDBX.
Unknown nested extension values retain protected JSON. Types 2–8 disable native
password-quality checks in the same validated batch insertion; login/default
quality behavior remains enabled. Import-only metadata never changes ordinary
full-field edit semantics. Schema tests use synthetic values; real exports,
dynamic linked-field behavior, collections/history/attachments remain pending.

Bitwarden type-3 custom fields resolve official login/card/identity targets into
protected import-time copies, including username aliases. Their raw field.value
is not authoritative. Invalid/cross-type IDs and non-string target contents reject
before any rows are published; errors disclose record numbers only. The snapshots
remain masked in VaultState and survive KDBX save/reopen. This protects migrated
values, without promising dynamic synchronization when an original field changes.

Opt-in registrable-domain browser matching must respect both public registration
and private hosting tenant boundaries. The complete pinned static Mozilla PSL
replaces the hand-maintained subset; wildcard and exception rules are evaluated.
IP/nonregistrable hosts retain exact full-host identities, never equal failed
lookups. Bridge/RPC KDBX regressions reject unrelated tenants while same-site
subdomains and PSL exceptions still match. The default switch remains off;
list updates require dependency review. Native extension/runtime URL authority
and all remaining autofill authorization checks are separate evidence gaps.

Bitwarden URI import must never silently weaken source match modes or discard
destinations. Until native per-URI policies are implemented, explicit modes 0–5
and multiple nonempty URIs fail the entire parse before any bulk import. Invalid
URI/mode shapes and embedded whitespace fail with content-free record numbers.
Absent/null default mode with one URI and no-URI items remain supported; native
matching itself is unchanged. This closes an unsafe migration path, not full
Bitwarden URI compatibility or the broader autofill authorization audit.

Shared `util::url_host` parses destination authority with the existing URL
crate, excluding userinfo and canonicalizing IDNA/IP identities. Bare hosts,
host:port and protocol-relative addresses remain supported; malformed authorities,
internal whitespace/control characters and bare userinfo are rejected. Browser
credential tests exercise both host adapters after synthetic KDBX reopen in
default and registrable-domain modes. `url_authority` also retains explicitly
written ports, including defaults: Hostname compares host plus optional port,
while Domain remains deliberately broader. Renderer Hostname suggestions retain
that port and valid IPv6 brackets. A shared synthetic corpus checks both parsers
and suggested alt rules after KDBX reopen; regex interactions and native extension
authorization remain separate validation boundaries.

Exact KPRPC URL matching requires identical complete strings after validating
that both have a usable host. Scheme, path case, query, fragment and explicit
port are never discarded by this policy. Bridge preserves original request/submit
URL case; frontend Exact suggestions retain the same full input so the rule can
match the actual requested URL. Existing explicit
regex allow/block priority remains unchanged; this does not prove all rule
interactions or native extension request construction.

KPRPC regex helpers skip genuinely empty expressions, mirroring the official
plugin's `IsNullOrEmpty` guard. An empty allow rule must not broaden a host
constraint and an empty block must not hide legitimate matches. This shared
boundary applies to Bridge/RPC and both auto-type scans. Do not trim meaningful
whitespace patterns or discard an explicit wildcard. Other regex dialect and
resource-limit behavior remain separate evidence gaps.

Bitwarden source password-history archives are always protected custom fields,
including values for empty prior passwords. Their plaintext exists only in the
explicit import/reveal flow; normal vault snapshots remain masked. A source
custom label cannot overwrite the archive, and parser errors contain only the
record index. Password-only source history is not a complete native snapshot.
