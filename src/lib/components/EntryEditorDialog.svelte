<script lang="ts">
  import { get } from "svelte/store";
  import type {
    EntryInput,
    VaultEntry,
    VaultGroup,
    CustomField,
    AttachmentInput,
  } from "$lib/types/vault";
  import { appSettings } from "$lib/services/settings";
  import { vault } from "$lib/services/vault";
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
  let password = $state("");
  let passwordLoading = $state(false);
  let url = $state(entry?.url ?? "");
  let notes = $state(entry?.notes ?? "");
  let totp = $state(entry?.totp ?? "");
  let expiresLocal = $state(entry?.expires ? toLocalInput(entry.expires) : "");
  let targetGroupUuid = $state(entry?.groupUuid ?? groupUuid);
  let showPassword = $state(false);
  let customFields = $state<CustomField[]>(entry?.customFields?.map((f) => ({ ...f })) ?? []);
  let attachments = $state<AttachmentInput[]>(
    entry?.attachments?.map((a) => ({ name: a.name, size: a.size })) ?? [],
  );
  let fileInputEl: HTMLInputElement | undefined = $state();

  /** In the Tauri runtime the current password must be fetched on demand. */
  $effect(() => {
    const targetUuid = entry?.uuid;
    password = "";
    if (!targetUuid) return;
    passwordLoading = true;
    void vault
      .getEntryPassword(targetUuid)
      .then((value) => {
        password = value;
        passwordLoading = false;
      })
      .catch(() => {
        passwordLoading = false;
      });
  });

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

  function addCustomField(): void {
    customFields = [...customFields, { name: "", value: "" }];
  }

  function updateCustomField(index: number, patch: Partial<CustomField>): void {
    customFields = customFields.map((f, i) => (i === index ? { ...f, ...patch } : f));
  }

  function removeCustomField(index: number): void {
    customFields = customFields.filter((_, i) => i !== index);
  }

  function pickFiles(): void {
    fileInputEl?.click();
  }

  function readFileAsBase64(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(",")[1] ?? "");
      reader.onerror = () => reject(new Error("读取文件失败"));
      reader.readAsDataURL(file);
    });
  }

  async function handleFiles(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const files = input.files ? Array.from(input.files) : [];
    input.value = "";
    if (!files.length) return;
    for (const file of files) {
      const data = await readFileAsBase64(file);
      attachments = [...attachments, { name: file.name, size: file.size, data }];
    }
  }

  function removeAttachment(index: number): void {
    attachments = attachments.filter((_, i) => i !== index);
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  /** Convert an ISO-8601 UTC timestamp to the `datetime-local` input format. */
  function toLocalInput(iso: string): string {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return "";
    const pad = (n: number): string => String(n).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
      date.getHours(),
    )}:${pad(date.getMinutes())}`;
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
      expires: expiresLocal ? new Date(expiresLocal).toISOString() : undefined,
      customFields: customFields
        .map((f) => ({ name: f.name.trim(), value: f.value }))
        .filter((f) => f.name !== ""),
      attachments: attachments.map((a) =>
        a.data ? { name: a.name, size: a.size, data: a.data } : { name: a.name, size: a.size },
      ),
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
            disabled={passwordLoading}
            placeholder={passwordLoading ? "加载中…" : ""}
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

      <label class="field">
        <span>过期时间</span>
        <input class="text-input" type="datetime-local" bind:value={expiresLocal} />
        <span class="field-hint">到期后条目标记为已过期</span>
      </label>

      <section class="field full">
        <span class="section-title">自定义字段</span>
        {#if customFields.length === 0}
          <p class="section-empty">暂无自定义字段</p>
        {/if}
        {#each customFields as field, i (i)}
          <div class="custom-field-row">
            <input
              class="text-input"
              type="text"
              placeholder="字段名"
              value={field.name}
              oninput={(e) => updateCustomField(i, { name: e.currentTarget.value })}
            />
            <input
              class="text-input"
              type="text"
              placeholder="值"
              value={field.value}
              oninput={(e) => updateCustomField(i, { value: e.currentTarget.value })}
            />
            <button
              class="icon-btn"
              onclick={() => removeCustomField(i)}
              aria-label="删除字段"
              title="删除字段"
            >
              <AppIcon name="x" size={13} />
            </button>
          </div>
        {/each}
        <button class="add-row-btn" onclick={addCustomField}>
          <AppIcon name="plus" size={12} />添加字段
        </button>
      </section>

      <section class="field full">
        <span class="section-title">附件</span>
        {#if attachments.length === 0}
          <p class="section-empty">暂无附件</p>
        {/if}
        {#each attachments as attachment, i (attachment.name + i)}
          <div class="attachment-row">
            <AppIcon name="file" size={14} />
            <span class="attachment-name" title={attachment.name}>{attachment.name}</span>
            <span class="attachment-size">{formatSize(attachment.size)}</span>
            <button
              class="icon-btn"
              onclick={() => removeAttachment(i)}
              aria-label="移除附件"
              title="移除附件"
            >
              <AppIcon name="x" size={13} />
            </button>
          </div>
        {/each}
        <button class="add-row-btn" onclick={pickFiles}>
          <AppIcon name="upload" size={12} />添加附件
        </button>
        <input
          class="file-input"
          type="file"
          multiple
          bind:this={fileInputEl}
          onchange={handleFiles}
          tabindex="-1"
          aria-hidden="true"
        />
      </section>

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

  .field-hint {
    display: block;
    margin-top: 4px;
    color: var(--text-faint);
    font-size: 10px;
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

  .section-title {
    display: block;
    margin-bottom: 5px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    font-weight: 560;
  }

  .section-empty {
    margin: 0 0 8px;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .custom-field-row {
    display: flex;
    gap: 6px;
    margin-bottom: 6px;
  }

  .custom-field-row .text-input:first-child {
    flex: 0 0 42%;
  }

  .custom-field-row .text-input {
    flex: 1;
  }

  .attachment-row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px;
    margin-bottom: 6px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .attachment-name {
    flex: 1;
    min-width: 0;
    color: var(--text-primary);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .attachment-size {
    flex: 0 0 auto;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .add-row-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 10px;
    border: 1px dashed var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .add-row-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .file-input {
    display: none;
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
