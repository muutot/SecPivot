# Project-wide UI Style and Theming

ShowEntriesOfSubGroups composes the existing folder-icon SettingToggleCard.
The More > View cascade composes existing ContextMenu/MenuItem checkbox chrome
and check/folder icons; false exposes an unchecked menuitemcheckbox too. No new
surface, CSS rule, font, palette, metric or theme token is added. Directory
separator and entry-row heights retain their existing half/full-height contract.
Five isolated desktop/320px Chinese-light/English-dark/custom browser checks
confirm contained settings/cascade widths, identical Cascadia stacks, mouse/
keyboard toggles and Escape focus return. Native WebView remains separate.

EntryEditorDialog linked fields reuse the existing custom row, field-hint,
link icon and shared field Button. A scoped flex/wrap hint row owns only6px
gaps/8px bottom spacing/anywhere wrapping below the field controls. No new
font, palette, metric token, surface or inline icon crowding is introduced.
EntryDetail reuses protected-badge chrome; history reuses the wrapped Metadata
policy summary, change badge and value-swap button. Chinese light/English dark
desktop/320px isolated browser checks retain identical Cascadia stacks, neutral
white-gray glass, contained hints and working mouse/Enter unlink operations.
History exposes same-value unlink counts and current/history switching. These
synthetic previews do not prove desktop WebView/provider behavior.

BitwardenImportSettingsDialog composes the existing medium/scrollable ModalShell,
Select, TextField, Feedback and Button. Its scoped layout uses compact10px/6px
gaps, semantic secondary/muted text and anywhere wrapping. WebsiteRulesPanel
and HistoryVersionDialog reuse existing hints for Bitwarden source semantics.
No font, palette, surface or theme token is added; controls retain shared chrome.
Chinese light/English dark desktop and320px previews confirm identical Cascadia
stacks,90px four-row source input, mouse/keyboard selection, invalid feedback,
multiple-group confirmation and cancellation. The short English Import action
keeps both footer buttons contained. History retains its source profile and
shared scrolling; keyboard focus brings Close into view on narrow displays.
These isolated synthetic previews do not prove native WebView/provider behavior.

WebsiteRulesPanel's v2 Prefix/JavaScript Regex choices reuse shared Select and
TextField controls. Regex shows an expression placeholder; the existing hint
explains case-sensitive literal prefixes and case-insensitive JS expressions.
History uses its existing read-only rule rows and new localized mode labels.
No CSS, font, surface, metric or theme token is added. Chinese light/English dark
desktop/320px isolated renders retain white-gray glass and the identical Cascadia
root stack without horizontal overflow; mouse/keyboard Select and history View
activation work. These synthetic previews do not establish native WebView behavior.

quickFindExcludeExpired uses the existing SettingToggleCard/search icon in the
display panel. It adds no CSS, palette, font or primitive; Chinese light/English
dark desktop/320px previews retain neutral glass surfaces, the identical Cascadia
root stack and contained descriptions/controls. The filter has no visual chrome.

The auto-reload setting composes the existing SettingToggleCard and refresh icon.
It introduces no CSS, color, surface or font token. FileReloadMonitor has no visible
markup. Main-page inert/key boundaries preserve layout and existing controls;
mounted inline-activity predicates add no styling. Isolated Chinese light/English
dark desktop/320px checks retain neutral white-gray surfaces, identical Cascadia
font stacks and contained descriptions. Native WebView/provider remains separate.

LocalBackupSettingsCard reuses shared settings cards, controls and feedback.
Its scoped CSS owns only count/action flex spacing, a96px numeric wrapper,
eight-pixel full-width note spacing and anywhere note wrapping. Chinese light/
English dark desktop and320px previews retain the Cascadia stack, neutral glass
surfaces and contained actions/errors. No theme token or control chrome is added.

LoggingSettingsCard reuses SettingToggleCard and Feedback. Its optional metadata
path owns compact spacing, text-secondary, secondary font size and anywhere
wrapping; no new font, control chrome, surface or theme token is introduced.
Chinese light and English dark desktop/320px previews cover toggle/path,
unavailable/full feedback and contained width with the same Cascadia root stack.

Keyfile search uses shared SettingToggleCard and Select. Its local list wrapper
is a min-width:0 flex column with6px top spacing; status notes use muted/secondary
tokens and anywhere wrapping. Candidate paths retain the existing Select menu
chrome/overflow behavior. No theme token, background or font is added.
Select portals the listbox to document.body so blurred/scrollable modals cannot
clip its fixed viewport position. The action removes the list on unmount; initial
list focus uses preventScroll to avoid an outside-scroll dismissal. Chinese light/
English dark and320px renders cover mouse/keyboard selection, Escape, full path
display and contained width with the shared Cascadia font.

