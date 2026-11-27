# Settings Shell and Panel Patterns

BitwardenImportSettingsDialog collects source-account URI choices per import,
not app preferences. It uses a medium scrollable ModalShell with shared Select
for all six defaults, four-row TextField for comma-separated domain groups and
inline Feedback on invalid input. It explicitly explains export omissions,
per-row precedence and blank-equivalent behavior. Cancel/confirm settle the
origin-bound page promise; supersession/tab/epoch/unmount also cancel it.

AppearanceSettingsPanel's display section adds quickFindExcludeExpired after
focusSearchOnType. It reuses search-icon SettingToggleCard and updateGeneral,
default off. Its text distinguishes toolbar quick search from ordinary browsing
and advanced filters. A composed real-panel/real-filter preview confirms live
result changes, blank browsing and advanced expired-only results in both themes;
320px wrapping retains reachable controls and the shared font without overflow.

GeneralSettingsPanel adds autoReloadChangedFiles with the existing refresh-icon
SettingToggleCard and updateGeneral. Its default-off text identifies visible/focus
30s checks, in-memory dirty merge, preserved conflicts, dialog/inline/drag pauses
and remote-download cost. It adds no scoped CSS or control primitive. Both themes
and320px checks retain shared typography and contained wrapped descriptions.
FileReloadMonitor adds no visible shell; transient statuses use existing tips.

DatabaseSettingsPanel starts with LocalBackupSettingsCard, composing shared
SettingToggleCard/TextField/Button/Feedback. The default-off toggle applies
immediately; count1–50 and filename-template drafts require native validation,
explicit Apply and flush. Busy/mobile/browser controls cannot apply drafts;
disposed generations discard replies. Folder/old-key/platform notes occupy the
full card width, keeping the toggle description compact in both locales. Count
uses a96px shared numeric control; the template/action note wraps at320px.
Neutral-white light/dark previews retain the shared Cascadia font and contained
controls; no new palette, surface, font or primitive is introduced.

GeneralSettingsPanel composes LoggingSettingsCard over shared SettingToggleCard
and inline Feedback. It identifies the app-data Logs location, typed/redacted
contents,1MiB cap and retained older files; native metadata shows the current path
or unavailable/full feedback. Preference changes flush before inspection; mounted
enabled cards poll every5s without repersisting. Generations discard stale replies
and disposal clears the timer. Browser fallback disables the native control.
The path owns only compact spacing and semantic secondary text/anywhere wrapping.

The two keyfile-search preferences use existing SettingToggleCard on the desktop
Security page. Both map to IntegrationSettings; removable is disabled while main
search is off. Descriptions identify Windows drive-root scope and explicit choice.
VaultCredentialFields composes KeyfileSuggestionList only for opening scopes;
the list reuses Select and semantic secondary notes in a six-pixel compact column.
Search results never auto-select or submit. Mobile keeps manual browsing and no
search controls. No new surface/font/control primitive is introduced.
Shared Select owns its body listbox portal and no-scroll initial focus; its
existing aria-controls/keyboard/outside-click behavior remains the menu boundary.
Light/dark/320px candidates and settings render without horizontal overflow;
explicit selection fills the readonly keyfile field and Escape closes only the list.

SecuritySettingsPanel composes recommended/required MasterKeyExpiryRuleCard
instances. Each reuses card/heading, control TextField, action Button and inline
Feedback; local drafts require native validation and explicit Apply. Empty rules
disable; XML duration/date cutoffs apply next opening. Drafts never persist on
invalid or disposed replies. Compact note/action flex rows wrap text at320px.
MasterKeyChangeForm extracts the existing repetition/keyfile/weak-KDF flow and
scoped layout, shared by the panel and MasterKeyExpiryDialog. Its submit uses
shared primary Button for manual changes and pale-yellow warning for renewal.
The dialog uses confirm/small ModalShell at prompt layer; required review offers
Change/Close, recommended also Later, and readonly disables Change. Back never
acknowledges a requirement. Main KdfOpenPrompt resolves renewal before KDF.
Chinese light/English dark and320px previews cover wrapping/controls/shared font;
no real credential change or native vault runs in that browser preview.

SecuritySettingsPanel's Master key attempts card uses the existing setting-card,
setting-heading and heading-inline structure plus a 96px shared TextField at
control size. Positive-int32 min/max/step are passed through; valid edits update
masterKeyTries, default3. No new palette/font/control styling is introduced.
The preference applies to new opening prompts. VaultWelcome shows exhaustion
through shared inline Feedback after ending its credential modal; LockScreen
replaces the fields with Feedback and a shared Try again Button. Retry resets
the prompt and keeps the database available, matching KeePass's loop semantics.

