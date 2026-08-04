<script lang="ts">
  import { get } from "svelte/store";
  import type {
    EntryInput,
    EntryPatch,
    VaultEntry,
    VaultGroup,
    CustomField,
    AttachmentInput,
  } from "$lib/types/vault";
  import { appSettings } from "$lib/services/settings";
  import { vault } from "$lib/services/vault";
  import { generatePassword, estimateEntropy, entropyLabel } from "$lib/utils/password";
  import { formatBytes } from "$lib/utils/format";
  import { toDateTimeInput } from "$lib/utils/date";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import { KEEPASS_COLORS, KEEPASS_ICON_CHOICES, keepassIconName } from "$lib/utils/keepass-icons";
  import GroupPicker from "$lib/components/GroupPicker.svelte";

  interface Props {
    mode: "create" | "edit" | "edit-multi";
    groups: VaultGroup[];
    groupUuid: string;
    entry: VaultEntry | null;
    /** All selected entries in batch mode (`mode === "edit-multi"`). */
    entries: VaultEntry[];
    onclose: () => void;
    onsaved: (input: EntryInput | null, patch: EntryPatch | null) => void;
  }

  let { mode, groups, groupUuid, entry, entries, onclose, onsaved }: Props = $props();

  // The dialog is mounted per open (editorOpen), so `mode` never changes
  // during an instance's lifetime; capturing it once is intentional.
  // svelte-ignore state_referenced_locally
  const multi = mode === "edit-multi";
  const initialEntry = (() => entry)();
  const initialGroupUuid = (() => groupUuid)();

  /** Shared value of a field across batch targets, or `null` when the values
   * differ (KeePass's "multiple values" case). */
  function sharedValue(pick: (e: VaultEntry) => string): string | null {
    if (!multi) return pick(initialEntry as VaultEntry) ?? "";
    if (entries.length === 0) return "";
    const first = pick(entries[0]) ?? "";
    return entries.every((e) => (pick(e) ?? "") === first) ? first : null;
  }

  let title = $state(multi ? (sharedValue((e) => e.title) ?? "") : (initialEntry?.title ?? ""));
  let username = $state(
    multi ? (sharedValue((e) => e.username) ?? "") : (initialEntry?.username ?? ""),
  );
  let password = $state("");
  let passwordLoading = $state(false);
  let url = $state(multi ? (sharedValue((e) => e.url) ?? "") : (initialEntry?.url ?? ""));
  let notes = $state(multi ? (sharedValue((e) => e.notes) ?? "") : (initialEntry?.notes ?? ""));
  let totp = $state("");
  let totpLoading = $state(false);
  let expiresLocal = $state(
    multi
      ? (sharedValue((e) => e.expires ?? "") ?? "")
      : initialEntry?.expires
        ? toDateTimeInput(initialEntry.expires)
        : "",
  );
  let iconIndex = $state<number | null>(multi ? null : (initialEntry?.icon ?? null));
  /** Whether the user clicked an icon in single mode; untouched means the
   * backend keeps the entry's current icon (custom favicons survive
   * content-only edits). */
  let iconTouched = $state(false);
  /** Data URL of the entry's database custom icon (web favicon), if any.
   * Only single mode renders it, as the first icon option. */
  const customIconUrl =
    !multi && initialEntry?.customIcon
      ? (get(vault)?.customIcons ?? {})[initialEntry.customIcon]
      : undefined;
  /** Whether the custom favicon option is the active selection. */
  let customIconSelected = $state(!!customIconUrl);
  /** Header icon follows the entry being edited (and live picks). In create
   * and batch mode no entry icon exists, so the generic key glyph is shown. */
  const headerIconName = $derived(keepassIconName(iconIndex ?? initialEntry?.icon ?? -1));
  let colorHex = $state(multi ? "" : (initialEntry?.color ?? ""));
  let targetGroupUuid = $state(initialEntry?.groupUuid ?? initialGroupUuid);
  let activeTab = $state<"fields" | "meta" | "custom" | "attachments">("fields");
  let showPassword = $state(false);
  let customFields = $state<CustomField[]>(
    initialEntry?.customFields?.map((f) => ({ ...f })) ?? [],
  );
  /** Editor-local attachment state: `size` is display-only (shown in the UI)
   * and stripped before sending — the backend contract has no `size`. */
  type EditorAttachment = { name: string; size: number; data?: string };
  let attachments = $state<EditorAttachment[]>(
    initialEntry?.attachments?.map((a) => ({ name: a.name, size: a.size })) ?? [],
  );
  let fileInputEl: HTMLInputElement | undefined = $state();

  /** In batch mode every optional field starts "untouched": the displayed
   * placeholder (`多个值`) is not the real value, and a field is applied to
   * all targets only once the user edits it (KeePass multi-edit semantics). */
  const untouched = $state(new Set<string>());
  if (multi) {
    for (const key of [
      "title",
      "username",
      "password",
      "url",
      "notes",
      "totp",
      "expires",
      "icon",
      "color",
    ]) {
      untouched.add(key);
    }
  }
  const titleMulti = $derived(multi && sharedValue((e) => e.title) === null);
  const usernameMulti = $derived(multi && sharedValue((e) => e.username) === null);
  const urlMulti = $derived(multi && sharedValue((e) => e.url) === null);
  const notesMulti = $derived(multi && sharedValue((e) => e.notes) === null);
  const expiresMulti = $derived(multi && sharedValue((e) => e.expires ?? "") === null);
  const iconMulti = $derived(multi && new Set(entries.map((e) => e.icon ?? -1)).size > 1);
  const colorMulti = $derived(multi && new Set(entries.map((e) => e.color ?? "")).size > 1);

  function markTouched(key: string): void {
    untouched.delete(key);
  }

  /** In the Tauri runtime the current password must be fetched on demand.
   * Batch mode never loads passwords (KeePass shows `<multiple values>`). */
  $effect(() => {
    if (multi) return;
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

  /** TOTP seeds are not part of the snapshot; fetch on demand when editing.
   * Batch mode never loads seeds. */
  $effect(() => {
    if (multi) return;
    const targetUuid = entry?.uuid;
    totp = "";
    if (!targetUuid || !entry?.hasTotp) return;
    totpLoading = true;
    void vault
      .getEntryTotp(targetUuid)
      .then((value) => {
        totp = value ?? "";
        totpLoading = false;
      })
      .catch(() => {
        totpLoading = false;
      });
  });

  const entropy = $derived(estimateEntropy(password));
  const strength = $derived(entropyLabel(entropy));

  function generate(): void {
    const settings = get(appSettings);
    password = generatePassword(settings.database.generator);
    showPassword = true;
    markTouched("password");
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

  function pickIcon(index: number): void {
    markTouched("icon");
    iconTouched = true;
    if (customIconUrl && iconIndex === null) customIconSelected = false;
    iconIndex = iconIndex === index ? null : index;
  }

  function pickCustomIcon(): void {
    markTouched("icon");
    iconTouched = true;
    iconIndex = null;
    customIconSelected = true;
  }

  function pickColor(color: string): void {
    markTouched("color");
    colorHex = colorHex.toUpperCase() === color ? "" : color;
  }

  function clearColor(): void {
    markTouched("color");
    colorHex = "";
  }

  function submit(): void {
    if (multi) {
      const patch: EntryPatch = {};
      if (!untouched.has("title")) patch.title = title.trim();
      if (!untouched.has("username")) patch.username = username.trim();
      if (!untouched.has("password")) patch.password = password;
      if (!untouched.has("url")) patch.url = url.trim();
      if (!untouched.has("notes")) patch.notes = notes;
      if (!untouched.has("totp")) patch.totp = totp.trim() || "";
      if (!untouched.has("expires")) {
        if (expiresLocal) patch.expires = new Date(expiresLocal).toISOString();
        else patch.clearExpires = true;
      }
      if (!untouched.has("icon")) {
        if (iconIndex !== null) patch.icon = iconIndex;
        else patch.clearIcon = true;
      }
      if (!untouched.has("color")) {
        if (colorHex) patch.color = colorHex;
        else patch.clearColor = true;
      }
      if (Object.keys(patch).length === 0) {
        onclose();
        return;
      }
      onsaved(null, patch);
      return;
    }
    if (!title.trim() && !username.trim() && !password) return;
    // Tri-state icon: untouched = key omitted (backend keeps the current
    // icon); a picked index sets the built-in; `null` resets to default.
    const iconValue: number | null | undefined = !iconTouched
      ? undefined
      : iconIndex !== null
        ? iconIndex
        : customIconSelected
          ? undefined
          : null;
    onsaved(
      {
        groupUuid: targetGroupUuid,
        title: title.trim(),
        username: username.trim(),
        password,
        url: url.trim(),
        notes,
        totp: totp.trim() || undefined,
        expires: expiresLocal ? new Date(expiresLocal).toISOString() : undefined,
        ...(iconValue !== undefined ? { icon: iconValue } : {}),
        color: colorHex || undefined,
        customFields: customFields
          .map((f) => ({ name: f.name.trim(), value: f.value }))
          .filter((f) => f.name !== ""),
        attachments: attachments.map((a) =>
          a.data ? { name: a.name, data: a.data } : { name: a.name },
        ),
      },
      null,
    );
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
      <span class="modal-icon"
        >{#if customIconSelected && customIconUrl}
          <img class="modal-icon-img" src={customIconUrl} alt="" draggable="false" />
        {:else}
          <AppIcon name={headerIconName} size={18} />
        {/if}</span
      >
      <div>
        <strong
          >{mode === "create"
            ? "新建条目"
            : multi
              ? `批量编辑 ${entries.length} 个条目`
              : "编辑条目"}</strong
        >
        <p>
          {mode === "create"
            ? "在当前分组创建新条目"
            : multi
              ? "修改应用到所有选中条目,未修改的字段保持不变"
              : "保存对条目的修改"}
        </p>
      </div>
    </div>

    <div class="editor-tabs" role="tablist" aria-label="条目字段分组">
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "fields"}
        aria-selected={activeTab === "fields"}
        onclick={() => (activeTab = "fields")}
      >
        字段
      </button>
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "meta"}
        aria-selected={activeTab === "meta"}
        onclick={() => (activeTab = "meta")}
      >
        元属性
      </button>
      {#if !multi}
        <button
          type="button"
          role="tab"
          class="editor-tab"
          class:active={activeTab === "custom"}
          aria-selected={activeTab === "custom"}
          onclick={() => (activeTab = "custom")}
        >
          自定义字段{#if customFields.length}({customFields.length}){/if}
        </button>
        <button
          type="button"
          role="tab"
          class="editor-tab"
          class:active={activeTab === "attachments"}
          aria-selected={activeTab === "attachments"}
          onclick={() => (activeTab = "attachments")}
        >
          附件{#if attachments.length}({attachments.length}){/if}
        </button>
      {/if}
    </div>

    {#if activeTab === "fields"}
      <div class="form-grid" role="tabpanel">
        <label class="field">
          <span>标题</span>
          <input
            class="text-input"
            type="text"
            bind:value={title}
            placeholder={titleMulti ? "多个值" : "例如：GitHub"}
            oninput={() => markTouched("title")}
          />
        </label>

        {#if !multi}
          <label class="field">
            <span>分组</span>
            <GroupPicker
              {groups}
              value={targetGroupUuid}
              onchange={(uuid) => (targetGroupUuid = uuid)}
            />
          </label>
        {/if}

        <label class="field">
          <span>用户名</span>
          <input
            class="text-input"
            type="text"
            bind:value={username}
            autocomplete="off"
            placeholder={usernameMulti ? "多个值" : undefined}
            oninput={() => markTouched("username")}
          />
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
              placeholder={multi ? "多个值" : passwordLoading ? "加载中…" : ""}
              oninput={() => markTouched("password")}
            />
            <button class="icon-btn" onclick={generate} title="生成密码">
              <AppIcon name="refresh" size={14} />
            </button>
            <button
              class="icon-btn"
              onclick={() => (showPassword = !showPassword)}
              title="显示密码"
            >
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
            <span class="strength-label {strength.className}"
              >{strength.label} · {entropy} bits</span
            >
          </div>
        </label>

        <label class="field full">
          <span>网址</span>
          <input
            class="text-input"
            type="url"
            bind:value={url}
            placeholder={urlMulti ? "多个值" : "https://"}
            oninput={() => markTouched("url")}
          />
        </label>

        <label class="field full">
          <span>备注</span>
          <textarea
            class="text-input textarea"
            bind:value={notes}
            rows={4}
            placeholder={notesMulti ? "多个值" : undefined}
            oninput={() => markTouched("notes")}></textarea>
        </label>
      </div>
    {/if}

    {#if activeTab === "meta"}
      <div class="form-grid" role="tabpanel">
        <label class="field">
          <span>TOTP 种子</span>
          <input
            class="text-input mono"
            type="text"
            bind:value={totp}
            placeholder={multi ? "多个值" : totpLoading ? "正在加载…" : "Base32 密钥或 otpauth URI"}
            disabled={totpLoading}
            oninput={() => markTouched("totp")}
          />
          {#if multi}
            <span class="field-hint">输入新种子将替换所有选中条目的 TOTP</span>
          {/if}
        </label>

        <label class="field">
          <span>过期时间</span>
          <input
            class="text-input"
            type="datetime-local"
            bind:value={expiresLocal}
            placeholder={expiresMulti ? "多个值" : undefined}
            oninput={() => markTouched("expires")}
          />
          <span class="field-hint"
            >{multi ? "清空并保存将移除所有选中条目的过期时间" : "到期后条目标记为已过期"}</span
          >
        </label>

        <section class="field full">
          <span class="section-title">图标</span>
          <div class="icon-grid">
            {#if !multi && customIconUrl}
              <button
                type="button"
                class="icon-option"
                class:selected={customIconSelected}
                onclick={pickCustomIcon}
                title="自定义图标(网页图标)"
                aria-pressed={customIconSelected}
              >
                <img class="icon-option-img" src={customIconUrl} alt="" draggable="false" />
              </button>
            {/if}
            {#each KEEPASS_ICON_CHOICES as index}
              <button
                type="button"
                class="icon-option"
                class:selected={!multi && iconIndex === index}
                onclick={() => pickIcon(index)}
                title={multi && iconMulti ? `多个值 → ${index}` : `内置图标 ${index}`}
                aria-pressed={iconIndex === index}
              >
                <AppIcon name={keepassIconName(index)} size={16} />
              </button>
            {/each}
          </div>
          {#if multi}
            <span class="field-hint">点击图标将应用到所有选中条目;未点击则保持不变</span>
          {/if}
        </section>

        <section class="field full">
          <span class="section-title">颜色标记</span>
          <div class="color-row">
            {#each KEEPASS_COLORS as color (color)}
              <button
                type="button"
                class="color-option"
                class:selected={!multi && colorHex.toUpperCase() === color}
                style:background={color}
                onclick={() => pickColor(color)}
                title={color}
                aria-label={`颜色 ${color}`}
              ></button>
            {/each}
            <input
              class="color-input"
              type="color"
              value={colorHex || "#000000"}
              oninput={(e) => {
                markTouched("color");
                colorHex = e.currentTarget.value.toUpperCase();
              }}
              title="自定义颜色"
            />
            {#if colorHex}
              <button type="button" class="icon-btn" onclick={clearColor} title="清除颜色">
                <AppIcon name="x" size={13} />
              </button>
            {/if}
          </div>
          {#if multi}
            <span class="field-hint">选择颜色将应用到所有选中条目;未选择则保持不变</span>
          {/if}
        </section>
      </div>
    {/if}

    {#if !multi && activeTab === "custom"}
      <div class="form-grid" role="tabpanel">
        <section class="field full">
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
      </div>
    {/if}

    {#if !multi && activeTab === "attachments"}
      <div class="form-grid" role="tabpanel">
        <section class="field full">
          {#if attachments.length === 0}
            <p class="section-empty">暂无附件</p>
          {/if}
          {#each attachments as attachment, i (attachment.name + i)}
            <div class="attachment-row">
              <AppIcon name="file" size={14} />
              <span class="attachment-name" title={attachment.name}>{attachment.name}</span>
              <span class="attachment-size">{formatBytes(attachment.size)}</span>
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
      </div>
    {/if}

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
    width: min(500px, calc(100% - 40px));
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

  .modal-icon-img {
    width: 16px;
    height: 16px;
    object-fit: contain;
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

  .editor-tabs {
    display: flex;
    gap: 2px;
    margin: -12px 0 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .editor-tab {
    padding: 5px 12px;
    border: 0;
    border-bottom: 2px solid transparent;
    border-radius: var(--settings-control-radius, 6px) var(--settings-control-radius, 6px) 0 0;
    background: transparent;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
    transition:
      color 80ms ease,
      border-color 80ms ease;
  }

  .editor-tab:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .editor-tab.active {
    color: var(--text-primary);
    border-bottom-color: var(--selection-color);
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

  .icon-grid {
    display: grid;
    grid-template-columns: repeat(9, 1fr);
    gap: 4px;
  }

  .icon-option {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 30px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .icon-option-img {
    width: 16px;
    height: 16px;
    object-fit: contain;
  }

  .icon-option:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .icon-option.selected {
    color: var(--accent-color, var(--primary-color));
    border-color: color-mix(in srgb, var(--primary-color) 55%, transparent);
    background: color-mix(in srgb, var(--primary-color) 12%, transparent);
  }

  .color-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .color-option {
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    cursor: pointer;
  }

  .color-option.selected {
    border-color: var(--text-primary);
    box-shadow: 0 0 0 2px var(--hover-bg) inset;
  }

  .color-input {
    width: 32px;
    height: 24px;
    padding: 1px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    cursor: pointer;
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