MasterKeyExpiryRuleCard uses existing settings cards, TextField, Button and
Feedback, with a scoped note/action flex row and anywhere text wrapping.
MasterKeyChangeForm retains existing field/keyfile spacing and uses a stretched
shared submit Button: primary in settings, warning for opening renewal.
MasterKeyExpiryDialog uses confirm/small ModalShell at prompt layer with only
compact semantic filename/note spacing and wrapping. No new palette, font or
theme token is introduced. Light rendering retains translucent white-gray
surfaces, shared Cascadia font and pale lemon-yellow warning buttons; dark
desktop and320px Chinese/English checks show contained text and reachable actions.

The Master key attempts card composes existing settings primitives and a shared
control-size numeric TextField; its only local layout rule reserves96px for the
number. TextField's optional min/max/step attributes add no CSS. Retry exhaustion
uses existing Feedback/Button layouts in VaultWelcome/LockScreen. Both themes
retain the shared Cascadia font and neutral translucent light surfaces.

KdfUpgradeDialog uses the existing confirm-size ModalShell at the prompt layer,
plain Later and warning Use recommended Buttons. Filename/cost paragraphs own
only compact margins, secondary text size, semantic text colors and anywhere
wrapping; no new font, palette, shell or footer rule is introduced. Light mode
retains neutral white-gray translucency and pale lemon-yellow warning actions.
The main-route KdfOpenPrompt host adds no CSS and is excluded from TCATO.

KdfWarningDialog reuses ModalShell's confirm size/prompt layer and Button's
existing warning variant (pale lemon yellow in light mode). Its threshold note
owns only zero margin and secondary font/color tokens. The security preference
uses SettingToggleCard; no new token, font, surface or control chrome is added.
An isolated component preview covers Chinese light/English dark and 320px
rendering without operating a vault or submitting a credential change.

The autoSaveOnClose setting reuses SettingToggleCard with the save icon and
existing compact card spacing. It adds no CSS, colors or fonts. Chinese light
and English dark desktop/320px rendering retain white-gray light surfaces,
the shared Cascadia Code root stack and contained wrapped descriptions.

MasterPasswordFields reuses TextField (32px welcome / 30px settings) and field
Button, retaining the six-pixel input/action gap and ten-pixel field spacing.
Its flex input wrapper has min-width:0; labels use existing secondary/muted/faint
tokens. The repeat row stays present when disabled, with a localized hint.
It introduces no surface/font/color token or child-control override. Rendered
light 320px and dark desktop settings and welcome checks show no horizontal
overflow and the same approved root font family.

EntryTable directory separators use half the effective entry height, with a
14px readability floor and line-height 1: desktop defaults are 15px/30px,
narrow defaults 18px/36px. Entry-row density stays unchanged. Prefix row offsets
drive mounted ranges, spacers, scroll clamping and focus; PageUp/PageDown travel
by rendered pixels and skip directory rows. Existing separator colors/borders
and the tiny-font token remain. Mixed-height unit/production-handler tests and
rendered desktop/narrow End/Home checks cover the scrolling contract.

Group rows use their existing compact dimensions for drag/drop. The top/bottom
quarters show a 2px insertion line using selection-color; the middle uses the
existing selection tint for nesting. The All entries row accepts top-level
group drops. Sources/targets are disabled for root, recycle-bin or read-only
operations. No new theme token, control size or background palette is added.

DatabaseSettingsDialog's KDF costs use shared numeric TextField controls in an
auto-fit grid with 100px minimum tracks (bounded by container width); benchmark
and explicit Apply suggestion use field Buttons in a six-pixel wrapped row.
Result text reuses secondary text/font tokens. Target and cost labels retain
the existing compact dialog hierarchy; no new theme token or surface is added.

The entry table supports an optional KeePass-style zebra striping (`alternatingRowColors`, default off): `EntryTable.svelte` shades odd rows with `color-mix(in srgb, var(--card-bg) 55%, transparent)` keyed off the stable virtual-row index (not `nth-child`, which shifts while scrolling); hover/selected states override it, no new color token. The group tree has optional connector lines (`groupTreeLines`, default off): `GroupTree` → `GroupNode` threads the `treeLines` prop and nested nodes use a single `inset 1px 0 0 var(--border-subtle)` box shadow. Both remain within the approved compact neutral language.

DatabaseSettingsDialog composes `templates/form/RadioGroup.svelte` for its three
storage-choice groups. The template owns the existing 30px choice chrome, 6px gap,
selection tint and theme variables. Labels stay on one line; complete choices may
wrap, and below 360px horizontal padding is 6px so default 320px labels fit.
Each group has one Tab stop; arrows/Home/End select and focus, while native
Enter/Space and outer Tab/Escape retain ownership. Global focus-visible remains.
No new color token, modal surface or scrollbar rule is introduced.

AdvancedSearchDialog and GroupAutoTypeDialog use the same RadioGroup with
`size="compact"` for 28px choices. Search supplies `stretch={false}` to keep
natural chip widths; group and database choices retain the default stretching.
Both keep localized labels, theme selection tint and six-pixel wrapping gaps.
Browser keyboard/ARIA and dark/light Chinese/English checks include 320px radios.

AdvancedSearchDialog places its keyword regex/exclusion options 8px below the
TextField, leaving room for both controls' external focus outlines. The scoped
keyword-options wrapper owns this gap; the other toggle rows and shared
TextField chrome retain their existing spacing and compact sizes.

