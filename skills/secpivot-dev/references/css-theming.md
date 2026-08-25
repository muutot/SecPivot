# Project-wide UI Style and Theming

This is the authoritative style reference for SecPivot UI. Read it before any markup or CSS change. The visual language is derived from the Clipboard Desktop project and is intentionally kept compatible so settings can share identical primitives.

## Style source-of-truth order

1. `src/routes/+page.svelte` and the settings shell define the product's visual language.
2. `src/lib/types/theme.ts::{DARK_THEME_COLORS,LIGHT_THEME_COLORS}` defines preset values.
3. `src/lib/utils/theme.ts::applyThemeColors` defines the ThemeColors → CSS-variable mapping.
4. `src/app.css` provides root defaults, global reset, font variables, the shared modal backdrop/layer modifiers, focus/accessibility rules, and imports shared settings/modal CSS.
5. `src/lib/styles/settings-shared.css` owns reusable settings-panel primitives.
6. `SettingsDialog.svelte` owns settings-shell layout and panel-specific styles.
7. Component-scoped CSS owns only component-specific layout/visual behavior.

When these disagree, inspect the rendered target and current code, fix the narrow source of divergence, and update this reference if the approved rule changes.

## Approved compact visual language

Preserve these characteristics unless the task explicitly requests a redesign:

- Dense desktop utility layout with one continuous neutral surface, subtle borders, compact spacing, and restrained radii.
- Near-black/dark-neutral surfaces with high-legibility primary text and progressively quieter secondary, muted, and faint text.
- Light-green `--accent` for product/focus emphasis, gray `--selection-color` for selection/current state, generic-blue `--link-color` for links, and semantic success/danger/warning colors.
- Toolbar row (the top chrome, itself the drag region via `data-tauri-drag-region`, frame-less window) owns compact action buttons on the left (new entry · save / save-as · lock, grouped by a 1px `--border-subtle` divider), a **centered compact search box** (`--input-bg`, subtle border, muted placeholder) in the middle, and on the right either the individual view/tool icons (detail toggle, security report, export CSV, settings) or one More menu when `general.toolbarOverflowMenu` is enabled, followed on desktop by a divider and the window controls (minimize / maximize-restore / close — shared `WindowControls.svelte`, close hover = `--danger-color`). The More menu also owns save-as so lower-frequency actions share one surface. Search is intentionally low-emphasis: this is a password manager, so it must not dominate the header. The vault file name lives in the status bar, not the toolbar.
- The window is frame-less (`decorations: false`); `WindowControls.svelte` provides the shared minimize/maximize/close cluster with two variants: `toolbar` (bordered compact buttons matching `.icon-action`) and `chrome` (flat titlebar-style). The welcome/lock views get a 30px absolute top bar (`standalone-bar`) carrying the chrome variant and acting as the drag region; the settings shell deliberately shows NO window controls (only the ← back button) — window actions live in the main toolbar.
- Three-pane main layout: group tree (left) → entry list (center) → detail (right). The group-tree title bar owns its local tools in the order new subgroup → expand all → collapse all; adding a group uses the current selection as its parent and is not duplicated in the global toolbar. Rows are transparent at rest, use `--hover-bg` on hover/focus, and mix selection color into the selected state. The detail pane is hideable via the toolbar chevron or the More-menu action (hidden collapses `--detail-width` to `0` and unmounts the pane); when visible its body is organized into tabs 字段 (fields) / 元属性 (meta) / 附件 (attachments) / 历史 (history), with the active tab underlined in `--selection-color`. On the attachments tab the whole area is a drag-and-drop dropzone (`.attachment-dropzone`): a dashed `--border-subtle` frame with `--font-size-tiny` hint text, highlighting `--selection-color` with a `color-mix` tint while a file drag hovers it, and adding dropped files in memory via `add_attachments` (fields never rewritten). On the fields tab the notes editor sits at the bottom of a column layout: fields scroll in the flex-1 region above, while `.notes-section` hugs its content (auto-sized, capped at ~45% of the detail body via `resizeNotes`, taller content scrolls internally) and needs no manual save (input + blur debounced auto-save via `update_entries`). The notes area is dual-mode: a read view (`notes-view`) renders the note text with inline clickable contacts — URLs open externally, emails/phones copy — and clicking anywhere else on the note switches to the `notes-textarea` editor; blur or switching entries returns to the read view. The group tree and detail pane are additionally resizable via their `--group-width` / `--detail-width` variables and col-resize handles.
- The entry table is a fixed-width grid (icon → configurable built-in/custom columns → actions). `EntryTable.svelte` owns its header/row markup, fixed-height windowing, transient resize/reorder interaction state, and desktop/narrow scoped CSS; `+page.svelte` supplies sorted rows, persisted column state, selection actions, and vault mutations. Only the visible rows plus six overscan rows per edge are mounted, using normal-flow top/bottom spacers so horizontal alignment and native vertical scrolling stay intact; row heights are 30px desktop, 34px compact desktop, and 48px narrow. Each column is a fixed px track: drag the header divider to resize (clamped 30–400px), drag a header to reorder (4px pointer threshold distinguishes drag from sort-click; the insertion point is marked with a `--selection-color` 2px line), right-click a header for the visibility config menu, and click a sortable header to cycle sort. The `general.entryColumns` array order is the persisted display order (resize + reorder both save via `saveLayout`). Columns are left-packed with the leftover space empty; the title column's `width: 0` is an auto sentinel that renders the 200px default. When the column total exceeds the window width, `.entry-table` scrolls horizontally as one unit — the rows carry `width: max-content` so the single horizontal scrollbar keeps them aligned under the full-width header bar. The URL subtitle (scheme stripped, shown only when a URL exists) is gated by the `showDescriptions` setting. Rows render the entry's background tag as a 3px `--row-color` bar and, when the entry has one, the KeePass `ForegroundColor` as the row text color (`--row-fg`; the row/title fall back to `--text-primary` when unset).
- Status bar is a low-contrast footer separated by one subtle border; its left side carries the entry count, the active group filter (筛选于 …), and the unsaved-changes indicator; popovers/dialogs are elevated surfaces with a border plus shadow.
- **Narrow / mobile layout (`@media (max-width: 720px)`):** the app-shell drops its `min-width` floor and becomes a single-column stack. Its sole grid column stays `minmax(0, 1fr)` so toolbar min-content cannot widen the main content beyond the viewport. Because `.main-content` switches to `display: block`, it and `.entry-panel` must keep an explicit height chain (`height: 100%` + `min-height: 0`); otherwise the entry panel grows to content height and `.entry-list` never overflows internally, leaving the page unscrollable by wheel under the shell's `overflow: hidden`. A `menu` toggle in `toolbar-left` opens the group tree as a left slide-in drawer (`.mobile-nav-open` on the shell + a `.mobile-drawer-backdrop` to dismiss; choosing a group closes it). The desktop entry grid collapses to a viewport-width summary list (icon, title/username, favorite/copy actions) with no horizontal scrollbar; the full persisted column grid remains desktop-only. The detail pane becomes a full-width overlay with a `back` button in `EntryDetail` (`onback` prop) returning to the list. Detail flex children must keep `min-width: 0`, field rows stay within the overlay, and long single-line values ellipsize instead of widening the viewport. Resize handles, desktop grid columns, and mobile window controls are hidden. Toolbar text labels are hidden so controls stay icon-only; at `max-width: 420px` the toolbar becomes two rows (primary actions above, search plus secondary actions below) and uses 32px touch targets. Android/iOS defaults `toolbarOverflowMenu` on, leaving new entry, save, and lock visible while routing the lower-frequency actions through the shared viewport-clamped `ContextMenu`; desktop defaults it off, and the setting remains user-overridable on both platforms.
- **Narrow settings layout (`@media (max-width: 720px)`):** `SettingsDialog` removes the fixed 168px grid column and lets the content pane fill the viewport. A menu button in the section header opens the primary-category sidebar as a left drawer; the backdrop and category selection close it. The secondary category row remains single-line and horizontally scrollable when needed.
- Motion is short and functional. Respect the global reduced-motion rule.