KdfOpenPrompt uses keyTransformWeakWarning for open/unlock/replacement lifetime
inspection, with a separate KdfUpgradeDialog over the shared ModalShell/Button
primitives. It displays the filename, exact current/recommended costs and
read-only note; Later keeps the opened vault, while Use recommended stages an
unsaved native change. In-flight upgrades disable actions/Escape. Exact MiB
conversion and algorithm/version/lanes are visible; long names/costs wrap inside
the dialog. A declined recommendation does not repeat on an ordinary tab revisit.

SecuritySettingsPanel adds the default-on keyTransformWeakWarning SettingToggleCard
and shares KdfWarningDialog with DatabaseSettingsDialog. The prompt composes
ModalShell at the existing prompt layer and compact Button actions: Cancel
preserves the draft, and the warning variant explicitly keeps weak parameters.
Escape cancels only the prompt. Parameter approval is exact and one-use; a
changed draft/cost requires fresh review. Master-key view changes dismiss the
prompt and invalidate its approval. Chinese/English strings describe the
official cost threshold and stronger new-vault defaults. Only the preference
is persisted; no password or key is rendered in the warning.

SecuritySettingsPanel exposes `autoSaveOnClose` after autoSaveAfterEdit using
the canonical SettingToggleCard and save icon. Its default-off description
explains close/lock/exit saving, keeping sessions on failure and the disabled
setting's Save/Discard/Cancel choice. Both locales use shared card typography,
overflow and neutral light surfaces; rendered light/dark desktop and 320px
checks retain the approved root font without horizontal overflow. Native save
questions use the platform dialog plugin, not a second renderer modal shell.

SecuritySettingsPanel exposes the default-on repeatPasswordOnlyWhenHidden toggle
and composes MasterPasswordFields with 30px shared controls for master-key change.
The same component owns welcome local/remote create and open password fields
(32px input variant). It labels both inputs with instance-scoped IDs, uses shared
Button for the Show/Hide action, changes both input types together and disables
inputs/actions while busy. Automatic repetition leaves the disabled field visible
with a localized hint, avoiding a moving form. Only unique field/row layout belongs
to this component. The current preference is checked by all create/change handlers.
Rendered dark/light/narrow checks cover the inputs and settings shell.

DatabaseSettingsDialog shows exact imported AES rounds or Argon2 iterations,
memory MiB and parallelism, retaining Argon2 v1.0/v1.3. `kdf-parameters.ts` uses
BigInt for lossless u64/byte conversions. Costs use shared TextField controls in
a compact responsive grid; benchmark target and wrapped field Button actions
reuse existing theme/size tokens. A 100–5000 ms target runs credential-free native
calibration and presents the measured cost; Apply suggestion changes only draft
inputs, while Save persists. Cost inputs and Save are disabled while measuring;
Cancel/Escape may close and invalidate the reply without cancelling an active
native hash. Changed inputs hide stale suggestions. Both locales show limits
and a non-exact-target note. Browser preview does not implement native timing.

The database settings modal uses `RadioGroup` for KDF, cipher and compression.
Its compact buttons expose checked radio state and one Tab entry per group;
arrows/Home/End synchronize selection/focus. Keep Twofish is a renderer-only
choice mapped to the existing null cipher, so it never requests re-encryption.
The original session/view guards remain; save patches additionally carry exact
KDF costs when changed.

Advanced search field scope and group Auto-Type enable/inherit choices compose
the same RadioGroup in its 28px compact size. Search uses content-width choices;
group choices fill the row. Each exposes checked radio state and one Tab entry,
with arrows/Home/End updating bound values and focus. Existing search save/apply,
group inherit omission, boolean mapping and guarded save logic remain unchanged.

Read this file and `css-theming.md` before changing settings markup or CSS.

Light mode uses the approved white/gray glass-inspired appearance through the
existing semantic tokens: translucent neutral-white cards, gray selected navigation/segments
and the shared root Cascadia Code / SFMono-Regular UI font stack. SettingsDialog owns its light-only shell/sidebar gradients;
settings-shared.css owns card highlights, 14px backdrop blur and theme segment decoration. Compact
spacing/control sizes stay unchanged. Dark/custom modes retain their existing
styles; this does not enable native Windows material or change opacity settings.