History's explicit View rules action stays in the existing wrapped Metadata
summary. Loaded content spans that row's width; rule rows reuse read-value borders,
compact Button/inline Feedback and existing muted/secondary text metrics. Full URL
bytes render as read-only, pre-wrapped text with anywhere wrapping; mode labels
align at the first line via the scoped .website-rules .website-rule selector.
Source changes and hide clear contents immediately. No new tokens or native links
are introduced. The focusable list scrolls within min(240px,30vh), using existing
scrollbar/focus tokens and contained overscroll; shared modal scrolling keeps Close reachable;
the list's scoped noninteractive-tabindex annotation documents keyboard scrolling
and is backed by the rendered End-key scroll check, without changing its list semantics.
Keyboard activation retains focus on View/Hide and existing swap controls.

This is the authoritative style reference for SecPivot UI. Read it before any markup or CSS change. The visual language is derived from the Clipboard Desktop project and is intentionally kept compatible so settings can share identical primitives.

The user-approved light preset uses neutral white/gray surfaces and gray selection/focus.
All themes share the root Cascadia Code / SFMono-Regular UI font stack; light
must not override font-family. `settings-bootstrap` publishes the exact mode through
`data-theme`; mode-scoped styles never activate for dark/custom. The existing
21 tokens remain mirrored by Rust ThemeColors::light. Password/code fields keep
the existing mono font. Light setting cards use 12px corners, 86%/58% translucent
white gradients, 14px backdrop blur and quiet inset/drop shadows. Modal surfaces
use 16px corners, 90%/82% white gradients, 18px surface blur and a lighter blurred
backdrop. Shared RadioGroup/theme choices use neutral gray with pale text.
Primary Button uses a pale blue derived from linkColor; danger uses pale red,
and warning uses pale lemon yellow for the emergency-export action. Its light
preset warningColor is #807500 for readable semantic text; the button derives
90% saturation / 88% lightness (84% on hover) and a 65% / 68% border from that
yellow hue with relative HSL, avoiding tan fills or brown outlines. Light action text
uses textPrimary, including hover. These tints reuse existing semantic tokens;
the shell, cards, inputs and sidebar never gain blue or brown background fills.
Layouts, card padding and 28/30px controls stay compact. Glass decoration is gated
off under forced colors. This is renderer styling, not Windows desktop acrylic.
Preset neutral text/surface contrast is at least 4.97:1; selected choice text is 5.33:1.
Rendered light primary/danger/warning button text contrast is at least 10.60:1
at rest; hover keeps dark text and the corresponding semantic tint.
Browser inspection also measures composed card/control colors and theme switching.

## Style source-of-truth order

UrlOverridesSettingsPanel uses canonical setting-card/setting-heading/setting-icon
classes and shared 30px TextFields. The global template uses mono; custom inputs
stack below 480px with layout wrappers, preserving shared child-control ownership.
Light card gradients/radii therefore apply without a local surface/token duplicate.

1. `src/routes/+page.svelte` and the settings shell define the product's visual language.
2. `src/lib/types/theme.ts::{DARK_THEME_COLORS,LIGHT_THEME_COLORS}` defines preset values.
3. `src/lib/utils/theme.ts::applyThemeColors` defines the ThemeColors → CSS-variable mapping.
4. `src/app.css` provides root defaults, global reset, font variables, the shared modal backdrop/layer modifiers, focus/accessibility rules, and imports shared settings CSS.
5. `src/lib/styles/settings-shared.css` owns reusable settings-panel primitives.
6. `SettingsDialog.svelte` owns settings-shell layout and panel-specific styles.
7. Component-scoped CSS owns only component-specific layout/visual behavior.

When these disagree, inspect the rendered target and current code, fix the narrow source of divergence, and update this reference if the approved rule changes.

## Approved compact visual language

Preserve these characteristics unless the task explicitly requests a redesign:

- Dense desktop utility layout with one continuous neutral surface, subtle borders, compact spacing, and restrained radii.
- Near-black/dark-neutral surfaces with high-legibility primary text and progressively quieter secondary, muted, and faint text.
- Dark preset: light-green `--accent` for product/focus emphasis and gray `--selection-color`; light preset: neutral-gray accent/selection over white/gray surfaces. Both use blue `--link-color` and semantic success/danger/warning colors.
- Toolbar row (the top chrome, itself the drag region via `data-tauri-drag-region="deep"`, frame-less window) owns left/right action groups (all 18 toolbar buttons `newEntry/save/saveAs/lock` + right group `toggleDetail/securityReport/similarPasswords/hibpCheck/expiredEntries/clearHistory/importMenu/exportMenu/dbSettings/appSettings/moreMenu` + window `windowMinimize/Maximize/Close` are ordered globally via `toolbarFullOrder` 18, assigned per-button sides via `toolbarSides` left/right, and per-item dividers via `toolbarFullSeparators` → `toolbar-divider` within same side; each `toolbarItems` entry (now 15 + 3 window) controls whether that action shows directly on the toolbar or is collected into the More menu / hidden for window buttons, `moreMenu` always visible and sortable; detail toggle uses `eye/eye-off` and is bound to `showDetailOnSelect` (click entry auto-shows detail, `selectionVersion` re-triggers same entry after close, `eye` enabled / `eye-off` disabled), close hover = `--danger-color`). The General → Toolbar secondary group provides the full 18-item sortable list with left/right switches and `|` dividers; old configs without `toolbarItems` derive initial values from the legacy `toolbarOverflowMenu` platform default, empty `toolbarFullOrder` restores default full order. Search is intentionally low-emphasis: this is a password manager, so it must not dominate the header. The vault file name lives in the status bar, not the toolbar.
- The window is frame-less (`decorations: false`); `WindowControls.svelte` provides the shared minimize/maximize/close cluster with two variants: `toolbar` (bordered compact buttons matching `.icon-action`) and `chrome` (flat titlebar-style). The welcome/lock views get a 30px absolute top bar (`standalone-bar`) carrying the chrome variant and acting as the drag region; the settings shell deliberately shows NO window controls (only the ← back button) — window actions live in the main toolbar.
- Three-pane main layout: group tree (left) → entry list (center) → detail (right). The group-tree title bar owns its local tools in the order new subgroup → expand all → collapse all; adding a group uses the current selection as its parent and is not duplicated in the global toolbar. Rows are transparent at rest, use `--hover-bg` on hover/focus, and mix selection color into the selected state. The detail pane is hideable via the toolbar `eye/eye-off` (now bound to `showDetailOnSelect` – click entry auto-shows detail, `selectionVersion` allows same entry to re-open after close) or the More-menu action (hidden collapses `--detail-width` to `0` and unmounts the pane); when visible its body is organized into tabs 字段 (fields) / 元属性 (meta) / 附件 (attachments) / 历史 (history), with the active tab underlined in `--selection-color`. On the attachments tab the whole area is a drag-and-drop dropzone (`.attachment-dropzone`): a dashed `--border-subtle` frame with `--font-size-tiny` hint text, highlighting `--selection-color` with a `color-mix` tint while a file drag hovers it, and adding dropped files in memory via `add_attachments` (fields never rewritten). On the fields tab the notes editor sits at the bottom of a column layout: fields scroll in the flex-1 region above, while `.notes-section` hugs its content (auto-sized, capped at ~45% of the detail body via `resizeNotes`, taller content scrolls internally) and needs no manual save (input + blur debounced auto-save via `update_entries`). The notes area is dual-mode: a read view (`notes-view`) renders the note text with inline clickable contacts — URLs open externally, emails/phones render as plain text — and clicking anywhere else on the note switches to the `notes-textarea` editor; blur or switching entries returns to the read view. The group tree and detail pane are additionally resizable via their `--group-width` / `--detail-width` variables and col-resize handles.
- The entry table is a fixed-width grid (icon → configurable built-in/custom columns → actions). `EntryTable.svelte` owns its header/row markup, windowing using indexed row offsets, transient resize/reorder interaction state, and desktop/narrow scoped CSS; `+page.svelte` supplies sorted rows, persisted column state, selection actions, and vault mutations. Only the visible rows plus six overscan rows per edge are mounted, using normal-flow top/bottom spacers so horizontal alignment and native vertical scrolling stay intact; the entry-row height comes from the layout setting `density.entryRowHeight` (default 30px, `--entry-row-height` drives both the virtualization math and the CSS; narrow rows floor at 36px, group-separator rows use half that height with a 14px floor). Each column is a fixed px track: drag the header divider to resize (clamped 30–400px), drag a header to reorder (4px pointer threshold distinguishes drag from sort-click; the insertion point is marked with a `--selection-color` 2px line), right-click a header for the visibility config menu, and click a sortable header to cycle sort. The `general.entryColumns` array order is the persisted display order (resize + reorder both save via `saveLayout`). Columns are left-packed with the leftover space empty; the title column's `width: 0` is an auto sentinel that renders the 200px default. When the column total exceeds the window width, `.entry-table` scrolls horizontally as one unit — the rows carry `width: max-content` so the single horizontal scrollbar keeps them aligned under the full-width header bar. The URL subtitle (scheme stripped, shown only when a URL exists) is gated by the `showDescriptions` setting. Rows render the entry's background tag as a 3px `--row-color` bar and, when the entry has one, the KeePass `ForegroundColor` as the row text color (`--row-fg`; the row/title fall back to `--text-primary` when unset).
- Status bar is a low-contrast footer separated by one subtle border; its left side carries the entry count, the active group filter (筛选于 …), and the unsaved-changes indicator; popovers/dialogs are elevated surfaces with a border plus shadow.
- **Narrow / mobile layout (`@media (max-width: 720px)`):** the app-shell drops its `min-width` floor and becomes a single-column stack. Its sole grid column stays `minmax(0, 1fr)` so toolbar min-content cannot widen the main content beyond the viewport. Because `.main-content` switches to `display: block`, it and `.entry-panel` must keep an explicit height chain (`height: 100%` + `min-height: 0`); otherwise the entry panel grows to content height and `.entry-list` never overflows internally, leaving the page unscrollable by wheel under the shell's `overflow: hidden`. A `menu` toggle in `toolbar-left` opens the group tree as a left slide-in drawer (`.mobile-nav-open` on the shell + a `.mobile-drawer-backdrop` to dismiss; choosing a group closes it). The desktop entry grid collapses to a viewport-width summary list (icon, title/username, favorite/copy actions) with no horizontal scrollbar; the full persisted column grid remains desktop-only. The detail pane becomes a full-width overlay with a `back` button in `EntryDetail` (`onback` prop) returning to the list. Detail flex children must keep `min-width: 0`, field rows stay within the overlay, and long single-line values ellipsize instead of widening the viewport. Resize handles, desktop grid columns, and mobile window controls are hidden. Toolbar text labels are hidden so controls stay icon-only; at `max-width: 420px` the toolbar becomes two rows (primary actions above, search plus secondary actions below) and uses 32px touch targets. The per-item `toolbarItems` (now 15 + 3 window, `newEntry/save/lock` always `true`) and full-order `toolbarFullOrder` (18) defaults keep the same platform semantics (mobile starts with `saveAs/toggleDetail/securityReport/exportMenu/appSettings` collected into the More menu; desktop starts with `saveAs/toggleDetail/securityReport/exportMenu/appSettings` pinned, others collected, `moreMenu` always visible at right, window three at far right), each remains individually overridable in General → Toolbar (global order, left/right, dividers, auto-show `eye`), and window buttons remain hidden on mobile regardless; `showDetailOnSelect` (default `true`) controls click-to-show and is toggled by both the toolbar `eye` button and the settings card.
- **Narrow settings layout (`@media (max-width: 720px)`):** `SettingsDialog` removes the fixed 168px grid column and lets the content pane fill the viewport. A menu button in the section header opens the primary-category sidebar as a left drawer; the backdrop and category selection close it. The secondary category row remains single-line and horizontally scrollable when needed.
- Motion is short and functional. Respect the global reduced-motion rule.

