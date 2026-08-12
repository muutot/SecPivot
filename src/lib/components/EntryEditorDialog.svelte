<script lang="ts">
  import { get } from "svelte/store";
  import type {
    EntryInput,
    EntryPatch,
    EntryFlags,
    VaultEntry,
    VaultGroup,
    CustomField,
    AttachmentInput,
    EntryAutoTypeConfig,
  } from "$lib/types/vault";
  import { appSettings } from "$lib/services/settings";
  import { vault } from "$lib/services/vault";
  import { generatePassword, estimateEntropy, entropyLabel } from "$lib/utils/password";
  import { formatBytes } from "$lib/utils/format";
  import { toDateTimeInput } from "$lib/utils/date";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import { shortestMatchable } from "$lib/utils/match-url";
  import { KEEPASS_COLORS, KEEPASS_ICON_CHOICES, keepassIconName } from "$lib/utils/keepass-icons";
  import GroupPicker from "$lib/components/GroupPicker.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";

  interface Props {
    mode: "create" | "edit" | "edit-multi";
    groups: VaultGroup[];
    groupUuid: string;
    entry: VaultEntry | null;
    /** All selected entries in batch mode (`mode === "edit-multi"`). */
    entries: VaultEntry[];
    onclose: () => void;
    onsaved: (
      input: EntryInput | null,
      patch: EntryPatch | null,
      autotype: EntryAutoTypeConfig | null,
      flags?: EntryFlags | null,
    ) => Promise<void> | void;
  }

  let { mode, groups, groupUuid, entry, entries, onclose, onsaved }: Props = $props();

  // The dialog is mounted per open (editorOpen), so `mode` never changes
  // during an instance's lifetime; capturing it once is intentional.
  // svelte-ignore state_referenced_locally
  const multi = mode === "edit-multi";
  const initialEntry = (() => entry)();
  const initialGroupUuid = (() => groupUuid)();

  /** Guards against double-submit: the button disables while a save is in
   *  flight, and re-submits are ignored until the previous attempt settles. */
  let saving = $state(false);
  async function runSave(fire: () => Promise<void> | void): Promise<void> {
    if (saving) return;
    saving = true;
    try {
      await fire();
    } finally {
      saving = false;
    }
  }

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
  /** Whether the current password is known. In single edit mode the value is
   * fetched on demand; saving before it settles would re-encrypt the entry
   * with an empty password (a data-loss wipe). True only once the load
   * succeeds (or there is nothing to preserve, e.g. create mode). */
  let passwordReady = $state(multi);
  let url = $state(multi ? (sharedValue((e) => e.url) ?? "") : (initialEntry?.url ?? ""));
  let notes = $state(multi ? (sharedValue((e) => e.notes) ?? "") : (initialEntry?.notes ?? ""));
  let tags = $state(multi ? (sharedValue((e) => e.tags ?? "") ?? "") : (initialEntry?.tags ?? ""));
  let overrideUrl = $state(multi ? "" : (initialEntry?.overrideUrl ?? ""));
  let qualityCheck = $state(multi ? true : (initialEntry?.qualityCheck ?? true));
  let foregroundHex = $state(multi ? "" : (initialEntry?.foregroundColor ?? ""));
  let totp = $state("");
  let totpLoading = $state(false);
  /** Whether the current TOTP seed is known. Same data-loss reasoning as
   * `passwordReady`: saving while the seed load is pending sends `totp:
   * undefined`, which the backend interprets as "remove the seed". */
  let totpReady = $state(multi);
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
  let activeTab = $state<"fields" | "meta" | "autotype" | "keyvault" | "custom" | "attachments">(
    "fields",
  );
  let showPassword = $state(false);
  let customFields = $state<CustomField[]>(
    initialEntry?.customFields?.map((f) => ({ ...f })) ?? [],
  );
  /** Protected custom-field values are absent from `VaultEntry` snapshots, so
   * editing an existing entry must fetch them on demand. Saving before they
   * settle would overwrite protected values with empty strings (data-loss);
   * the editor gates the save until every protected value is loaded. */
  let protectedFieldsReady = $state(!initialEntry);
  let protectedFieldsLoading = $state(false);
  /** Indices of custom fields whose value input is currently revealed while
   * protected (password-type masking is only a display affordance; the actual
   * value is never part of the snapshot in the Tauri runtime). */
  let revealedCustomFields = $state<Set<number>>(new Set());
  /** Editor-local attachment state: `size` is display-only (shown in the UI)
   * and stripped before sending — the backend contract has no `size`. */
  type EditorAttachment = { name: string; size: number; data?: string };
  let attachments = $state<EditorAttachment[]>(
    initialEntry?.attachments?.map((a) => ({ name: a.name, size: a.size })) ?? [],
  );
  let fileInputEl: HTMLInputElement | undefined = $state();
  /** Entry Auto-Type editor state (single mode only; batch edits never touch it). */
  let autoTypeEnabled = $state(multi ? true : (initialEntry?.autoType?.enabled ?? true));
  let autoTypeDefaultSeq = $state(multi ? "" : (initialEntry?.autoType?.defaultSequence ?? ""));
  type EditorAssociation = { window: string; sequence: string };
  let autoTypeAssociations = $state<EditorAssociation[]>(
    multi ? [] : (initialEntry?.autoType?.associations ?? []).map((a) => ({ ...a })),
  );

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
      "tags",
    ]) {
      untouched.add(key);
    }
  }
  const titleMulti = $derived(multi && sharedValue((e) => e.title) === null);
  const usernameMulti = $derived(multi && sharedValue((e) => e.username) === null);
  const urlMulti = $derived(multi && sharedValue((e) => e.url) === null);
  const notesMulti = $derived(multi && sharedValue((e) => e.notes) === null);
  const tagsMulti = $derived(multi && sharedValue((e) => e.tags ?? "") === null);
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
    // Create mode (no entry yet): nothing to preserve, so the field is ready.
    if (!targetUuid) {
      passwordReady = true;
      return;
    }
    passwordLoading = true;
    passwordReady = false;
    void vault
      .getEntryPassword(targetUuid)
      .then((value) => {
        password = value;
        passwordLoading = false;
        passwordReady = true;
      })
      .catch(() => {
        passwordLoading = false;
        // The value is unknown, so saving must not overwrite it with an empty
        // one; keep the field gated (the save button stays disabled).
        passwordReady = false;
      });
  });

  /** TOTP seeds are not part of the snapshot; fetch on demand when editing.
   * Batch mode never loads seeds. */
  $effect(() => {
    if (multi) return;
    const targetUuid = entry?.uuid;
    totp = "";
    // Create mode: no existing seed to preserve; the empty field is the user's
    // intent (no seed). Entry without a seed: nothing to preserve either.
    if (!targetUuid || !entry?.hasTotp) {
      totpReady = true;
      return;
    }
    totpLoading = true;
    totpReady = false;
    void vault
      .getEntryTotp(targetUuid)
      .then((value) => {
        totp = value ?? "";
        totpLoading = false;
        totpReady = true;
      })
      .catch(() => {
        totpLoading = false;
        // Unknown seed: keep the save gated so it is not wiped.
        totpReady = false;
      });
  });

  const entropy = $derived(estimateEntropy(password));
  const strength = $derived(entropyLabel(entropy));

  /** Names of protected custom fields carried by the original snapshot. Captured
   * from the immutable `initialEntry` (not the mutable editor state) so the
   * load effect below never re-runs on its own writes. */
  const initialProtectedNames = (
    multi || !initialEntry ? [] : (initialEntry.customFields ?? []).filter((f) => f.protected)
  ).map((f) => f.name);
  /** UUID of the entry the protected fields were already loaded for. */
  let protectedFieldsLoadedFor = $state<string | null>(null);

  /** Protected custom-field values are absent from the snapshot; fetch them on
   * demand when editing an existing entry so saving never overwrites them with
   * empty strings. Create mode starts with fresh fields (no values to keep). */
  $effect(() => {
    if (multi) return;
    const targetUuid = entry?.uuid;
    if (!targetUuid || initialProtectedNames.length === 0) {
      protectedFieldsReady = true;
      return;
    }
    if (protectedFieldsLoadedFor === targetUuid) return;
    protectedFieldsLoading = true;
    protectedFieldsReady = false;
    void Promise.all(
      initialProtectedNames.map(async (name) => {
        const value = await vault.getCustomFieldValue(targetUuid, name);
        return { name, value };
      }),
    )
      .then((resolved) => {
        const values = new Map(resolved.map((r) => [r.name, r.value]));
        customFields = customFields.map((f) => {
          if (!f.protected) return f;
          const value = values.get(f.name);
          return value === undefined ? f : { ...f, value: value ?? "" };
        });
        protectedFieldsLoading = false;
        protectedFieldsReady = true;
        protectedFieldsLoadedFor = targetUuid;
      })
      .catch(() => {
        protectedFieldsLoading = false;
        // Unknown values: keep the save gated so protected fields are not wiped.
        protectedFieldsReady = false;
      });
  });

  function generate(): void {
    const settings = get(appSettings);
    password = generatePassword(settings.database.generator);
    showPassword = true;
    markTouched("password");
  }

  function addCustomField(): void {
    customFields = [...customFields, { name: "", value: "", protected: false }];
    revealedCustomFields = new Set();
  }

  function updateCustomField(index: number, patch: Partial<CustomField>): void {
    customFields = customFields.map((f, i) => (i === index ? { ...f, ...patch } : f));
  }

  function toggleCustomFieldProtected(index: number): void {
    const field = customFields[index];
    if (!field) return;
    const nowProtected = !field.protected;
    customFields = customFields.map((f, i) =>
      i === index ? { ...f, protected: nowProtected } : f,
    );
    if (!nowProtected) {
      const next = new Set(revealedCustomFields);
      next.delete(index);
      revealedCustomFields = next;
    }
  }

  function toggleCustomFieldReveal(index: number): void {
    const next = new Set(revealedCustomFields);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    revealedCustomFields = next;
  }

  function removeCustomField(index: number): void {
    customFields = customFields.filter((_, i) => i !== index);
    const next = new Set<number>();
    for (const i of revealedCustomFields) {
      if (i < index) next.add(i);
      else if (i > index) next.add(i - 1);
    }
    revealedCustomFields = next;
  }

  // ---------------------------------------------------------------------------
  // KeyVault (KeePassRPC per-entry match config, stored in `KPRPC JSON`)
  // ---------------------------------------------------------------------------

  const KPRPC_FIELD = "KPRPC JSON";
  type KeyVaultAccuracy = "Exact" | "Hostname" | "Domain";
  type KeyVaultRule = { value: string; regex: boolean; block: boolean };
  /** `KPRPC JSON` is managed by the KeyVault tab, so it is hidden from the
   * custom-fields tab (but stays in `customFields` state so both KeyVault and
   * the backend's `sync_custom_fields` still read/write it). */
  const visibleCustomFields = $derived(customFields.filter((f) => f.name !== KPRPC_FIELD));

  let kvAccuracy = $state<KeyVaultAccuracy>("Domain");
  let kvRules = $state<KeyVaultRule[]>([]);
  /** A protected `KPRPC JSON` field's value is absent from the snapshot, so it
   * cannot be edited structurally (would be wiped as empty). */
  let kprpcProtected = $state(
    multi ||
      !initialEntry ||
      !!initialEntry.customFields?.find((f) => f.name === KPRPC_FIELD)?.protected,
  );

  /** Real URL pasted for identification (from Kee's `FindLogins urls=[...]`). */
  let kvIdentifyInput = $state("");
  /** Shortest-matchable suggestion, empty until identified. */
  let kvIdentifySuggestion = $state("");
  /** Explains the suggestion or an error; `urlHost` of the input is appended. */
  let kvIdentifyMessage = $state("");

  function parseKprpcField(): void {
    const field = customFields.find((f) => f.name === KPRPC_FIELD);
    if (!field) {
      kvAccuracy = "Domain";
      kvRules = [];
      return;
    }
    let config: Record<string, unknown> = {};
    try {
      config = JSON.parse(field.value) as Record<string, unknown>;
    } catch {
      kvAccuracy = "Domain";
      kvRules = [];
      return;
    }
    kvAccuracy = config.blockHostnameOnlyMatch
      ? "Exact"
      : config.blockDomainOnlyMatch
        ? "Hostname"
        : "Domain";
    const list = (key: string): string[] =>
      Array.isArray(config[key])
        ? (config[key] as unknown[]).filter((v): v is string => typeof v === "string")
        : [];
    kvRules = [
      ...list("altURLs").map((value) => ({ value, regex: false, block: false })),
      ...list("regExURLs").map((value) => ({ value, regex: true, block: false })),
      ...list("blockedURLs").map((value) => ({ value, regex: false, block: true })),
      ...list("regExBlockedURLs").map((value) => ({ value, regex: true, block: true })),
    ];
  }

  /** Rebuild the `KPRPC JSON` custom field from the structured tab state so the
   * custom-fields tab and the backend stay in sync with the KeyVault UI. */
  function syncKprpcField(): void {
    if (kprpcProtected) return;
    const trimmed = kvRules.map((r) => r.value.trim()).filter(Boolean).length;
    const empty = trimmed === 0 && kvAccuracy === "Domain";
    if (empty) {
      customFields = customFields.filter((f) => f.name !== KPRPC_FIELD);
      return;
    }
    const config = {
      version: 1,
      altURLs: kvRules
        .filter((r) => !r.regex && !r.block)
        .map((r) => r.value.trim())
        .filter(Boolean),
      regExURLs: kvRules
        .filter((r) => r.regex && !r.block)
        .map((r) => r.value.trim())
        .filter(Boolean),
      blockedURLs: kvRules
        .filter((r) => !r.regex && r.block)
        .map((r) => r.value.trim())
        .filter(Boolean),
      regExBlockedURLs: kvRules
        .filter((r) => r.regex && r.block)
        .map((r) => r.value.trim())
        .filter(Boolean),
      blockHostnameOnlyMatch: kvAccuracy === "Exact",
      blockDomainOnlyMatch: kvAccuracy === "Hostname",
    };
    const json = JSON.stringify(config);
    const index = customFields.findIndex((f) => f.name === KPRPC_FIELD);
    customFields =
      index >= 0
        ? customFields.map((f, i) => (i === index ? { ...f, value: json, protected: false } : f))
        : [...customFields, { name: KPRPC_FIELD, value: json, protected: false }];
  }

  function addKeyVaultRule(): void {
    kvRules = [...kvRules, { value: "", regex: false, block: false }];
    syncKprpcField();
  }

  function updateKeyVaultRule(index: number, patch: Partial<KeyVaultRule>): void {
    kvRules = kvRules.map((r, i) => (i === index ? { ...r, ...patch } : r));
    syncKprpcField();
  }

  function removeKeyVaultRule(index: number): void {
    kvRules = kvRules.filter((_, i) => i !== index);
    syncKprpcField();
  }

  /** Identify the shortest URL that matches the pasted real address under the
   * currently selected accuracy, and append it as a "match" rule. */
  function identifyMatchable(): void {
    const raw = kvIdentifyInput.trim();
    if (!raw) return;
    const result = shortestMatchable(raw, kvAccuracy);
    if (result.startsWith("无法识别")) {
      kvIdentifyMessage = result;
      kvIdentifySuggestion = "";
      return;
    }
    kvIdentifyMessage = "建议地址:";
    kvIdentifySuggestion = result;
  }

  function applyIdentifySuggestion(): void {
    if (!kvIdentifySuggestion) return;
    kvRules = [...kvRules, { value: kvIdentifySuggestion, regex: false, block: false }];
    kvIdentifySuggestion = "";
    kvIdentifyInput = "";
    kvIdentifyMessage = "";
    syncKprpcField();
  }

  function setKeyVaultAccuracy(accuracy: KeyVaultAccuracy): void {
    kvAccuracy = accuracy;
    syncKprpcField();
  }

  /** Switch tabs; re-parse the KeyVault state when entering its tab so raw
   * edits made in the custom-fields tab are reflected structurally. */
  function activateTab(
    tab: "fields" | "meta" | "autotype" | "custom" | "attachments" | "keyvault",
  ): void {
    activeTab = tab;
    if (tab === "keyvault") {
      // Recompute protection from the live field list (it may have been
      // unprotected in the custom tab after the dialog opened).
      kprpcProtected =
        multi || !initialEntry || !!customFields.find((f) => f.name === KPRPC_FIELD)?.protected;
      parseKprpcField();
    }
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
    if (!multi && (!passwordReady || !totpReady || !protectedFieldsReady)) return;
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
      if (!untouched.has("tags")) patch.tags = tags.trim() || "";
      if (Object.keys(patch).length === 0) {
        onclose();
        return;
      }
      void runSave(() => onsaved(null, patch, null));
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
    const autotype: EntryAutoTypeConfig = {
      enabled: autoTypeEnabled,
      defaultSequence: autoTypeDefaultSeq.trim() || undefined,
      associations: autoTypeAssociations
        .map((a) => ({ window: a.window.trim(), sequence: a.sequence }))
        .filter((a) => a.window !== ""),
    };
    void runSave(() =>
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
          tags: tags.trim(),
          customFields: customFields
            .map((f) => ({
              name: f.name.trim(),
              value: f.value,
              protected: f.protected ?? false,
            }))
            .filter((f) => f.name !== ""),
          attachments: attachments.map((a) =>
            a.data ? { name: a.name, data: a.data } : { name: a.name },
          ),
        },
        null,
        autotype,
        { overrideUrl: overrideUrl.trim(), qualityCheck, foregroundColor: foregroundHex },
      ),
    );
  }
