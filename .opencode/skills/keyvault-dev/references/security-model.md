# Security Model

KeyVault stores credentials in standard KDBX 4.0 files. This reference documents the security invariants the frontend and backend must preserve.

## Core invariants

- **Local-only**: the vault master password and the derived key never leave the machine. There is no sync, telemetry, or upload.
- **In-memory session**: the decrypted database and the master password live only in backend managed state while a vault is open. `close_vault` and lock paths clear the session and zeroize the password buffer.
- **No persistence of secrets**: config (`config.json`) stores only preferences, never passwords or vault content. The browser demo fallback stores fake demo data only.
- **Clipboard hygiene**: copying a password schedules an automatic clear after `clipboardClearSeconds` (0 disables). The timer is reset on each copy and skipped when the user copies other content afterward. Lock clears the clipboard when `clearOnLock` is enabled.
- **No secret logging**: password and vault content must never be written to logs, debug output, or error strings.

## Lock lifecycle

Lock happens on:

- explicit lock action (`close_vault`);
- idle timeout when `autoLockMinutes > 0` (not yet implemented; see `TODO.md`);
- `lockAfterAction` immediately after a password copy (config field reserved; not yet wired).

After lock, the frontend `vault` store is cleared and the UI returns to the welcome screen. The optional last-database path (`rememberLastDatabase`) may persist the path but never the password.

## KeePass strengths

The `keepass` crate handles KDF (Argon2id/Argon2/AES-KDF), cipher (AES-256/ChaCha20), and compression. Defaults favor Argon2id + AES-256 + Gzip. Customizing these only affects newly created databases.

## Password generation

`src/lib/utils/password.ts` generates passwords from the configured charsets. It uses `crypto.getRandomValues`. Entropy is estimated as `length × log2(poolSize)`; strength buckets are `<40` weak, `<72` fair, else strong. Similar/ambiguous exclusion filters `Il1O0` and `{}[]()/\\'"`~,;:.<>` respectively.

## Threat notes / accepted trade-offs

- No keyfile, Windows Hello, or hardware-token unlock yet (roadmap).
- No auto-type, TOTP compute, or clipboard watcher yet (roadmap).
- The WebView clipboard write relies on platform clipboard; scheduled clearing uses a timer and cannot guarantee clearing if the process exits before the timer fires.

## Verification for security changes

- Targeted tests for lock/clear/session semantics (Rust).
- Confirm no password reaches `config.json` or logs by construction and test.
- Browser fallback changes are UI-only; do not claim desktop security behavior from them.