The detail history hint describes the database's count and size limits. Do not
hard-code a version count or promise that the pre-restore version is always
retained: the configured limits also apply to restore snapshots. Keep this
copy in the existing muted `.history-hint` paragraph in both locales.

The Save As toolbar tooltip and Appearance toolbar description use the same
localized key. Explain that successful Save As switches future saves to the new
file; keep the compact visible label unchanged. A remote session becomes local.

AppToolbar reserves at least 160px for its search area. Left/right action groups
stay within their allocated width and scroll horizontally when custom visibility,
side assignments or labels exceed it. Buttons and the read-only badge do not
shrink; groups reuse the thin scrollbar token and contained horizontal overscroll.
Button focus keeps the existing theme outline inset so scrolling does not clip it.
Focus entry scrolls the button instantly with nearest block/inline alignment;
browser automatic focus scrolling alone can briefly leave the final button clipped.
At 420px and below the two-row grid uses a minimum 160px search track and a shrinkable
secondary track; the primary group is capped to the full row. All action buttons,
including the more-specific primary button rule, retain 32px touch targets there.
Sparse configurations remain compact without adding a scrollbar when they fit.
Actual rendered geometry and native keyboard traversal in the browser fixture
cover the access path; this is not Android/WebView or vault-operation evidence.

## Theme color contract

The entry editor's Website rules tab follows its existing compact, single-line,
scrolling tab row. `WebsiteRulesPanel` uses shared 30px TextField/Select/field
Button controls and Toggle/inline Feedback. Local CSS owns flex layout and hint
spacing only, using existing text/theme metrics without new tokens. Below 480px
each URL occupies a full row with scope/remove controls below it. Add/remove
restores focus to the new/nearest URL (or Add URL when empty). Load failures and
OverrideURL conflicts have visible recovery text; tab changes preserve the draft.

History details reuse the existing Metadata grid, read-value change border,
tab-dot and swap button for the website-policy presence summary. The rule change
also contributes to the existing difference count and timeline chip mapping.
Stored/Not set describes presence only, without fetching or exposing JSON rules.
The summary spans the Metadata grid width and wraps its compact controls on
narrow screens. History tabs reuse the editor's one-line, nonshrinking horizontal
scroll pattern, nearest focus scrolling and inset focus outline; localized labels
must not enlarge the modal content box.

The `ThemeColors` interface has 21 semantic values (identical names to Clipboard plus `linkColor` for URL/contact links):

