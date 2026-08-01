# Security Model

KeyVault stores credentials in standard KDBX 4.0 files. This reference documents the security invariants the frontend and backend must preserve.

## Core invariants

- **Local-only**: the vault master password and the derived key never leave the machine. There is no sync, telemetry, or upload.
- **In-memory session**: the decrypted database and the master password live only in backend managed state while a vault is open. `close_vault` and lock paths clear the session and zeroize the password buffer.
- **No persistence of secrets**: config (`config.json`) stores only preferences, never passwords or vault content. The browser demo fallback stores fake demo data only.
- **Clipboard hygiene**: copying a password schedules an automatic clear after `clipboardClearSeconds` (0 disables). The timer is reset on each copy and skipped when the user copies other content afterward. Lock clears the clipboard when `clearOnLock` is enabled.
- **No secret logging**: password and vault content must never be written to logs, debug output, or error strings.
- **Passwords never reach the frontend**: `VaultState`/`VaultEntry` carry no entry passwords in the Tauri runtime. Reveal/copy, editor prefill, CSV export, and the security report resolve secrets server-side (`get_entry_password`, `export_csv`, `security_report`) so plaintext crosses the IPC only on explicit user action. The browser demo fallback keeps fake passwords in `VaultEntry.password` for its local simulation only.

## Lock lifecycle

Lock happens on:

- explicit lock action (`close_vault`);
- idle timeout when `autoLockMinutes > 0` (timer refreshed on `pointerdown`/`keydown`/`mousemove`/`wheel`/`scroll`, re-armed at most once per 15 s to avoid churn, reset on vault open, skipped when 0 or no vault open);
- `lockAfterAction` immediately after a password copy;
- focus loss when `lockOnFocusLoss` is enabled (installed from `+layout.svelte` via `installFocusLock`; locks only while a vault is open).

The frontend lock path (`lockVault` in `src/lib/services/security.ts`) zeroizes the session by calling `vault.close()`, and clears the clipboard first when `clearOnLock` is enabled. Password copies go through `copySensitive` so `lockAfterAction` applies only to the password, not usernames/URLs/notes.

After lock, the frontend `vault` store is cleared and the UI shows the lock screen (`LockScreen.svelte`) when a last-database path is remembered (`rememberLastDatabase`); otherwise it returns to the welcome screen. The remembered path (`vault.remembered`) is set on open/create and cleared only via "use another database"; the password is never persisted or remembered.

## KeePass strengths

The `keepass` crate handles KDF (Argon2id/Argon2/AES-KDF), cipher (AES-256/ChaCha20), and compression. Defaults favor Argon2id + AES-256 + Gzip. Customizing these only affects newly created databases.

## Password generation

`src/lib/utils/password.ts` generates passwords from the configured charsets. It uses `crypto.getRandomValues`. Entropy is estimated as `length × log2(poolSize)`; strength buckets are `<40` weak, `<72` fair, else strong. Similar/ambiguous exclusion filters `Il1O0` and `{}[]()/\\'"`~,;:.<>` respectively.

## Threat notes / accepted trade-offs

- Keyfile unlock is supported as a second factor: the keyfile path is user-picked, its bytes are read and kept in the session for save, and cleared on close. Keyfile contents never enter config, logs, or the frontend. Windows Hello and hardware-token (challenge-response) unlock remain on the roadmap.
- CSV export writes plaintext credentials (including passwords and TOTP seeds) to a user-chosen file via the save dialog and the `export_csv` command; the user must keep that file as secure as the vault itself. The security report analyzes empty/weak/duplicate passwords entirely server-side — only counts, uuids, and entropy bits cross the IPC.
- Auto-type replays entry fields as keystrokes via `enigo`; fields are resolved server-side from the vault session (the frontend never sends the password in the `auto_type` payload), a 300 ms grace period lets the user focus the target window, and execution runs on a background thread. The sequence itself may still be observed by other applications on the same machine — an accepted trade-off of the feature.
- No TOTP compute or clipboard watcher yet (roadmap).
- The WebView clipboard write relies on platform clipboard; scheduled clearing uses a timer and cannot guarantee clearing if the process exits before the timer fires.

## Verification for security changes

- Targeted tests for lock/clear/session semantics (Rust).
- Confirm no password reaches `config.json`, logs, or the `VaultState`/`VaultEntry` payload by construction and test.
- Browser fallback changes are UI-only; do not claim desktop security behavior from them.