</script>

<ModalShell
  title={mode === "create" ? "新建条目" : multi ? `批量编辑 ${entries.length} 个条目` : "编辑条目"}
  description={mode === "create"
    ? "在当前分组创建新条目"
    : multi
      ? "修改应用到所有选中条目,未修改的字段保持不变"
      : "保存对条目的修改"}
  ariaLabel={mode === "create" ? "新建条目" : "编辑条目"}
  size="large"
  scrollable
>
  {#snippet icon()}
    {#if customIconSelected && customIconUrl}
      <img class="modal-icon-img" src={customIconUrl} alt="" draggable="false" />
    {:else}
      <AppIcon name={headerIconName} size={18} />
    {/if}
  {/snippet}
  {#snippet children()}
    <div class="editor-tabs" role="tablist" aria-label="条目字段分组">
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "fields"}
        aria-selected={activeTab === "fields"}
        onclick={() => activateTab("fields")}
      >
        字段
      </button>
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "meta"}
        aria-selected={activeTab === "meta"}
        onclick={() => activateTab("meta")}
      >
        元属性
      </button>
      {#if !multi}
        <button
          type="button"
          role="tab"
          class="editor-tab"
          class:active={activeTab === "autotype"}
          aria-selected={activeTab === "autotype"}
          onclick={() => activateTab("autotype")}
        >
          自动填充
        </button>
        <button
          type="button"
          role="tab"
          class="editor-tab"
          class:active={activeTab === "keyvault"}
          aria-selected={activeTab === "keyvault"}
          onclick={() => activateTab("keyvault")}
        >
          KeyVault
        </button>
        <button
          type="button"
          role="tab"
          class="editor-tab"
          class:active={activeTab === "custom"}
          aria-selected={activeTab === "custom"}
          onclick={() => activateTab("custom")}
        >
          自定义字段{#if visibleCustomFields.length}({visibleCustomFields.length}){/if}
        </button>
        <button
          type="button"
          role="tab"
          class="editor-tab"
          class:active={activeTab === "attachments"}
          aria-selected={activeTab === "attachments"}
          onclick={() => activateTab("attachments")}
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

        <label class="field full">
          <span>标签</span>
          <input
            class="text-input"
            type="text"
            bind:value={tags}
            placeholder={tagsMulti ? "多个值" : "逗号分隔，例如：工作, 邮箱"}
            oninput={() => markTouched("tags")}
          />
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

        {#if !multi}
          <label class="field">
            <span>覆盖 URL（OverrideURL）</span>
            <input
              class="text-input mono"
              type="text"
              bind:value={overrideUrl}
              placeholder="https://real.example"
              oninput={() => markTouched("overrideUrl")}
            />
            <span class="field-hint">仅用于匹配（浏览器桥/RPC/自动填充），不改变显示的网址</span>
          </label>

          <label class="field">
            <span>质量检查</span>
            <div class="flag-row">
              <button
                type="button"
                class="flag-toggle"
                class:active={qualityCheck}
                onclick={() => (qualityCheck = !qualityCheck)}
                aria-pressed={qualityCheck}
              >
                {qualityCheck ? "已启用" : "已禁用"}
              </button>
            </div>
            <span class="field-hint">禁用后条目不参与弱密码安全检查</span>
          </label>

          <section class="field full">
            <span class="section-title">前景色（文字）</span>
            <div class="color-row">
              {#each KEEPASS_COLORS as color (color)}
                <button
                  type="button"
                  class="color-option"
                  class:selected={foregroundHex.toUpperCase() === color}
                  style:background={color}
                  onclick={() =>
                    (foregroundHex = foregroundHex.toUpperCase() === color ? "" : color)}
                  title={color}
                  aria-label={`前景色 ${color}`}
                ></button>
              {/each}
              <input
                class="color-input"
                type="color"
                value={foregroundHex || "#000000"}
                oninput={(e) => (foregroundHex = e.currentTarget.value.toUpperCase())}
                title="自定义前景色"
              />
              {#if foregroundHex}
                <button
                  type="button"
                  class="icon-btn"
                  onclick={() => (foregroundHex = "")}
                  title="清除前景色"
                >
                  <AppIcon name="x" size={13} />
                </button>
              {/if}
            </div>
          </section>
        {/if}
      </div>
    {/if}

    {#if !multi && activeTab === "autotype"}
      <div class="autotype-section" role="tabpanel">
        <div class="autotype-row">
          <span class="autotype-label">启用自动填充</span>
          <button
            type="button"
            class="toggle-switch"
            class:active={autoTypeEnabled}
            role="switch"
            aria-checked={autoTypeEnabled}
            aria-label="启用自动填充"
            onclick={() => (autoTypeEnabled = !autoTypeEnabled)}
          ></button>
        </div>
        <label class="field">
          <span>默认序列</span>
          <input
            class="text-input mono"
            type="text"
            bind:value={autoTypeDefaultSeq}
            placeholder={"{USERNAME}{TAB}{PASSWORD}{ENTER}"}
          />
        </label>
        <span class="autotype-label">窗口关联</span>
        {#each autoTypeAssociations as association, index (index)}
          <div class="association-row">
            <input
              class="text-input association-window"
              type="text"
              bind:value={association.window}
              placeholder="窗口标题（* 通配）"
            />
            <input
              class="text-input mono association-sequence"
              type="text"
              bind:value={association.sequence}
              placeholder="序列，留空用默认"
            />
            <button
              class="association-remove"
              type="button"
              title="删除关联"
              onclick={() => autoTypeAssociations.splice(index, 1)}
            >
              <AppIcon name="x" size={12} />
            </button>
          </div>
        {/each}
        <button
          class="add-row-btn"
          type="button"
          onclick={() =>
            (autoTypeAssociations = [...autoTypeAssociations, { window: "", sequence: "" }])}
        >
          <AppIcon name="plus" size={12} />添加窗口关联
        </button>
      </div>
    {/if}

    {#if !multi && activeTab === "keyvault"}
      <div class="form-grid" role="tabpanel">
        {#if kprpcProtected}
          <section class="field full">
            <p class="section-empty">
              受保护的 `KPRPC JSON` 字段值未载入快照,为避免被清空,无法在此编辑。
            </p>
          </section>
        {:else}
          <section class="field full">
            <span class="section-title">匹配精度</span>
            <div class="kv-accuracy-row">
              <button
                type="button"
                class="kv-accuracy-option"
                class:active={kvAccuracy === "Domain"}
                onclick={() => setKeyVaultAccuracy("Domain")}
                title="域名相同即匹配(默认)"
              >
                域名
              </button>
              <button
                type="button"
                class="kv-accuracy-option"
                class:active={kvAccuracy === "Hostname"}
                onclick={() => setKeyVaultAccuracy("Hostname")}
                title="主机名+端口相同才匹配"
              >
                主机名
              </button>
              <button
                type="button"
                class="kv-accuracy-option"
                class:active={kvAccuracy === "Exact"}
                onclick={() => setKeyVaultAccuracy("Exact")}
                title="完整网址精确匹配"
              >
                精确
              </button>
            </div>
            <span class="field-hint">匹配精度决定一个网址需要多"像"才会命中该条目</span>
          </section>

          <section class="field full">
            <span class="section-title">匹配 / 阻止 网址或正则</span>
            {#if kvRules.length === 0}
              <p class="section-empty">暂无规则;匹配仅使用主「网址」字段</p>
            {/if}
            {#each kvRules as rule, i (i)}
              <div class="kv-rule-row">
                <input
                  class="text-input kv-rule-value"
                  type="text"
                  placeholder="https:// 或正则表达式"
                  value={rule.value}
                  oninput={(e) => updateKeyVaultRule(i, { value: e.currentTarget.value })}
                />
                <button
                  type="button"
                  class="kv-tag"
                  class:active={rule.regex}
                  onclick={() => updateKeyVaultRule(i, { regex: !rule.regex })}
                  title="正则表达式"
                >
                  .*
                </button>
                <button
                  type="button"
                  class="kv-tag"
                  class:active={!rule.block}
                  class:block={rule.block}
                  onclick={() => updateKeyVaultRule(i, { block: !rule.block })}
                  title={rule.block ? "阻止此网址匹配" : "匹配此网址"}
                >
                  {rule.block ? "阻止" : "匹配"}
                </button>
                <button
                  type="button"
                  class="icon-btn"
                  onclick={() => removeKeyVaultRule(i)}
                  aria-label="删除规则"
                  title="删除规则"
                >
                  <AppIcon name="x" size={13} />
                </button>
              </div>
            {/each}
            <button class="add-row-btn" onclick={addKeyVaultRule}>
              <AppIcon name="plus" size={12} />添加网址 / 正则
            </button>
            <span class="field-hint">以 KeePassRPC `KPRPC JSON` 兼容格式存储;阻止规则优先生效</span>
          </section>

          <section class="field full">
            <span class="section-title">识别最短匹配地址</span>
            <div class="kv-identify-row">
              <input
                class="text-input kv-identify-input"
                type="text"
                placeholder="粘贴浏览器里真实网址(如 Kee 日志 FindLogins urls)"
                value={kvIdentifyInput}
                oninput={(e) => (kvIdentifyInput = e.currentTarget.value)}
              />
              <button
                type="button"
                class="kv-identify-btn"
                onclick={identifyMatchable}
                title="按当前匹配精度算出最短仍能命中的地址"
              >
                <AppIcon name="search" size={13} />识别
              </button>
            </div>
            {#if kvIdentifyMessage}
              <div class="kv-identify-result">
                <p class="field-hint kv-identify-msg">
                  {kvIdentifyMessage}
                  {#if kvIdentifySuggestion}
                    <code class="kv-identify-code">{kvIdentifySuggestion}</code>
                  {/if}
                </p>
                {#if kvIdentifySuggestion}
                  <button
                    type="button"
                    class="kv-identify-btn"
                    onclick={applyIdentifySuggestion}
                    title="把识别出的建议地址加入匹配规则"
                  >
                    <AppIcon name="check" size={12} />应用
                  </button>
                {/if}
              </div>
            {/if}
          </section>
        {/if}
      </div>
    {/if}

    {#if !multi && activeTab === "custom"}
      <div class="form-grid" role="tabpanel">
        <section class="field full">
          {#if visibleCustomFields.length === 0}
            <p class="section-empty">暂无自定义字段</p>
          {/if}
          {#each customFields as field, i (i)}
            {#if field.name !== KPRPC_FIELD}
              <div class="custom-field-row">
                <input
                  class="text-input"
                  type="text"
                  placeholder="字段名"
                  value={field.name}
                  oninput={(e) => updateCustomField(i, { name: e.currentTarget.value })}
                />
                <input
                  class="text-input mono"
                  type={field.protected && !revealedCustomFields.has(i) ? "password" : "text"}
                  placeholder="值"
                  value={field.value}
                  disabled={field.protected && protectedFieldsLoading}
                  oninput={(e) => updateCustomField(i, { value: e.currentTarget.value })}
                />
                {#if field.protected}
                  <button
                    class="icon-btn"
                    onclick={() => toggleCustomFieldReveal(i)}
                    aria-label={revealedCustomFields.has(i) ? "隐藏字段值" : "显示字段值"}
                    title={revealedCustomFields.has(i) ? "隐藏字段值" : "显示字段值"}
                  >
                    <AppIcon name={revealedCustomFields.has(i) ? "eye-off" : "eye"} size={13} />
                  </button>
                {/if}
                <button
                  class="icon-btn"
                  class:active={field.protected}
                  onclick={() => toggleCustomFieldProtected(i)}
                  aria-label={field.protected ? "取消保护此字段" : "保护此字段"}
                  title={field.protected ? "受保护 (值不进入快照)" : "保护此字段 (值不进入快照)"}
                >
                  <AppIcon name={field.protected ? "lock" : "unlock"} size={13} />
                </button>
                <button
                  class="icon-btn"
                  onclick={() => removeCustomField(i)}
                  aria-label="删除字段"
                  title="删除字段"
                >
                  <AppIcon name="x" size={13} />
                </button>
              </div>
            {/if}
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
  {/snippet}
  {#snippet actions()}
    <button class="modal-button" onclick={onclose}>取消</button>
    <button
      class="modal-button primary"
      onclick={submit}
      disabled={saving || (!multi && (!passwordReady || !totpReady || !protectedFieldsReady))}
      title={!multi && (!passwordReady || !totpReady || !protectedFieldsReady)
        ? "正在载入敏感字段…"
        : undefined}>保存</button
    >
  {/snippet}
</ModalShell>

<style>
  .modal-icon-img {
    width: 16px;
    height: 16px;
    object-fit: contain;
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

  .text-input.select {
    appearance: none;
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

  .icon-btn.active {
    color: var(--accent-color, var(--primary-color));
    border-color: var(--accent-color, var(--primary-color));
    background: color-mix(in srgb, var(--accent-color, var(--primary-color)) 12%, transparent);
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

  .kv-accuracy-row {
    display: flex;
    gap: 6px;
  }

  .kv-accuracy-option {
    flex: 1;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .kv-accuracy-option:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .kv-accuracy-option.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .kv-rule-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }

  .kv-rule-value {
    flex: 1;
    height: 30px;
    font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
  }

  .kv-tag {
    height: 30px;
    padding: 0 10px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
    cursor: pointer;
  }

  .kv-tag:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .kv-tag.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .kv-tag.block {
    color: var(--danger-color);
    border-color: color-mix(in srgb, var(--danger-color) 55%, transparent);
    background: color-mix(in srgb, var(--danger-color) 12%, transparent);
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

  .autotype-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .autotype-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .autotype-label {
    display: block;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .association-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .association-window {
    flex: 1;
  }

  .association-sequence {
    flex: 1;
  }

  .association-remove {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex: 0 0 auto;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .association-remove:hover {
    color: var(--danger-color);
    background: color-mix(in srgb, var(--danger-color) 10%, transparent);
  }

  .file-input {
    display: none;
  }

  .kv-identify-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .kv-identify-input {
    flex: 1;
  }

  .kv-identify-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--hover-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .kv-identify-btn:hover {
    background: var(--accent-bg, var(--hover-bg));
  }

  .kv-identify-code {
    color: var(--accent-color, var(--text-primary));
    font-family: var(--font-mono, monospace);
  }

  .kv-identify-result {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }

  .kv-identify-msg {
    margin: 0;
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

  .flag-row {
    display: flex;
    align-items: center;
  }

  .flag-toggle {
    height: 28px;
    padding: 0 12px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .flag-toggle:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .flag-toggle.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }
</style>