| CSS variable          | ThemeColors key    | Purpose                           |
| --------------------- | ------------------ | --------------------------------- |
| `--bg-app`            | `bg`               | application/body background       |
| `--bg-settings`       | `settingsBg`       | main/settings shell background    |
| `--accent`            | `accent`           | product/focus accent              |
| `--text-primary`      | `textPrimary`      | primary content                   |
| `--text-secondary`    | `textSecondary`    | secondary content                 |
| `--text-muted`        | `textMuted`        | descriptions/metadata             |
| `--text-faint`        | `textFaint`        | lowest-emphasis text/icons        |
| `--placeholder-color` | `placeholderColor` | placeholders                      |
| `--border-color`      | `border`           | regular borders                   |
| `--border-subtle`     | `borderSubtle`     | dividers/quiet borders            |
| `--card-bg`           | `cardBg`           | card/elevated controls            |
| `--surface-bg`        | `surfaceBg`        | popovers/panels                   |
| `--statusbar-bg`      | `statusBarBg`      | footer/status bar                 |
| `--hover-bg`          | `hoverBg`          | hover and quiet selected surfaces |
| `--input-bg`          | `inputBg`          | inputs and inset surfaces         |
| `--selection-color`   | `selectionColor`   | selection/current state           |
| `--link-color`        | `linkColor`        | URL / clickable contact links     |
| `--success-color`     | `successColor`     | successful state                  |
| `--danger-color`      | `dangerColor`      | destructive/error state           |
| `--warning-color`     | `warningColor`     | caution/favorite emphasis         |
| `--scrollbar-color`   | `scrollbarColor`   | scroll thumb                      |

Use these variables for reusable surfaces, text, borders, controls, and status states. Derive translucency with `color-mix` instead of inventing parallel shades. URL link text uses `--link-color` (a dedicated theme token, editable in General → 外观), never `--selection-color`, which is reserved for selection/current state (see `.field-text.link`/`.field-text.contact` and `.notes-link` in `EntryDetail.svelte` and `.read-text.link` in `HistoryVersionDialog.svelte`).

## Global font and display variables

| Variable                  | Current default    | Use                                                                                                            |
| ------------------------- | ------------------ | -------------------------------------------------------------------------------------------------------------- |
| `--font-size-base`        | `14px`             | general UI/body                                                                                                |
| `--font-size-secondary`   | `11px`             | metadata/descriptions                                                                                          |
| `--font-size-tiny`        | `10px`             | smallest notes                                                                                                 |
| `--font-size-cardTitle`   | `13px`             | card title                                                                                                     |
| `--font-size-cardPreview` | `11px`             | card preview                                                                                                   |
| `--font-mono`             | ui-monospace stack | monospace code/ids (client IDs, TOTP codes, side-channel passwords); do not hardcode font stacks in components |

`settings-bootstrap.ts` applies these and the 21 theme variables at startup. Live settings panels must update the same contract; do not create alternate variable names for the same meaning.

## Settings semantic metrics

The settings shell defines the settings scope variables consumed by shell and child panels:

| Variable                                                                           | Standard fallback/use                           |
| ---------------------------------------------------------------------------------- | ----------------------------------------------- |
| `--settings-page-title-size`                                                       | base + 4px; standalone panel `h2`               |
| `--settings-heading-size`                                                          | card-title size; card/section heading           |
| `--settings-description-size`                                                      | secondary size; descriptions                    |
| `--settings-note-size`                                                             | tiny size; notes/counts                         |
| `--settings-control-size`                                                          | secondary size; buttons/inputs/select/list rows |
| `--settings-feedback-size`                                                         | description size                                |
| `--settings-feedback-radius`                                                       | 7px                                             |
| `--settings-card-radius`                                                           | 9px                                             |
| `--settings-control-radius`                                                        | 6px                                             |
| `--settings-icon-radius`                                                           | 7px                                             |
| `--settings-close-size` / `--settings-close-radius` / `--settings-close-font-size` | 28px / 7px / 19px                               |

## Template layer (controls)

All control chrome lives in `src/lib/components/templates/**` as Svelte
components with scoped styles — feature components compose them and never
re-declare primitive classes:

- `templates/form/Toggle.svelte` — switch + knob (`checked` bindable,
  `onchange`); replaces the retired `.toggle-switch`/`.toggle-knob`.
- `templates/form/TextField.svelte` — input/textarea with `mono`, `numeric`,
  `invalid`, `size="control"` (30px settings form), `paddingRightPx` (embedded
  trailing action), `autofocus`; replaces `.text-input` and `.settings-input`.
- `templates/form/Feedback.svelte` — success/danger banner, `inline` variant;
  replaces `.settings-feedback`.
- `templates/action/Button.svelte` — variants `plain`/`primary`/`danger`/`warning`
  (dialog footers), `action`/`field` (compact settings actions); replaces
  `.modal-button` and `.settings-action-button`.
- `templates/menu/MenuItem.svelte` — viewport-menu row with `leading` snippet;
  replaces `.menu-item`.

A static guard (`tests/ui-primitives.test.mjs`) fails when a retired class
name reappears in any component outside `templates/`. Extend its
`RETIRED_CLASSES` list as further primitives migrate.

## Shared settings layout primitives

`settings-shared.css` still owns the panel **layout** vocabulary (not controls):

- standalone `header`, `.eyebrow`, `h2`, header description, and `.close-button`;
- `.settings-scroll` (and scrollbar treatment) plus
  `.settings-scroll--stack-rows` for the sub-720px stacked rows;