ModalShell follows the same neutral-white treatment with 90%/82% translucent gradient
stops and 18px surface blur in light mode only, outside forced colors.
Shared primary actions use pale blue, destructive actions pale red, and the
warning variant pale lemon yellow with a yellow border, including hover; all
retain compact sizes and legible dark text. Light warning decoration derives
the warning token's yellow hue at fixed high saturation/lightness, rather than
mixing a dark amber/brown token into white.

## Navigation and ownership

`SettingsDialog.svelte` is the parent shell for the settings page. Its primary categories:

| Primary category | Secondary sections / implementation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| General          | General (UI language, remember-last-database) / Network (favicon download) → `GeneralSettingsPanel` with `section` prop.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Appearance       | Theme / Display / Window / Layout / Toolbar → `AppearanceSettingsPanel` with `section` prop. The Toolbar tab is the dedicated secondary group for full toolbar收纳: it lists all 18 toolbar buttons (`newEntry/save/saveAs/lock` + right group + `moreMenu` + `windowMinimize/Maximize/Close`), each toggle controls whether the item shows directly on the main toolbar/window chrome (`true`) or is collected into the More menu / hidden (`false`, `moreMenu` always visible), plus global ordering (`toolbarFullOrder`), side assignment (`toolbarSides` left/right) and dividers (`toolbarFullSeparators`), and the detail auto-show toggle (`showDetailOnSelect`). Migration from the legacy `toolbarOverflowMenu` derives the initial `toolbarItems` values; empty `toolbarFullOrder` restores default full order. |
| Security         | Single current section → `SecuritySettingsPanel` (idle lock, clipboard clear, KeePass `defaultExpireDays` for new entries, lock/tray/remember-password toggles, master-key change)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Database         | Single current section → `DatabaseSettingsPanel`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Remote           | S3 / WebDAV tabs → `RemoteSettingsPanel` with a fixed `kind` prop. Each tab lists only that transport's profiles; every profile has canonical path `<kind>/<name>` and stores only that protocol's fields. WebDAV omits region/bucket and labels credentials as username/password. Credential fields never echo stored values (IPC masked): a stored secret renders as a "saved — leave blank to keep" placeholder, typing replaces it.                                                                                                                                                                                                                                                                                                                                                                                   |
| Integrations     | KeePassHttp / KeePassRPC / URL Overrides tabs → `BridgeSettingsPanel` / `RpcSettingsPanel` / `UrlOverridesSettingsPanel`. The URL Overrides tab mirrors KeePass `Tools → Options → Integration → URL Overrides`: global-template switch `urlOverrideEnabled` + single `urlOverride` input, 27 code-defined built-in rules toggled by the `builtInOverridesEnabled` bitmask (fetched via `list_builtin_url_overrides`, all off by default), and user-defined `customOverrides` rows (enabled/scheme/urlOverride). Templates expand first, then built-ins are checked before customs; the first matching scheme wins.                                                                                                                                                                                                       |
| About            | Single current section → `AboutSettingsPanel`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |

URL Overrides adds a separate shared 30px monospace TextField for the single
global `urlOverride` template. `urlOverrideEnabled` controls only this template;
entry overrides and enabled scheme rows remain active. The panel uses shared
setting-card/setting-heading/icon styles, including light-mode decoration.
Below 480px custom scheme/template inputs stack inside scoped layout wrappers;
the parent never styles child control internals. Template edits publish immediately
and preserve spacing through frontend/native persistence even when disabled.

The left sidebar retains primary categories. The right content pane owns breadcrumb, secondary row, description, and the selected panel.

