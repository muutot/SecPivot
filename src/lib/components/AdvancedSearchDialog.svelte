<script lang="ts">
  import type { AdvancedSearchQuery, SearchFieldScope } from "$lib/utils/entry-search";
  import ModalShell from "$lib/components/ModalShell.svelte";

  interface Props {
    initialQuery?: AdvancedSearchQuery | null;
    onapply: (query: AdvancedSearchQuery) => void;
    onclear: () => void;
    onclose: () => void;
  }

  let { initialQuery = null, onapply, onclear, onclose }: Props = $props();

  // The dialog is mounted per open, so the query is captured once.
  // svelte-ignore state_referenced_locally
  const captured = initialQuery;
  let text = $state(captured?.text ?? "");
  let field = $state<SearchFieldScope>(captured?.field ?? "all");
  let regex = $state(captured?.regex ?? false);
  let exclude = $state(captured?.exclude ?? false);
  let onlyExpired = $state(captured?.onlyExpired ?? false);
  let onlyFavorites = $state(captured?.onlyFavorites ?? false);
  let tags = $state(captured?.tags ?? "");
  let requireQualityCheck = $state(captured?.requireQualityCheck ?? false);

  const FIELD_OPTIONS: { value: SearchFieldScope; label: string }[] = [
    { value: "all", label: "全部字段" },
    { value: "title", label: "标题" },
    { value: "username", label: "用户名" },
    { value: "url", label: "网址" },
    { value: "notes", label: "备注" },
    { value: "tags", label: "标签" },
    { value: "custom", label: "自定义字段" },
  ];

  function apply(): void {
    onapply({
      text,
      field,
      regex,
      exclude,
      onlyExpired,
      onlyFavorites,
      tags,
      requireQualityCheck,
    });
    onclose();
  }
</script>

<ModalShell
  title="高级搜索"
  description="组合条件过滤当前视图；正则非法时视为不匹配"
  size="medium"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    <div class="block">
      <span class="label">字段范围</span>
      <div class="chips" role="radiogroup" aria-label="字段范围">
        {#each FIELD_OPTIONS as option (option.value)}
          <button
            type="button"
            class="chip"
            class:active={field === option.value}
            onclick={() => (field = option.value)}
          >
            {option.label}
          </button>
        {/each}
      </div>
    </div>
    <div class="block">
      <span class="label">关键词</span>
      <input
        class="text-input mono"
        type="text"
        bind:value={text}
        placeholder="留空则只用下方条件"
      />
      <div class="toggles">
        <button type="button" class="toggle" class:active={regex} onclick={() => (regex = !regex)}>
          正则
        </button>
        <button
          type="button"
          class="toggle"
          class:active={exclude}
          onclick={() => (exclude = !exclude)}
        >
          排除匹配
        </button>
      </div>
    </div>
    <div class="block">
      <span class="label">标签（空格或逗号分隔，需全部命中）</span>
      <input class="text-input" type="text" bind:value={tags} placeholder="work dev" />
    </div>
    <div class="toggles">
      <button
        type="button"
        class="toggle"
        class:active={onlyExpired}
        onclick={() => (onlyExpired = !onlyExpired)}
      >
        仅过期
      </button>
      <button
        type="button"
        class="toggle"
        class:active={onlyFavorites}
        onclick={() => (onlyFavorites = !onlyFavorites)}
      >
        仅收藏
      </button>
      <button
        type="button"
        class="toggle"
        class:active={requireQualityCheck}
        onclick={() => (requireQualityCheck = !requireQualityCheck)}
      >
        质量检查开启
      </button>
    </div>
  {/snippet}
  {#snippet actions()}
    <button
      class="modal-button"
      onclick={() => {
        onclear();
        onclose();
      }}
    >
      清除筛选
    </button>
    <button class="modal-button" onclick={onclose}>取消</button>
    <button class="modal-button primary" onclick={apply}>应用</button>
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

  .chips,
  .toggles {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chips {
    margin-bottom: 12px;
  }

  .chip,
  .toggle {
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .chip:hover,
  .toggle:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .chip.active,
  .toggle.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }
</style>