- `.setting-card`, `.toggle-card`, `.setting-heading`, `.setting-icon`,
  `.heading-inline`, `.value-label`, `.setting-row`, `.settings-label`,
  `.settings-row`, `.settings-note(.warn)`;
- `.transparency-slider` including WebKit/Firefox tracks/thumbs;
- color-swatch/color-input helpers for the custom theme editor;
- `.theme-segmented` / `.theme-segment(.active)` and `.swatch*` — the shared
  segmented control used by the appearance theme mode and the general language
  switch;
- `.mirror-dir`, `.auto-save-note`, and the default pointer cursor.

`src/app.css` imports this file globally. Child panels must rely on these
layout classes and add only their panel-specific layout.
The theme/language option group wraps complete buttons when the available width
cannot fit their single-line labels. Buttons share each row's remaining space,
retain 30px height and 6px group gaps, and theme swatches never shrink below 12px.
Check both locales at the allowed maximum secondary font size on narrow screens.
`SettingToggleCard.svelte` and `SettingRangeCard.svelte` own the canonical
standard-card markup and consume these classes without defining parallel scoped
styles. Migrating these layout blocks into template components is planned
follow-up work.

## Modal surface

`ModalShell.svelte` is the canonical dialog surface: backdrop/prompt layer,
fixed size variants, bordered surface, header icon/title/description, optional
close button, optional scroll behavior, and the `.modal-actions` footer that
hosts `<Button>` templates from the caller's `actions` snippet. Dialogs supply
only their body/action snippets; do not reintroduce parallel modal surfaces,
header/icon blocks, or footer layouts.

Its Tab trap recomputes enabled, rendered controls on each key. Negative
tabindex, effective `:disabled` (including fieldsets), inert ancestry and hidden
layout/visibility never define a wrap boundary. Hidden attachment file inputs
remain programmatically invoked by their visible Add button. Only the top modal
handles unconsumed keys. `tests/modal-focus.test.mjs` exercises the production
handler with a DOM seam; actual native Tab/layout needs browser verification.

EntryEditorDialog's secondary tabs retain compact padding, one-line labels and
nonshrinking items. The row scrolls horizontally with the existing thin
scrollbar token when labels, locale or configured font exceed its width. Focus
entry scrolls the item with nearest block/inline alignment so partially visible
tabs are fully revealed; the existing focus outline is inset to avoid clipping.
Keep this overflow inside the tab row,
including on narrow screens, rather than widening the modal surface.

## Viewport menu

`ViewportMenuShell.svelte` is the canonical viewport-fixed popover menu: it
clamps to the window with an 8px margin on content/viewport resize, bounds its
width and height and scrolls internally when the menu exceeds the viewport.
It focuses the first enabled item on mount. `utils/menu-keyboard.ts` captures
menu navigation before page bubble shortcuts, scopes Up/Down/Home/End to the
current menu level and skips disabled items. Escape restores invocation focus;
leaf Enter/Space preserve native activation and stop page bubble shortcuts.
The page's earlier window-capture shortcuts also respect `activeContextMenu`.
Tab restores it before normal browser traversal. Cleanup restores only owned
focus; outside click/focus and newly focused dialogs keep their focus. It
closes on Escape, Tab or outside click/focus,
accepts role/aria label plus a `column-config` modifier for the bounded,
scrollable column picker, and owns the elevated-surface styles in its scoped
block. Items are composed from `templates/menu/MenuItem.svelte`; do not
introduce a second fixed-menu surface or duplicate item base rules.
`ContextMenu` cascades use viewport-fixed positioning to escape the scrollable
parent's clipping, flip beside the anchor when space permits, clamp within the
same margin and follow parent scroll/viewport resize. Long cascades scroll in
their own surface. Parents expose `aria-haspopup`/`aria-expanded`/`aria-controls`
with instance-scoped IDs. Right/Enter/Space enter enabled child rows; Left or
Escape returns to the parent, with a second Escape closing the root. Deferred
child focus must check connection, expanded identity and current parent focus.
Pointer movement that removes a focused child first transfers focus to the
enabled sibling (or previous parent for a disabled sibling), preserving Escape.
Items use `tabindex=-1`; the menu owns keyboard navigation, and global
`:focus-visible` supplies the existing theme/forced-colors outline. Reuse the
existing surface, border and scrollbar tokens.

## CSS ownership decision

Before adding a rule, place it at the narrowest correct stable level:

- Theme color or global accessibility/reset → `app.css`, `ThemeColors`, presets, and `theme.ts` together.
- Shared settings card/control/feedback primitive → `settings-shared.css`.
- Shared modal shell primitive → `ModalShell.svelte`.
- Shared control (toggle/text field/button/feedback/menu item) → the matching
  `templates/**` component.
- Settings shell/navigation layout → `SettingsDialog.svelte`.
- One reusable component's unique layout → that component's scoped `<style>`.