The Appearance → Theme tab owns theme mode (dark/light/custom segmented control). The General → General tab owns the UI language segmented control (中文/English, `general.language`, applies live through the `src/lib/i18n/` dictionaries). In custom mode the single `自定义配色` card owns both the live palette and the multi-theme actions — there is no separate `多主题配置` card: the card header merges the theme `Select` (当前配色 vs. a saved `customThemes` entry — edits write `themeColors` live or the selected theme's stored copy), `另存为` (ModalShell prompt, default `自定义 N`, deduped, `添加`), `修改名称` (rename prompt), `应用` (copy into `themeColors` + force `custom`) and `删除`, together with the `深色预设`/`浅色预设`/`恢复默认` preset fills (which fill whichever palette the selector points at). All naming goes through the dialogs; no inline name field is rendered. Dark/light modes show a read-only palette card instead. Switching the mode control to dark/light never mutates the persisted `themeColors`: `selectThemeColors` serves the built-in presets directly for non-custom modes, so returning to custom restores the last custom palette (live edits or the last `应用`d theme) rather than the mode preset.

The Appearance → Layout section owns the always-on density controls: the entry-table row height (`entryRowHeight`, applied via the `--entry-row-height` variable so CSS matches the setting; narrow rows floor at 36px) plus group-tree spacing (gap, padding, indent, radius). Subgroup separator lines are split across two homes: the Appearance → Display toggle (`showGroupSeparators`, default on) and the Appearance → Theme color card (`groupSeparatorColor` hex, empty = theme muted text; the hex field only persists empty/valid values and warns otherwise). The Appearance → Display section also owns the type-to-search toggle `focusSearchOnType` (default on): typing a printable character that matches no app shortcut in the main window focuses the search box and seeds that character. There is no compact-mode toggle — the layout settings apply directly. The Appearance → Toolbar secondary group owns full toolbar customization: global ordering (`toolbarFullOrder` 18 ids including `newEntry/save/saveAs/lock` + right group + `moreMenu` + `windowMinimize/Maximize/Close`, drag/Up/Down reorders), per-item dividers (`toolbarFullSeparators` → `toolbar-divider` after the item within same side), per-button side assignment (`toolbarSides` `left`/`right`), per-item visibility (`toolbarItems` 18 toggles, `moreMenu` always visible and sortable, window buttons hide when false) and the detail auto-show toggle (`showDetailOnSelect` `eye/eye-off`, click entry auto-shows detail). Legacy `toolbarOrder/Separators` (right-only 10) remain for migration and are superseded by the full set. The Appearance panel owns full-toolbar customization收纳: per-item `toolbarItems` toggles (saveAs, toggleDetail, securityReport, similarPasswords, hibpCheck, importMenu, exportMenu, expiredEntries, clearHistory, dbSettings, appSettings plus windowMinimize/windowMaximize/windowClose — `— □ ×`). Each toggle controls whether that action shows directly on the main toolbar/window chrome (`true`) or is collected into the More menu / hidden for window buttons (`false`). `toolbarItems` defaults follow the legacy `toolbarOverflowMenu` platform default (desktop: saveAs/toggleDetail/securityReport/exportMenu/appSettings `true`, others `false`; mobile all toolbar actions `false`; window buttons all `true`). Old configs without `toolbarItems` derive initial values from the legacy `toolbarOverflowMenu` flag during normalization.

The Remote section uses the same secondary-tab shell as General/Integrations:
S3 and WebDAV are separate namespaces, names only need to be unique inside the
current protocol, and entering a tab selects one profile from that protocol.
The panel displays the canonical path (`s3/config_1`, `webdav/config_1`) and the
matching local mirror path under `Storage/remote/<kind>/<name>`.

`renameRemoteProfile` validates profile existence, normalized nonempty name,
unchanged identity and same-protocol collisions before any side effect. Rejected
or unchanged names retain credential-presence hints and do not warn or persist.
A successful identity change keeps the existing credential-reset warning.

`removeRemoteProfile` validates existence and the remaining same-protocol profile
count before clearing credential-presence hints or scheduling persistence. Last,
missing and repeated removals are complete no-ops at the service boundary; the
panel also disables deleting the final profile. A successful active-profile
removal selects a remaining profile of that protocol and retains other hints.

At narrow widths (`max-width: 720px`), the primary sidebar becomes an off-canvas
drawer. The section header exposes the menu toggle, the content pane uses the
full viewport width, and selecting a primary category or pressing the backdrop
closes the drawer. Keep the secondary row horizontally scrollable instead of
wrapping labels character-by-character.

Theme-mode and language options use the shared wrapping `.theme-segmented` group.
Labels remain single-line within 30px buttons; complete options move to another
row when larger configured fonts need more space. Theme swatches retain 12px.

The `/settings` route wrapper (`.settings-shell`) owns `100dvh` and
`env(safe-area-inset-*)` padding, while `SettingsDialog` fills its content box;
do not put a second `100dvh` on the dialog or it will overflow the reserved
system-bar space on Android.

## Approved shell hierarchy

```text
breadcrumb + item count + close button
secondary row (tabs, or one current-section label)
one small description line
scrolling setting cards / panel-specific board
feedback overlay when needed
```

The breadcrumb and description use `--settings-description-size`; secondary section labels and card headings use `--settings-heading-size`. Do not add a second page title inside a child when the parent owns the header.

## Child panel contract

```svelte
<script lang="ts">
  interface Props {
    onclose: () => void;
    showHeader?: boolean;
  }

  let { onclose, showHeader = true }: Props = $props();
</script>

{#if showHeader}
  <header>...</header>
{/if}

<div class="settings-scroll">...</div>
```

Render every child from `SettingsDialog` with `showHeader={false}`. Conditional removal must happen in the child.

## Settings state pattern

```typescript
let s = $state($appSettings);

$effect(() => {
  const unsubscribe = appSettings.subscribe((value) => {
    s = value;
  });
  return unsubscribe;
});

function change<K extends keyof GeneralSettings>(key: K, value: GeneralSettings[K]) {
  appSettings.updateGeneral(key, value);
}
```

Do not mutate nested store state in place and assume persistence will notice. Create a new nested object/array and pass it through `updateGeneral`/`updateSecurity`/`updateDatabase`/`merge`.

Use `appSettings.flush()` before a close/restart boundary when the UI must guarantee that the latest debounced write reached the backend.

## Canonical card patterns

### Toggle/control card

Use `SettingToggleCard.svelte` for a standard switch card so the icon, text,
ARIA switch state, and shared class hierarchy stay identical across panels.

```svelte
<SettingToggleCard
  icon="..."
  {label}
  {description}
  checked={enabled}
  onchange={(checked) => update(checked)}
/>
```

A select, segmented control, number input, shortcut binding, or compact action
group may still use explicit card markup while preserving the same left/right
hierarchy. Do not force custom right-side controls through the switch template.

Use `templates/action/Button.svelte` with `variant="action"` for compact
secondary actions shared across panels (26px), or `variant="field"` when
aligning with a 30px settings input. The template replaces the retired
`.settings-action-button` classes and owns the button chrome. Keep local
layout in the parent rather than targeting the template's scoped classes.

### Slider card

Use `SettingRangeCard.svelte` for a standard numeric range. It owns the range
markup and derives the clamped `--slider-pct` value from `value` / `min` / `max`.

```svelte
<SettingRangeCard
  icon="..."
  {label}
  {description}
  {value}
  valueLabel={`${value}px`}
  {min}
  {max}
  onchange={(next) => update(next)}
/>
```

Do not add another slider-percentage helper in a child panel or wrap the range
input merely for styling. Extend the shared component only when a recurring
standard range-card capability is genuinely missing.

## Feedback and asynchronous state

- Use `templates/form/Feedback.svelte` with `message` and optional `success`; default styling is error. Set `inline` when the message belongs in the card flow instead of the panel overlay. The template replaces the retired `.settings-feedback` classes.
- Clear feedback with a timer whose cleanup is retained when the component can unmount or messages can be replaced.
- Disable controls or show a saving/loading state during commands that cannot safely overlap.
- Keep restart-required state explicit for path/config changes that do not apply live.

## Shared versus panel-specific CSS

Shared base classes belong in `settings-shared.css`. Panel-specific examples include theme color swatches, KDF segmented controls, charset chips, and the about grid.

Before adding CSS:

1. search `settings-shared.css`, the parent shell, and sibling panels for the same primitive;
2. use existing `--settings-*` and theme variables;
3. avoid redefining header/card/toggle/slider/feedback rules in a child;
4. test standalone and parent-composed rendering when `showHeader` or sizing is affected.

## Form details

- Remote text/password fields pass `TextField.ariaLabel` from the same localized
  heading keys, including protocol-specific endpoint and credential names. Keep
  these names stable when placeholders switch to the saved-credential hint.
- Hide number spin buttons when the control is visually a plain value field.
- Keep tabular numeric labels stable and non-shrinking.
- Let long labels/paths/numbers shrink, wrap, or ellipsize without overflowing the card.
- Add `.settings-scroll--stack-rows` to a panel whose explicit `.setting-row`
  controls cannot remain readable side-by-side below 720px. Use
  `.setting-row-input` on both inputs and shared `Select` roots so the modifier
  can expand the right-side control to the full card width.
- Use semantic status colors through variables and `color-mix`.

## New or changed setting checklist

1. Update the relevant TypeScript type, defaults, and normalization in `services/settings.ts`.
2. Update Rust `config.rs` serde/defaults when backend behavior should be typed.
3. Add/update panel UI using the approved shell and shared primitives.
4. Apply live document behavior or clearly mark restart-required behavior.
5. Update `data-contracts.md` and `css-theming.md`/`settings-panels.md` when applicable.
6. Run static/build checks plus rendered dark/light/custom and narrow-window verification when visual.
