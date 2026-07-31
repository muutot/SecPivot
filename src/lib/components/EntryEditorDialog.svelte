<script lang="ts">
  import { get } from "svelte/store";
  import type { EntryInput, VaultEntry, VaultGroup } from "$lib/types/vault";
  import { appSettings } from "$lib/services/settings";
  import { generatePassword, estimateEntropy, entropyLabel } from "$lib/utils/password";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    mode: "create" | "edit";
    groups: VaultGroup[];
    groupUuid: string;
    entry: VaultEntry | null;
    onclose: () => void;
    onsaved: (input: EntryInput) => void;
  }

  let { mode, groups, groupUuid, entry, onclose, onsaved }: Props = $props();

  let title = $state(entry?.title ?? "");
  let username = $state(entry?.username ?? "");
  let password = $state(entry?.password ?? "");
  let url = $state(entry?.url ?? "");
  let notes = $state(entry?.notes ?? "");
  let totp = $state(entry?.totp ?? "");
  let targetGroupUuid = $state(entry?.groupUuid ?? groupUuid);
  let showPassword = $state(false);

  const entries = $derived.by(() => {
    const list: { name: string; uuid: string }[] = [];
    function walk(group: VaultGroup, depth: number): void {
      list.push({ name: `${"　".repeat(depth)}${group.name}`, uuid: group.uuid });
      for (const child of group.children) walk(child, depth + 1);
    }
    for (const group of groups) walk(group, 0);
    return list;
  });

  const entropy = $derived(estimateEntropy(password));
  const strength = $derived(entropyLabel(entropy));

  function generate(): void {
    const settings = get(appSettings);
    password = generatePassword(settings.database.generator);
    showPassword = true;
  }

  function submit(): void {
    if (!title.trim() && !username.trim() && !password) return;
    onsaved({
      groupUuid: targetGroupUuid,
      title: title.trim(),
      username: username.trim(),
      password,
      url: url.trim(),
      notes,
      totp: totp.trim() || undefined,
    });
  }
</script>

<div class="modal-backdrop" role="presentation">
  <div
    class="editor-modal"
    role="dialog"
    aria-modal="true"
    aria-label={mode === "create" ? "新建条目" : "编辑条目"}
  >
    <div class="modal-head">
      <span class="modal-icon"><AppIcon name="key" size={18} /></span>
      <div>
        <strong>{mode === "create" ? "新建条目" : "编辑条目"}</strong>
        <p>{mode === "create" ? "在当前分组创建新条目" : "保存对条目的修改"}</p>
      </div>
    </div>

    <div class="form-grid">
      <label class="field">
        <span>标题</span>
        <input class="text-input" type="text" bind:value={title} placeholder="例如：GitHub" />
      </label>

      <label class="field">
        <span>分组</span>
        <select class="text-input select" bind:value={targetGroupUuid}>
          {#each entries as group (group.uuid)}
            <option value={group.uuid}>{group.name.trim()}</option>
          {/each}
        </select>
      </label>

      <label class="field">
        <span>用户名</span>
        <input class="text-input" type="text" bind:value={username} autocomplete="off" />
      </label>

      <label class="field">
        <span>密码</span>
        <div class="input-row">
          <input
            class="text-input mono"
            type={showPassword ? "text" : "password"}
            bind:value={password}
            autocomplete="new-password"
          />
          <button class="icon-btn" onclick={generate} title="生成密码">
            <AppIcon name="refresh" size={14} />
          </button>
          <button class="icon-btn" onclick={() => (showPassword = !showPassword)} title="显示密码">
            <AppIcon name={showPassword ? "eye-off" : "eye"} size={14} />
          </button>
        </div>
        <div class="strength-row">
          <span class="strength-bar"
            ><span
              class:strong={strength.className === "strong"}
              class:fair={strength.className === "fair"}
              class:weak={strength.className === "weak"}
              style:width={`${Math.min(100, entropy)}%`}
            ></span></span
          >
          <span class="strength-label {strength.className}">{strength.label} · {entropy} bits</span>
        </div>
      </label>

      <label class="field">
        <span>网址</span>
        <input class="text-input" type="url" bind:value={url} placeholder="https://" />
      </label>

      <label class="field">
        <span>TOTP 种子</span>
        <input
          class="text-input mono"
          type="text"
          bind:value={totp}
          placeholder="Base32 密钥或 otpauth URI"
        />
      </label>

      <label class="field full">
        <span>备注</span>
        <textarea class="text-input textarea" bind:value={notes} rows={3}></textarea>
      </label>
    </div>

    <div class="modal-actions">
      <button class="modal-button" onclick={onclose}>取消</button>
      <button class="modal-button primary" onclick={submit}>保存</button>
    </div>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 45%, transparent);
  }

  .editor-modal {
    width: min(460px, calc(100% - 40px));
    max-height: calc(100% - 48px);
    padding: 18px;
    border: 1px solid var(--border-color);
    border-radius: 13px;
    background: var(--surface-bg);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
    overflow: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .modal-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }

  .modal-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--selection-color);
    background: var(--hover-bg);
  }

  .modal-head strong {
    display: block;
    font-size: 13px;
    font-weight: 560;
  }

  .modal-head p {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .field {
    display: block;
    min-width: 0;
  }

  .field.full {
    grid-column: 1 / -1;
  }

  .field > span {
    display: block;
    margin-bottom: 5px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .text-input {
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: 12px;
  }

  .text-input.mono {
    font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
  }

  .text-input.select {
    appearance: none;
  }

  .textarea {
    height: auto;
    padding: 8px 10px;
    line-height: 1.5;
    resize: vertical;
  }

  .input-row {
    display: flex;
    gap: 6px;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .icon-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .strength-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }

  .strength-bar {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--hover-bg);
    overflow: hidden;
  }

  .strength-bar span {
    display: block;
    height: 100%;
    border-radius: 2px;
    transition: width 120ms ease;
  }

  .strength-bar span.weak {
    background: var(--danger-color);
  }

  .strength-bar span.fair {
    background: var(--warning-color);
  }

  .strength-bar span.strong {
    background: var(--success-color);
  }

  .strength-label {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .strength-label.weak {
    color: var(--danger-color);
  }

  .strength-label.fair {
    color: var(--warning-color);
  }

  .strength-label.strong {
    color: var(--success-color);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .modal-button {
    height: 30px;
    padding: 0 14px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: 12px;
    cursor: pointer;
  }

  .modal-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .modal-button.primary {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }
</style>