Do not use a parent scoped selector to style inside a child Svelte component.
Compose the child with props, wrap it in a layout element, or move a genuinely
shared rule into the owning template.

## Form and control conventions

- Toggle card: label/icon/description on the left, `<Toggle>` on the right in one row.
- Slider card: heading/value on one row, unwrapped `input[type="range"].transparency-slider` below, and `--slider-pct` updated from the current value.
- Number input: `TextField numeric` — textfield appearance, WebKit spin buttons hidden by the template.
- Select: use the shared `Select.svelte` component (button trigger with `aria-haspopup="listbox"`, fixed-position `role="listbox"` and keyboard navigation); never introduce raw `<select>` markup. Trigger matches settings control size/radius/colors; the list is a `--surface-bg` popover at the popover z-index with a short pop-in animation that respects reduced motion.
- Feedback: use `templates/form/Feedback.svelte` (`success` palette; `inline` for card-contained messages), dismissible by time and accessible.
- Buttons and inputs need visible focus. Never remove outline without a replacement.
- Segmented groups (theme/KDF/charset chips): 1px border, `--settings-control-radius`, active = selection-tinted.

Main-window transient tips reuse `TipKind` and the existing success/danger
palettes in `TipsHost.svelte`. Pass `"error"` from explicit failure branches
through page/flow callbacks; never select a color by parsing localized messages.
The severity repair adds no CSS variables, declarations or layout changes.

Tip body text uses `--text-primary` for legible dark/light/custom feedback.
Success/error identity stays in the existing tinted background and border;
do not lighten semantic colors with white for small message text. Measure the
rendered foreground/background contrast, not only the raw theme palette.
The fixed tip host stays content-sized up to 560px and the viewport minus
24px. Individual messages respect that width and wrap long filenames/URLs;
keep the existing 6px/12px padding, 11px secondary font and centered position.

## Fixed layer order

| Layer                                                               | Current z-index |
| ------------------------------------------------------------------- | --------------- |
| modal backdrop / dialog                                             | 50              |
| app-level security prompts (association approval, RPC side-channel) | 80              |
| transient tips / toast host (`TipsHost.svelte`)                     | 1000            |
| context menu / popover                                              | 9999            |

Use `.modal-backdrop` for the shared fixed, centered, dimmed layer at
z-index 50. App-level security prompts add `.modal-backdrop--prompt` to
select z-index 80; components own only their dialog surface.

Check the complete stacking context before changing a value; do not solve one overlap by arbitrary escalation.

## Frameless window drag regions

The window is frame-less, so chrome rows carry `data-tauri-drag-region`. Tauri (>= 2.6) reads three values, resolved by walking the composed path from the mousedown target upward and stopping at the first clickable element (button/input/select/textarea/label/summary/a, `contenteditable`, a `tabindex` other than `-1`, or an interactive `role`):

| Value   | Effect                                                                |
| ------- | --------------------------------------------------------------------- |
| bare    | self only — a click must land on this exact element, not a descendant |
| `deep`  | the whole subtree drags, except where a clickable element blocks it   |
| `false` | drag blocked here and for ancestors                                   |

Rules:

- A row whose visible area is entirely wrapped in child elements (the toolbar's three `toolbar-left`/`toolbar-center`/`toolbar-right` groups) must use `deep`, otherwise every blank stretch between controls is dead space. The hit-test still stops at the buttons and the search input, so they keep working.
- A container that is fully covered by interactive content (`.main-content`, `#settings-content`) must keep the bare form; `deep` there turns the entry list / settings body into a window-drag surface and breaks selection, text selection, and panel interaction.
- Never put the attribute on the interactive element itself — that is what makes a button drag the window.
- `tests/component-contracts.test.mjs` pins both halves of this rule (toolbar is `deep` and the only marked element; the full-surface containers stay bare).

## Style change gate

1. Identify whether the task changes the main page, settings, or a niche surface.
2. Inspect the target plus sibling components that use the same primitive.
3. Confirm token/ownership placement using this reference.
4. Search for duplicate selectors and raw colors/metrics before adding new declarations.
5. Keep markup hierarchy, keyboard focus, overflow, narrow-window behavior, and reduced/high-contrast behavior intact.
6. Run `npm run check` and `npm run build`.
7. Perform rendered/runtime comparison in dark and light/custom theme when the change affects theme-facing UI; test the target window size and a narrow size.
8. Run the documentation currency gate before commit.

Escape key action uses the existing setting-card/setting-heading/setting-icon
and heading-inline layout with shared Select and keyboard icon in Appearance Window. It adds no CSS, token, font, surface or metric; all five values reuse
existing control chrome. GroupNode consumes rename Escape and prevents a subsequent blur
commit, retaining compact row styling. Five isolated Chinese-light/English-dark/
custom desktop/320px browser cases retain the Cascadia stack, contained card/
Select widths and control/modal/menu cancellation ownership. The existing English
320px group context menu emits ResizeObserver notifications; ViewportMenuShell
remains unchanged from the base and needs a separate layout fix. Native WebView
and desktop window lifecycle remain separate runtime gates.
