# Pitfalls

Recurring traps discovered while developing KeyVault. Read before touching the relevant subsystem.

## Frontend

- **Do not persist passwords in config or localStorage.** The browser fallback stores only demo data (`demo://vault.kdbx`). Never store a real vault password outside the backend session.
- **`.settings-scroll` relies on flex-shrink.** It scrolls only because its parent `.settings-content` is a fixed-height flex column and the scroll div has `min-height: 0; overflow: auto`. Do not add `height: 100%` or wrap it in an unconstrained parent.
- **Theme updates must go through `applySettingsToDocument`.** Changing `appSettings.general.themeColors` does not re-theme the page until bootstrap applies it. Panels that edit theme colors should subscribe to the store and reapply.
- **Nested store state must be replaced, not mutated.** `appSettings.updateGeneral("fontSizes", {...})` with a new object. Mutating `s.general.fontSizes.base` in place is invisible to persistence.
- **Number inputs in settings are textfields with hidden spin buttons** (`.settings-input`). Reuse that class; do not hand-roll another appearance reset.
- **`--slider-pct` must be initialized on first render.** Derive it from value/min/max in the template so the filled track is correct before any input event.

## Backend / vault

- **KDBX passwords never enter `config.json` or error strings.** Keep key material inside the session state; error messages must be generic on auth failure.
- **Keyfile bytes live in the session, never the frontend or config.** `open_vault`/`create_vault` take a user-picked path; the backend reads and holds the raw bytes for save and clears them on `close_vault`. A passwordless vault is valid only with a keyfile (KeePass-compatible).
- **The root group is virtual.** Frontend `VaultState.root` maps to the DB's root group; keep `children`/`entries` split consistent or save will be wrong.
- **UUIDs must be stable across the session.** Render KDBX uuid bytes as hex and reuse them as the frontend `uuid`; regenerating on each mutation breaks selection/editing.
- **Save must re-open with the stored key, not re-derive from a lost password.** Hold `DatabaseKey` or the password (and keyfile bytes) in the session for the duration; zeroize on close.
- **Browser demo behavior is not desktop evidence.** Do not check desktop KDBX TODOs from `npm run dev` screenshots or localStorage behavior.

## Settings / style

- **Never re-define shared primitives in a panel.** Before adding CSS, grep `settings-shared.css` and `SettingsDialog.svelte` for the same primitive.
- **Do not use a parent scoped selector to style a child component.** Pass props/classes or move the rule into the shared stylesheet.
- **Segmented controls (theme/KDF/charset)** follow one pattern: bordered chip, `--settings-control-radius`, active = selection tint. Do not introduce a second segmented style.
- **Keep the sidebar width fixed (168px).** Narrow-window behavior hides the sidebar below ~560px; preserve the single-column fallback.