## Theme color contract

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

`settings-bootstrap.ts` applies these and the 20 theme variables at startup. Live settings panels must update the same contract; do not create alternate variable names for the same meaning.

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
- `templates/action/Button.svelte` — variants `plain`/`primary`/`danger`
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
- `.mirror-dir`, `.auto-save-note`, and the default pointer cursor.

`src/app.css` imports this file globally. Child panels must rely on these
layout classes and add only their panel-specific layout.
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

## Viewport menu

`ViewportMenuShell.svelte` is the canonical viewport-fixed popover menu: it
clamps to the window with an 8px margin, closes on Escape or click-outside,
accepts role/aria label plus a `column-config` modifier for the bounded,
scrollable column picker, and owns the elevated-surface styles in its scoped
block. Items are composed from `templates/menu/MenuItem.svelte`; do not
introduce a second fixed-menu surface or duplicate item base rules.

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
- Select: use the shared `Select.svelte` component (self-drawn trigger + fixed-position listbox, keyboard nav, `role="combobox"`/`listbox`); never introduce raw `<select>` markup. Trigger matches settings control size/radius/colors; the list is a `--surface-bg` popover at the popover z-index with a short pop-in animation that respects reduced motion.
- Feedback: use `templates/form/Feedback.svelte` (`success` palette; `inline` for card-contained messages), dismissible by time and accessible.
- Buttons and inputs need visible focus. Never remove outline without a replacement.
- Segmented groups (theme/KDF/charset chips): 1px border, `--settings-control-radius`, active = selection-tinted.

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

## Style change gate

1. Identify whether the task changes the main page, settings, or a niche surface.
2. Inspect the target plus sibling components that use the same primitive.
3. Confirm token/ownership placement using this reference.
4. Search for duplicate selectors and raw colors/metrics before adding new declarations.
5. Keep markup hierarchy, keyboard focus, overflow, narrow-window behavior, and reduced/high-contrast behavior intact.
6. Run `npm run check` and `npm run build`.
7. Perform rendered/runtime comparison in dark and light/custom theme when the change affects theme-facing UI; test the target window size and a narrow size.
8. Run the documentation currency gate before commit.
