<script lang="ts">
  import type { VaultGroup } from "$lib/types/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";

  interface Props {
    group: VaultGroup;
    onclose: () => void;
    onsaved: (meta: {
      notes?: string;
      tags?: string;
      enableSearching?: boolean;
    }) => Promise<boolean>;
  }

  let { group, onclose, onsaved }: Props = $props();

  // The dialog is mounted per open, so capturing the initial group is intended.
  // svelte-ignore state_referenced_locally
  let notes = $state(group.notes ?? "");
  // svelte-ignore state_referenced_locally
  let tags = $state(group.tags ?? "");
  // svelte-ignore state_referenced_locally
  let enableSearching = $state(group.enableSearching);
  let saving = $state(false);

  async function submit(): Promise<void> {
    if (saving) return;
    saving = true;
    try {
      const current = await onsaved({ notes, tags, enableSearching });
      if (current) onclose();
    } finally {
      saving = false;
    }
  }
</script>

<ModalShell
  title="分组属性"
  description={group.name}
  size="small"
  closeOnEscape={!saving}
  {onclose}
>
  {#snippet children()}
    <div class="block">
      <span class="label">备注</span>
      <TextField multiline rows={3} bind:value={notes} placeholder="分组备注" />
    </div>
    <div class="block">
      <span class="label">标签（逗号分隔）</span>
      <TextField bind:value={tags} placeholder="work, dev" />
    </div>
    <div class="block">
      <span class="label">搜索参与</span>
      <button
        type="button"
        class="toggle"
        class:active={enableSearching}
        onclick={() => (enableSearching = !enableSearching)}
        aria-pressed={enableSearching}
      >
        {enableSearching ? "参与搜索" : "排除于搜索"}
      </button>
      <p class="hint">关闭后该分组的条目不参与搜索、自动填充与字段引用（REF）解析</p>
    </div>
  {/snippet}
  {#snippet actions()}
    <Button onclick={onclose} disabled={saving}>取消</Button>
    <Button variant="primary" onclick={() => void submit()} disabled={saving}>
      {saving ? "保存中…" : "保存"}
    </Button>
  {/snippet}
</ModalShell>

<style>
  .block {
    margin-bottom: 12px;
  }

  .label {
    display: block;
    margin-bottom: 6px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .toggle {
    height: 28px;
    padding: 0 12px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .toggle:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .toggle.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .hint {
    margin: 6px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }
</style>
