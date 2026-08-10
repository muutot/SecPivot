# Settings Shell and Panel Patterns

Read this file and `css-theming.md` before changing settings markup or CSS.

## Navigation and ownership

`SettingsDialog.svelte` is the parent shell for the settings page. Its primary categories:

| Primary category | Secondary sections / implementation                                                                                                                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| General          | Appearance / Display / Compact → `GeneralSettingsPanel` with `section` prop                                                                                                                                                                |
| Security         | Single current section → `SecuritySettingsPanel`                                                                                                                                                                                           |
| Database         | Single current section → `DatabaseSettingsPanel`                                                                                                                                                                                           |
| Remote           | Single current section → `RemoteSettingsPanel` (profile selector + name + transport kind (S3/WebDAV) + endpoint/credentials/mirror; WebDAV hides region/bucket and relabels creds as username/password; edits apply to the active profile) |
| Integrations     | KeePassHttp / KeePassRPC tabs → `BridgeSettingsPanel` / `RpcSettingsPanel`                                                                                                                                                                 |
| About            | Single current section → `AboutSettingsPanel`                                                                                                                                                                                              |

The left sidebar retains primary categories. The right content pane owns breadcrumb, secondary row, description, and the selected panel.

At narrow widths (`max-width: 720px`), the primary sidebar becomes an off-canvas
drawer. The section header exposes the menu toggle, the content pane uses the
full viewport width, and selecting a primary category or pressing the backdrop
closes the drawer. Keep the secondary row horizontally scrollable instead of
wrapping labels character-by-character.

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

Use `.settings-action-button` for compact secondary actions shared across
panels. Its default height is 26px; add `.settings-action-button--field` only
when it must align with a 30px settings input. Keep only semantic or local
layout overrides in the child panel.

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

- Use `.settings-feedback`; add `.success` for success and default to error styling otherwise.
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

- Hide number spin buttons when the control is visually a plain value field.
- Keep tabular numeric labels stable and non-shrinking.
- Let long labels/paths/numbers shrink, wrap, or ellipsize without overflowing the card.
- Use semantic status colors through variables and `color-mix`.

## New or changed setting checklist

1. Update the relevant TypeScript type, defaults, and normalization in `services/settings.ts`.
2. Update Rust `config.rs` serde/defaults when backend behavior should be typed.
3. Add/update panel UI using the approved shell and shared primitives.
4. Apply live document behavior or clearly mark restart-required behavior.
5. Update `data-contracts.md` and `css-theming.md`/`settings-panels.md` when applicable.
6. Run static/build checks plus rendered dark/light/custom and narrow-window verification when visual.
