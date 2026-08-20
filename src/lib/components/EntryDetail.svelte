<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import type { VaultEntry } from "$lib/types/vault";
  import type { HistoryVersion } from "$lib/types/vault";
  import type { EntryPatch } from "$lib/types/vault";
  import type { EntryStorage } from "$lib/types/vault";
  import type { AttachmentInput } from "$lib/types/vault";
  import { copyValue } from "$lib/services/security";
  import { showTip } from "$lib/services/tips";
  import { formatBytes } from "$lib/utils/format";
  import { formatLocalDate } from "$lib/utils/date";
  import { classifyContact, linkifyContacts, type ContactKind } from "$lib/utils/contact";
  import { isTauriRuntime } from "$lib/services/settings";
  import { vault } from "$lib/services/vault";
  import {
    awaitCurrentView,
    canToggleSecretReveal,
    consumeCurrentView,
    KeyedViewGuard,
  } from "$lib/utils/session-state";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import { keepassIconName } from "$lib/utils/keepass-icons";
  import TotpWidget from "$lib/components/TotpWidget.svelte";
  import HistoryVersionDialog from "$lib/components/HistoryVersionDialog.svelte";
  import AttachmentPreviewDialog from "$lib/components/AttachmentPreviewDialog.svelte";

  interface Props {
    entry: VaultEntry;
    groupPath: string;
    inRecycleBin?: boolean;
    onfavorite: (entry: VaultEntry) => void;
    onedit: (entry: VaultEntry) => void;
    ondelete: (entry: VaultEntry) => void;
    onrestore?: (entry: VaultEntry) => void;
    /** Optional back affordance for narrow/mobile layouts where the detail
     *  pane is a full-screen overlay; hidden on wide desktop layouts. */
    onback?: () => void;
  }

  let {
    entry,
    groupPath,
    inRecycleBin = false,
    onfavorite,
    onedit,
    ondelete,
    onrestore,
    onback,
  }: Props = $props();

  const iconName = $derived(keepassIconName(entry.icon));

  /** Data URL of the database-stored custom icon (favicon), if any. */
  const customIconUrl = $derived(
    entry.customIcon ? $vault?.customIcons?.[entry.customIcon] : undefined,
  );

  let revealPassword = $state(false);
  let fetchedPassword = $state("");
  let passwordLoaded = $state(false);
  let passwordLoading = $state(false);
  /** Revealed/loaded protected custom-field values, keyed by field name.
   * Protected values are absent from `VaultEntry`; they are fetched on demand
   * for reveal and copy, exactly like the password. */
  let customFieldValues = $state<Record<string, string>>({});
  let customFieldLoaded = $state<Record<string, boolean>>({});
  let customFieldLoading = $state<Record<string, boolean>>({});
  let customFieldRevealed = $state<Record<string, boolean>>({});
  /** Inline field editor: at most one field is edited at a time. `kind`
   *  selects which stored field to persist; `name`/`protected` qualify
   *  custom fields. Protected fields must be revealed before editing. */
  type EditTarget = {
    kind: "username" | "password" | "url" | "custom";
    name?: string;
    protected?: boolean;
  };
  let editing = $state<EditTarget | null>(null);
  let editValue = $state("");
  let editSaving = $state(false);
  let editInput = $state<HTMLInputElement | null>(null);
  /** Live editable notes draft; persisted debounced on input and on blur.
   *  Initial value captured once; the reset `$effect` resyncs on entry change. */
  // svelte-ignore state_referenced_locally
  let notesDraft = $state(entry.notes ?? "");
  /** Notes editor mode: the read view renders inline clickable links, clicking
   *  anywhere else switches to the textarea editor (debounced auto-save). */
  let notesEditing = $state(false);
  let notesTextareaEl = $state<HTMLTextAreaElement | null>(null);
  let notesViewEl = $state<HTMLDivElement | null>(null);
  let fieldsLayoutEl = $state<HTMLDivElement | null>(null);
  let notesResizeObserver: ResizeObserver | undefined;
  let notesDirty = $state(false);
  let notesSaving = $state(false);
  let notesSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let notesSaveVersion = 0;
  let activeTab = $state<"fields" | "meta" | "attachments" | "history">("fields");
  let historyVersions = $state<HistoryVersion[]>([]);
  let historyLoading = $state(false);
  let historyLoadedUuid = $state<string | null>(null);
  let viewingVersion = $state<HistoryVersion | null>(null);
  let storage = $state<EntryStorage | null>(null);
  let storageLoading = $state(false);
  let previewAttachmentName = $state<string | null>(null);
  /** Highlight + drop-target state for drag-and-drop attachment add. */
  let attachmentDragActive = $state(false);
  /** Nesting counter for `dragenter`/`dragleave` so moving between the
   *  dropzone's own children does not flicker the highlight. */
  let attachmentDragDepth = 0;
  const detailView = new KeyedViewGuard();

  function detailSessionId(): string | null {
    return vault.getActiveSessionId();
  }

  /** Identity of the currently shown entry (session + UUID). Keying the reset
   *  effect on this string prevents wholesale snapshot replacement (saves,
   *  favorite toggles, remote refresh) from re-running the reset and clobbering
   *  an uncommitted inline draft; the value is stable across such replacements. */
  const detailKey = $derived.by(() => {
    const sessionId = detailSessionId();
    return sessionId ? `${sessionId}:${entry.uuid}` : null;
  });

  $effect(() => {
    detailView.activate(detailKey);
    untrack(() => {
      revealPassword = false;
      passwordLoaded = false;
      passwordLoading = false;
      fetchedPassword = "";
      notesDraft = entry.notes ?? "";
      notesEditing = false;
      notesDirty = false;
      notesSaving = false;
      if (notesSaveTimer) {
        clearTimeout(notesSaveTimer);
        notesSaveTimer = undefined;
      }
      customFieldValues = {};
      customFieldLoaded = {};
      customFieldLoading = {};
      customFieldRevealed = {};
      editing = null;
      editSaving = false;
      historyLoadedUuid = null;
      historyLoading = false;
      historyVersions = [];
      viewingVersion = null;
      storage = null;
      storageLoading = false;
      previewAttachmentName = null;
      attachmentDragActive = false;
      attachmentDragDepth = 0;
    });
  });

  onDestroy(() => {
    detailView.activate(null);
    if (notesSaveTimer) {
      clearTimeout(notesSaveTimer);
      notesSaveTimer = undefined;
    }
  });

  async function loadStorage(): Promise<void> {
    if (storageLoading || storage) return;
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    const view = detailView.capture();
    if (!sessionId || !view) return;
    storageLoading = true;
    try {
      const result = await vault.callInSession(sessionId, () => vault.getEntryStorage(uuid));
      if (detailView.isCurrent(view)) storage = result;
    } finally {
      if (detailView.isCurrent(view)) storageLoading = false;
    }
  }

  async function loadHistory(force = false): Promise<void> {
    if (!force && historyLoadedUuid === entry.uuid) return;
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    const view = detailView.capture();
    if (!sessionId || !view) return;
    historyLoading = true;
    try {
      const result = await vault.callInSession(sessionId, () => vault.getEntryHistory(uuid));
      if (detailView.isCurrent(view)) {
        historyVersions = result;
        historyLoadedUuid = uuid;
      }
    } finally {
      if (detailView.isCurrent(view)) historyLoading = false;
    }
  }

  async function restoreVersion(version: HistoryVersion): Promise<void> {
    const when = version.modified ? new Date(version.modified).toLocaleString("zh-CN") : "未知时间";
    if (!window.confirm(`确定恢复到 ${when} 的版本吗？当前内容会保留为新的历史记录。`)) return;
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    const view = detailView.capture();
    if (!sessionId || !view) return;
    try {
      await vault.callInSession(sessionId, () => vault.restoreEntryVersion(uuid, version.index));
      if (!detailView.isCurrent(view)) return;
      historyLoadedUuid = null;
      await loadHistory(true);
      if (!detailView.isCurrent(view)) return;
      flash("restored");
    } catch {
      if (detailView.isCurrent(view)) flash("error");
    }
  }

  async function deleteVersion(version: HistoryVersion): Promise<void> {
    const when = version.modified ? new Date(version.modified).toLocaleString("zh-CN") : "未知时间";
    if (!window.confirm(`确定删除 ${when} 的历史版本吗？此操作无法撤销。`)) return;
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    const view = detailView.capture();
    if (!sessionId || !view) return;
    try {
      await vault.callInSession(sessionId, () => vault.deleteEntryHistory(uuid, version.index));
      if (!detailView.isCurrent(view)) return;
      historyLoadedUuid = null;
      await loadHistory(true);
      if (!detailView.isCurrent(view)) return;
      flash("deleted");
    } catch {
      if (detailView.isCurrent(view)) flash("error");
    }
  }

  /** Passwords are fetched on demand; never included in `VaultEntry` from the backend. */
  async function ensurePassword(view = detailView.capture()): Promise<string | null> {
    if (!view || !detailView.isCurrent(view)) return null;
    if (passwordLoaded) return fetchedPassword;
    if (passwordLoading) return null;
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    if (!sessionId) return null;
    passwordLoading = true;
    try {
      const value = await vault.callInSession(sessionId, () => vault.getEntryPassword(uuid));
      if (!detailView.isCurrent(view)) return null;
      fetchedPassword = value;
      passwordLoaded = true;
      return value;
    } finally {
      if (detailView.isCurrent(view)) passwordLoading = false;
    }
  }

  /** Protected custom-field values are fetched on demand, never part of
   * `VaultEntry` from the backend (mirrors the password flow). */
  async function ensureCustomField(
    name: string,
    view = detailView.capture(),
  ): Promise<string | null> {
    if (!view || !detailView.isCurrent(view)) return null;
    if (customFieldLoaded[name]) return customFieldValues[name] ?? null;
    if (customFieldLoading[name]) return null;
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    if (!sessionId) return null;
    customFieldLoading = { ...customFieldLoading, [name]: true };
    try {
      const value = await vault.callInSession(sessionId, () =>
        vault.getCustomFieldValue(uuid, name),
      );
      if (!detailView.isCurrent(view)) return null;
      customFieldValues = { ...customFieldValues, [name]: value ?? "" };
      customFieldLoaded = { ...customFieldLoaded, [name]: true };
      return value;
    } finally {
      if (detailView.isCurrent(view)) {
        customFieldLoading = { ...customFieldLoading, [name]: false };
      }
    }
  }

  async function copyCustomField(name: string): Promise<void> {
    const view = detailView.capture();
    if (!view) return;
    try {
      const value = await ensureCustomField(name, view);
      if (value === null) {
        if (detailView.isCurrent(view)) flash("error");
        return;
      }
      const copied = await consumeCurrentView(
        detailView,
        view,
        async () => value,
        (currentValue) => copyValue(currentValue, true),
      );
      if (copied && detailView.isCurrent(view)) flash("custom");
    } catch {
      if (detailView.isCurrent(view)) flash("error");
    }
  }

  async function toggleCustomFieldReveal(name: string): Promise<void> {
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    if (!sessionId) return;
    try {
      const value = await ensureCustomField(name);
      if (!canToggleSecretReveal(value, sessionId, detailSessionId(), uuid, entry.uuid)) return;
      customFieldRevealed = { ...customFieldRevealed, [name]: !customFieldRevealed[name] };
    } catch {
      flash("error");
    }
  }

  async function copyPassword(): Promise<void> {
    const view = detailView.capture();
    if (!view) return;
    try {
      const password = await ensurePassword(view);
      if (password === null) return;
      const copied = await consumeCurrentView(
        detailView,
        view,
        async () => password,
        (currentPassword) => copyValue(currentPassword, true),
      );
      if (copied && detailView.isCurrent(view)) flash("password");
    } catch {
      if (detailView.isCurrent(view)) flash("error");
    }
  }

  async function toggleReveal(): Promise<void> {
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    if (!sessionId) return;
    try {
      const password = await ensurePassword();
      if (!canToggleSecretReveal(password, sessionId, detailSessionId(), uuid, entry.uuid)) return;
      revealPassword = !revealPassword;
    } catch {
      flash("error");
    }
  }

  function toastMessage(kind: string): { message: string; isError: boolean } {
    switch (kind) {
      case "error":
        return { message: "操作失败", isError: true };
      case "attachment":
        return { message: "附件已保存", isError: false };
      case "attachmentAdded":
        return { message: "附件已添加", isError: false };
      case "username":
        return { message: "已复制用户名", isError: false };
      case "password":
        return { message: "已复制密码", isError: false };
      case "restored":
        return { message: "已恢复历史版本", isError: false };
      case "deleted":
        return { message: "已删除历史版本", isError: false };
      case "url":
        return { message: "已复制网址", isError: false };
      case "email":
        return { message: "已复制邮箱", isError: false };
      case "phone":
        return { message: "已复制电话号码", isError: false };
      case "notes":
        return { message: "备注已保存", isError: false };
      case "saved":
        return { message: "已保存", isError: false };
      case "protected":
        return { message: "受保护字段需先显示后再编辑", isError: true };
      default:
        return { message: "已复制到剪贴板", isError: false };
    }
  }

  function flash(kind: string): void {
    const { message, isError } = toastMessage(kind);
    showTip(message, isError ? "error" : "success");
  }

  async function handleCopy(value: string, kind: string, sensitive = false): Promise<void> {
    try {
      await copyValue(value, sensitive);
      flash(kind);
    } catch {
      flash("error");
    }
  }

  function openUrlExternal(): void {
    if (entry.url) openExternalUrl(entry.url);
  }

  function openExternalUrl(url: string): void {
    if (!url) return;
    if (isTauriRuntime()) {
      void openUrl(url);
    } else {
      window.open(url, "_blank", "noopener,noreferrer");
    }
  }

  async function saveAttachment(name: string): Promise<void> {
    const sessionId = detailSessionId();
    const uuid = entry.uuid;
    const view = detailView.capture();
    if (!sessionId || !view) return;
    try {
      if (!isTauriRuntime()) throw new Error("browser");
      const picked = await awaitCurrentView(detailView, view, () =>
        saveDialog({ defaultPath: name }),
      );
      if (!picked.current || !picked.value) return;
      const dest = picked.value;
      await vault.callInSession(sessionId, () => vault.saveAttachment(uuid, name, dest));
      if (!detailView.isCurrent(view)) return;
      flash("attachment");
    } catch {
      if (detailView.isCurrent(view)) flash("error");
    }
  }

  /** Read a dropped file into base64 (kept in memory only, never persisted or
   *  logged), mirroring the editor dialog's attachment flow. */
  function readDroppedFileAsBase64(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(",")[1] ?? "");
      reader.onerror = () => reject(new Error("读取文件失败"));
      reader.readAsDataURL(file);
    });
  }

  function handleAttachmentDragEnter(event: DragEvent): void {
    if (!event.dataTransfer?.types.includes("Files")) return;
    event.preventDefault();
    attachmentDragDepth += 1;
    attachmentDragActive = true;
  }

  function handleAttachmentDragOver(event: DragEvent): void {
    if (!event.dataTransfer?.types.includes("Files")) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  }

  function handleAttachmentDragLeave(event: DragEvent): void {
    if (!event.dataTransfer?.types.includes("Files")) return;
    attachmentDragDepth = Math.max(0, attachmentDragDepth - 1);
    if (attachmentDragDepth === 0) attachmentDragActive = false;
  }

  async function handleAttachmentDrop(event: DragEvent): Promise<void> {
    event.preventDefault();
    attachmentDragDepth = 0;
    attachmentDragActive = false;
    const files = event.dataTransfer?.files ? Array.from(event.dataTransfer.files) : [];
    if (!files.length) return;
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    const view = detailView.capture();
    if (!sessionId || !view) return;
    try {
      const attachments: AttachmentInput[] = [];
      for (const file of files) {
        const data = await readDroppedFileAsBase64(file);
        attachments.push({ name: file.name, data });
      }
      await vault.callInSession(sessionId, () => vault.addAttachments(uuid, attachments));
      if (!detailView.isCurrent(view)) return;
      flash("attachmentAdded");
    } catch {
      if (detailView.isCurrent(view)) flash("error");
    }
  }

  /** Debounced auto-save for the inline notes editor. Fires 800ms after the
   *  last keystroke and again on blur, so the last committed draft never
   *  depends on a single timer. */
  function scheduleNotesSave(): void {
    notesSaveVersion += 1;
    if (notesSaveTimer) clearTimeout(notesSaveTimer);
    notesDirty = true;
    notesSaveTimer = setTimeout(() => void persistNotes(), 800);
  }

  async function persistNotes(): Promise<void> {
    if (notesSaveTimer) {
      clearTimeout(notesSaveTimer);
      notesSaveTimer = undefined;
    }
    const view = detailView.capture();
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    if (!view || !sessionId || notesSaving || !notesDirty) return;
    if (notesDraft === entry.notes) {
      notesDirty = false;
      return;
    }
    const value = notesDraft;
    const version = notesSaveVersion;
    notesSaving = true;
    try {
      await vault.callInSession(sessionId, () => vault.updateEntries([uuid], { notes: value }));
      if (!detailView.isCurrent(view)) return;
      if (notesSaveVersion === version) notesDirty = false;
      flash("notes");
    } catch {
      if (detailView.isCurrent(view)) flash("error");
    } finally {
      if (detailView.isCurrent(view)) {
        notesSaving = false;
        if (notesDirty && notesSaveVersion !== version) void persistNotes();
      }
    }
  }

  function editingMatches(target: EditTarget): boolean {
    return editing !== null && editing.kind === target.kind && editing.name === target.name;
  }

  /** The notes region hugs its content up to ~45% of the detail body; taller
   *  content scrolls inside the region so the fields keep the remaining space. */
  function resizeNotes(): void {
    const layout = fieldsLayoutEl;
    const active = notesTextareaEl ?? notesViewEl;
    if (!layout || !active) return;
    const cap = Math.max(64, Math.round(layout.clientHeight * 0.45));
    active.style.maxHeight = `${cap}px`;
    if (notesTextareaEl) {
      notesTextareaEl.style.height = "auto";
      notesTextareaEl.style.height = `${Math.min(notesTextareaEl.scrollHeight, cap)}px`;
    }
  }

  $effect(() => {
    void notesDraft;
    void notesEditing;
    resizeNotes();
  });

  $effect(() => {
    const layout = fieldsLayoutEl;
    if (!layout) return;
    resizeNotes();
    notesResizeObserver ??= new ResizeObserver(() => resizeNotes());
    notesResizeObserver.observe(layout);
    return () => {
      notesResizeObserver?.disconnect();
      notesResizeObserver = undefined;
    };
  });

  async function enterNotesEdit(): Promise<void> {
    notesEditing = true;
    await tick();
    const textarea = notesTextareaEl;
    textarea?.focus();
    const length = textarea?.value.length ?? 0;
    textarea?.setSelectionRange(length, length);
    resizeNotes();
  }

  function onNotesViewClick(): void {
    void enterNotesEdit();
  }

  function onNotesViewKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      void enterNotesEdit();
    }
  }

  /** Inline note contact: URLs open externally, emails/phones copy. The click
   *  is stopped from reaching the surrounding view, which would start editing. */
  function onNotesLinkClick(event: MouseEvent, kind: ContactKind, value: string): void {
    event.stopPropagation();
    if (kind === "url") openExternalUrl(value);
    else void handleCopy(value, kind);
  }

  function onNotesLinkKeydown(event: KeyboardEvent, kind: ContactKind, value: string): void {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      event.stopPropagation();
      if (kind === "url") openExternalUrl(value);
      else void handleCopy(value, kind);
    }
  }

  /** Right-click entry into inline edit mode. Protected fields (password,
   *  protected custom fields) must already be revealed — their value shown as
   *  plain text — before editing is allowed; otherwise the editor refuses to
   *  open and flashes a hint to reveal first. */
  async function startEdit(target: EditTarget): Promise<void> {
    const view = detailView.capture();
    if (!view || editSaving) return;
    let value: string;
    if (target.kind === "password") {
      if (!revealPassword) {
        if (detailView.isCurrent(view)) flash("protected");
        return;
      }
      value = fetchedPassword;
    } else if (target.kind === "custom" && target.protected) {
      if (!customFieldRevealed[target.name!]) {
        if (detailView.isCurrent(view)) flash("protected");
        return;
      }
      value = customFieldValues[target.name!] ?? "";
    } else if (target.kind === "custom") {
      value = entry.customFields?.find((f) => f.name === target.name)?.value ?? "";
    } else {
      value = entry[target.kind] ?? "";
    }
    if (!detailView.isCurrent(view)) return;
    editing = target;
    editValue = value;
    editSaving = false;
    await tick();
    editInput?.focus();
    editInput?.select();
  }

  /** Commit the inline edit: blur, Enter, or any other exit path persist the
   *  value. Unchanged values close the editor silently. */
  async function commitEdit(): Promise<void> {
    const target = editing;
    if (!target || editSaving) return;
    const value = editValue;
    const unchanged =
      target.kind === "custom"
        ? target.protected
          ? (customFieldValues[target.name!] ?? "") === value
          : (entry.customFields?.find((f) => f.name === target.name)?.value ?? "") === value
        : target.kind === "password"
          ? fetchedPassword === value
          : (entry[target.kind] ?? "") === value;
    if (unchanged) {
      editing = null;
      return;
    }
    // Exit edit mode before the await so the unmount blur cannot re-enter.
    editing = null;
    editSaving = true;
    const uuid = entry.uuid;
    const sessionId = detailSessionId();
    const view = detailView.capture();
    if (!sessionId || !view) {
      editSaving = false;
      return;
    }
    try {
      if (target.kind === "custom") {
        await vault.callInSession(sessionId, () =>
          vault.updateCustomFieldValue(uuid, target.name!, value, target.protected ?? false),
        );
      } else {
        let patch: EntryPatch;
        if (target.kind === "username") patch = { username: value };
        else if (target.kind === "password") patch = { password: value };
        else patch = { url: value };
        await vault.callInSession(sessionId, () => vault.updateEntries([uuid], patch));
      }
      if (detailView.isCurrent(view)) flash("saved");
    } catch {
      if (detailView.isCurrent(view)) flash("error");
    } finally {
      if (detailView.isCurrent(view)) editSaving = false;
    }
  }

  function cancelEdit(): void {
    if (!editing || editSaving) return;
    editing = null;
  }

  function onEditKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter") {
      event.preventDefault();
      void commitEdit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancelEdit();
    }
  }

  /** Keyboard entry for the field-value right-click targets (role="button"). */
  function onFieldKeydown(event: KeyboardEvent, target: EditTarget): void {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      void startEdit(target);
    }
  }
</script>

<div class="detail">
  <header class="detail-head">
    {#if onback}
      <button class="detail-btn back" onclick={onback} title="返回" aria-label="返回列表">
        <AppIcon name="chevron-left" size={16} />
      </button>
    {/if}
    <div class="detail-title-row">
      <span class="entry-icon" style:--entry-color={entry.color}
        >{#if customIconUrl}
          <img class="entry-icon-img" src={customIconUrl} alt="" draggable="false" />
        {:else}
          <AppIcon name={iconName} size={20} />
        {/if}</span
      >
      <div class="detail-titles">
        <h3 class="detail-title">{entry.title || "未命名条目"}</h3>
        <p class="detail-path">{groupPath}</p>
      </div>
    </div>
    <div class="detail-actions">
      {#if inRecycleBin && onrestore}
        <button class="detail-btn restore" onclick={() => onrestore(entry)} title="恢复条目">
          <AppIcon name="undo" size={15} />
        </button>
      {/if}
      <button
        class="detail-btn"
        class:star-active={entry.favorite}
        onclick={() => onfavorite(entry)}
        title={entry.favorite ? "取消收藏" : "收藏条目"}
      >
        <AppIcon name="star" size={15} />
      </button>
      <button class="detail-btn" onclick={() => onedit(entry)} title="编辑条目">
        <AppIcon name="edit" size={15} />
      </button>
      <button class="detail-btn danger" onclick={() => ondelete(entry)} title="删除条目">
        <AppIcon name="trash" size={15} />
      </button>
    </div>
  </header>

  <div class="detail-tabs" role="tablist" aria-label="详情选项卡">
    <button
      type="button"
      role="tab"
      class="detail-tab"
      class:active={activeTab === "fields"}
      aria-selected={activeTab === "fields"}
      onclick={() => (activeTab = "fields")}
    >
      字段
    </button>
    <button
      type="button"
      role="tab"
      class="detail-tab"
      class:active={activeTab === "meta"}
      aria-selected={activeTab === "meta"}
      onclick={() => {
        activeTab = "meta";
        void loadStorage();
      }}
    >
      元属性
    </button>
    <button
      type="button"
      role="tab"
      class="detail-tab"
      class:active={activeTab === "attachments"}
      aria-selected={activeTab === "attachments"}
      onclick={() => (activeTab = "attachments")}
    >
      附件
    </button>
    <button
      type="button"
      role="tab"
      class="detail-tab"
      class:active={activeTab === "history"}
      aria-selected={activeTab === "history"}
      onclick={() => {
        activeTab = "history";
        void loadHistory();
      }}
    >
      历史
    </button>
  </div>

  <div class="detail-body" role="tabpanel">
    {#if activeTab === "fields"}
      <div class="fields-layout" bind:this={fieldsLayoutEl}>
        <div class="fields-scroll">
          <div class="field-block">
            <span class="field-label">用户名</span>
            <div
              class="field-value"
              class:editing={editingMatches({ kind: "username" })}
              role="button"
              tabindex="0"
              oncontextmenu={(e) => {
                e.preventDefault();
                void startEdit({ kind: "username" });
              }}
              ondblclick={() => void startEdit({ kind: "username" })}
              onkeydown={(e) => onFieldKeydown(e, { kind: "username" })}
              title="双击或右键编辑"
            >
              {#if editingMatches({ kind: "username" })}
                <input
                  class="inline-edit"
                  bind:this={editInput}
                  bind:value={editValue}
                  onkeydown={onEditKeydown}
                  onblur={() => void commitEdit()}
                  spellcheck="false"
                />
              {:else}
                <span class="field-text">{entry.username || "—"}</span>
                {#if entry.username}
                  <button
                    class="copy-btn"
                    onclick={() => handleCopy(entry.username, "username")}
                    title="复制用户名"
                  >
                    <AppIcon name="copy" size={13} />
                  </button>
                {/if}
              {/if}
            </div>
          </div>

          <div class="field-block">
            <span class="field-label">密码</span>
            <div
              class="field-value"
              class:editing={editingMatches({ kind: "password" })}
              role="button"
              tabindex="0"
              oncontextmenu={(e) => {
                e.preventDefault();
                void startEdit({ kind: "password" });
              }}
              ondblclick={() => void startEdit({ kind: "password" })}
              onkeydown={(e) => onFieldKeydown(e, { kind: "password" })}
              title="双击或右键编辑"
            >
              {#if editingMatches({ kind: "password" })}
                <input
                  class="inline-edit mono"
                  bind:this={editInput}
                  bind:value={editValue}
                  onkeydown={onEditKeydown}
                  onblur={() => void commitEdit()}
                  spellcheck="false"
                />
              {:else}
                <span class="field-text mono"
                  >{revealPassword ? fetchedPassword : "••••••••••••"}</span
                >
                <button
                  class="copy-btn"
                  onclick={toggleReveal}
                  title={revealPassword ? "隐藏密码" : "显示密码"}
                >
                  <AppIcon name={revealPassword ? "eye-off" : "eye"} size={13} />
                </button>
                <button class="copy-btn" onclick={copyPassword} title="复制密码">
                  <AppIcon name="copy" size={13} />
                </button>
              {/if}
            </div>
          </div>

          {#if entry.url}
            <div class="field-block">
              <span class="field-label">网址</span>
              <div
                class="field-value"
                class:editing={editingMatches({ kind: "url" })}
                role="button"
                tabindex="0"
                oncontextmenu={(e) => {
                  e.preventDefault();
                  void startEdit({ kind: "url" });
                }}
                ondblclick={() => void startEdit({ kind: "url" })}
                onkeydown={(e) => onFieldKeydown(e, { kind: "url" })}
                title="双击或右键编辑"
              >
                {#if editingMatches({ kind: "url" })}
                  <input
                    class="inline-edit"
                    bind:this={editInput}
                    bind:value={editValue}
                    onkeydown={onEditKeydown}
                    onblur={() => void commitEdit()}
                    spellcheck="false"
                  />
                {:else}
                  <button class="url-link" onclick={openUrlExternal} title={entry.url}>
                    <span class="field-text link">{entry.url}</span>
                  </button>
                  <button
                    class="copy-btn"
                    onclick={() => handleCopy(entry.url, "url")}
                    title="复制网址"
                  >
                    <AppIcon name="copy" size={13} />
                  </button>
                {/if}
              </div>
            </div>
          {/if}

          {#if entry.hasTotp}
            <div class="field-block">
              <span class="field-label">OTP 验证码</span>
              <div class="field-value">
                <TotpWidget entryUuid={entry.uuid} />
              </div>
            </div>
          {/if}

          {#if entry.customFields?.some((f) => f.name !== "KPRPC JSON")}
            {#each entry.customFields.filter((f) => f.name !== "KPRPC JSON") as field}
              <div class="field-block">
                <span class="field-label">
                  {field.name}
                  {#if field.protected}
                    <span class="protected-badge" title="受保护字段 (值不进入快照)">
                      <AppIcon name="lock" size={10} />
                    </span>
                  {/if}
                </span>
                <div
                  class="field-value"
                  class:editing={editingMatches({ kind: "custom", name: field.name })}
                  role="button"
                  tabindex="0"
                  oncontextmenu={(e) => {
                    e.preventDefault();
                    void startEdit({
                      kind: "custom",
                      name: field.name,
                      protected: field.protected,
                    });
                  }}
                  ondblclick={() =>
                    void startEdit({
                      kind: "custom",
                      name: field.name,
                      protected: field.protected,
                    })}
                  onkeydown={(e) =>
                    onFieldKeydown(e, {
                      kind: "custom",
                      name: field.name,
                      protected: field.protected,
                    })}
                  title="双击或右键编辑"
                >
                  {#if editingMatches({ kind: "custom", name: field.name })}
                    <input
                      class="inline-edit"
                      class:mono={field.protected}
                      bind:this={editInput}
                      bind:value={editValue}
                      onkeydown={onEditKeydown}
                      onblur={() => void commitEdit()}
                      spellcheck="false"
                    />
                  {:else if field.protected}
                    <span class="field-text mono">
                      {customFieldRevealed[field.name]
                        ? customFieldValues[field.name] || ""
                        : "••••••••••••"}
                    </span>
                    {#if customFieldLoading[field.name]}
                      <span class="copy-btn" aria-hidden="true">
                        <AppIcon name="clock" size={13} />
                      </span>
                    {:else}
                      <button
                        class="copy-btn"
                        onclick={() => toggleCustomFieldReveal(field.name)}
                        title={customFieldRevealed[field.name] ? "隐藏字段值" : "显示字段值"}
                      >
                        <AppIcon
                          name={customFieldRevealed[field.name] ? "eye-off" : "eye"}
                          size={13}
                        />
                      </button>
                      <button
                        class="copy-btn"
                        onclick={() => copyCustomField(field.name)}
                        title="复制字段值"
                      >
                        <AppIcon name="copy" size={13} />
                      </button>
                    {/if}
                  {:else}
                    {@const contact = field.value ? classifyContact(field.value) : null}
                    {#if contact === "url"}
                      <button
                        class="field-text link contact"
                        onclick={() => openExternalUrl(field.value)}
                        title={field.value}
                      >
                        {field.value}
                      </button>
                    {:else if contact}
                      <button
                        class="field-text link contact"
                        onclick={() => handleCopy(field.value, contact)}
                        title="点击复制"
                      >
                        {field.value}
                      </button>
                    {:else}
                      <span class="field-text" title={field.value}>{field.value || "—"}</span>
                    {/if}
                    {#if field.value}
                      <button
                        class="copy-btn"
                        onclick={() => handleCopy(field.value, "custom")}
                        title="复制字段值"
                      >
                        <AppIcon name="copy" size={13} />
                      </button>
                    {/if}
                  {/if}
                </div>
              </div>
            {/each}
          {/if}
        </div>

        <div class="notes-section">
          <div class="notes-divider" role="presentation">
            <span>备注</span>
          </div>
          {#if notesEditing}
            <textarea
              class="notes-textarea"
              bind:this={notesTextareaEl}
              bind:value={notesDraft}
              placeholder="添加备注…"
              oninput={() => {
                scheduleNotesSave();
                resizeNotes();
              }}
              onblur={() => {
                void persistNotes();
                notesEditing = false;
              }}
              spellcheck="false"></textarea>
          {:else}
            <div
              class="notes-view"
              role="button"
              tabindex="0"
              aria-label="备注（点击编辑）"
              bind:this={notesViewEl}
              onclick={onNotesViewClick}
              onkeydown={onNotesViewKeydown}
            >
              {#if notesDraft}
                {#each linkifyContacts(notesDraft) as token, i (i)}
                  {#if token.kind === "text"}
                    <span>{token.value}</span>
                  {:else}
                    <button
                      class="notes-link"
                      type="button"
                      onclick={(e) => onNotesLinkClick(e, token.kind, token.value)}
                      onkeydown={(e) => onNotesLinkKeydown(e, token.kind, token.value)}
                      title={token.kind === "url" ? "打开链接" : "点击复制"}>{token.value}</button
                    >
                  {/if}
                {/each}
              {:else}
                <span class="notes-placeholder">添加备注…</span>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    {:else if activeTab === "meta"}
      <div class="field-block">
        <span class="field-label">所属分组</span>
        <div class="field-value">
          <span class="field-text">{groupPath || "—"}</span>
        </div>
      </div>

      <div class="field-block">
        <span class="field-label">创建时间</span>
        <div class="field-value">
          <span class="field-text">{formatLocalDate(entry.created)}</span>
        </div>
      </div>

      <div class="field-block">
        <span class="field-label">修改时间</span>
        <div class="field-value">
          <span class="field-text">{formatLocalDate(entry.modified)}</span>
        </div>
      </div>

      <div class="field-block">
        <span class="field-label">占用空间</span>
        <div class="field-value">
          {#if storageLoading}
            <span class="field-text faint">正在统计…</span>
          {:else if storage}
            <span
              class="field-text"
              title={`字段 ${formatBytes(storage.fields)} · 附件 ${formatBytes(storage.attachments)} · 历史 ${formatBytes(storage.history)}`}
            >
              {formatBytes(storage.total)}
            </span>
          {:else}
            <span class="field-text faint">—</span>
          {/if}
        </div>
      </div>

      {#if entry.expires}
        <div class="field-block">
          <span class="field-label">过期时间</span>
          <div class="field-value">
            <span class="field-text" class:expired-text={entry.expired}>
              {formatLocalDate(entry.expires)}{entry.expired ? " · 已过期" : ""}
            </span>
          </div>
        </div>
      {/if}

      {#if entry.tags}
        <div class="field-block">
          <span class="field-label">标签</span>
          <div class="field-value">
            <span class="field-text">{entry.tags}</span>
          </div>
        </div>
      {/if}

      <div class="field-block">
        <span class="field-label">收藏状态</span>
        <div class="field-value">
          <span class="field-text">{entry.favorite ? "已收藏" : "未收藏"}</span>
        </div>
      </div>

      <div class="field-block">
        <span class="field-label">UUID</span>
        <div class="field-value">
          <span class="field-text mono uuid-text" title={entry.uuid}>{entry.uuid}</span>
        </div>
      </div>
    {:else if activeTab === "attachments"}
      <div
        class="attachment-dropzone"
        class:dragging={attachmentDragActive}
        role="group"
        aria-label="附件拖放区域"
        ondragenter={handleAttachmentDragEnter}
        ondragover={handleAttachmentDragOver}
        ondragleave={handleAttachmentDragLeave}
        ondrop={handleAttachmentDrop}
      >
        {#if entry.attachments?.length}
          <div class="attachment-list">
            {#each entry.attachments as attachment}
              <div class="attachment-item" title={attachment.name}>
                <AppIcon name="file" size={14} />
                <span class="attachment-name">{attachment.name}</span>
                <span class="attachment-size">{formatBytes(attachment.size)}</span>
                <button
                  class="copy-btn"
                  onclick={() => (previewAttachmentName = attachment.name)}
                  title="预览附件"
                >
                  <AppIcon name="eye" size={13} />
                </button>
                <button
                  class="copy-btn"
                  onclick={() => saveAttachment(attachment.name)}
                  title="保存附件"
                >
                  <AppIcon name="download" size={13} />
                </button>
              </div>
            {/each}
          </div>
        {:else}
          <div class="tab-empty">
            <AppIcon name="file" size={18} />
            <p>该条目没有附件</p>
          </div>
        {/if}
        <p class="attachment-drop-hint">拖拽文件到此处添加附件</p>
      </div>
    {:else if activeTab === "history"}
      {#if historyLoading}
        <div class="tab-empty">
          <AppIcon name="clock" size={18} />
          <p>正在加载历史版本…</p>
        </div>
      {:else if historyVersions.length === 0}
        <div class="tab-empty">
          <AppIcon name="clock" size={18} />
          <p>该条目没有历史版本</p>
        </div>
      {:else}
        <div class="history-list">
          {#each historyVersions as version (version.index)}
            <div
              class="history-item"
              title={`${version.username || ""}${version.url ? ` · ${version.url}` : ""}`}
            >
              <AppIcon name="clock" size={14} />
              <div class="history-item-main">
                <span class="history-time">
                  {version.modified
                    ? new Date(version.modified).toLocaleString("zh-CN")
                    : "未知时间"}
                </span>
                <span class="history-title">{version.title || "未命名条目"}</span>
              </div>
              <button
                class="copy-btn"
                onclick={() => (viewingVersion = version)}
                title="查看此版本"
              >
                <AppIcon name="eye" size={13} />
              </button>
              <button class="copy-btn" onclick={() => restoreVersion(version)} title="恢复此版本">
                <AppIcon name="undo" size={13} />
              </button>
              <button
                class="copy-btn danger"
                onclick={() => deleteVersion(version)}
                title="删除此版本"
              >
                <AppIcon name="trash" size={13} />
              </button>
            </div>
          {/each}
        </div>
        <p class="history-hint">最多保留最近 10 个版本;恢复操作本身也会记录为新版本。</p>
      {/if}
    {/if}
  </div>

  {#if viewingVersion}
    <HistoryVersionDialog
      {entry}
      version={viewingVersion}
      onclose={() => (viewingVersion = null)}
    />
  {/if}

  {#if previewAttachmentName}
    {@const attachment = entry.attachments?.find((a) => a.name === previewAttachmentName)}
    {#if attachment}
      <AttachmentPreviewDialog
        entryUuid={entry.uuid}
        {attachment}
        onclose={() => (previewAttachmentName = null)}
      />
    {/if}
  {/if}
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border-left: 1px solid var(--border-subtle);
    background: var(--card-bg);
  }

  .detail-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .detail-title-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .entry-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--warning-color);
    background: var(--hover-bg);
  }

  .entry-icon-img {
    width: 20px;
    height: 20px;
    display: block;
    border-radius: 3px;
    object-fit: contain;
  }

  .entry-icon[style*="--entry-color"] {
    color: var(--entry-color);
    border-color: color-mix(in srgb, var(--entry-color) 45%, transparent);
    background: color-mix(in srgb, var(--entry-color) 12%, transparent);
  }

  .detail-titles {
    min-width: 0;
  }

  .detail-title {
    margin: 0;
    font-size: 13px;
    font-weight: 560;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-path {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-actions {
    display: flex;
    gap: 4px;
    flex: 0 0 auto;
  }

  .detail-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .detail-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .detail-btn.star-active {
    color: var(--warning-color);
    border-color: color-mix(in srgb, var(--warning-color) 40%, transparent);
  }

  .detail-btn.star-active:hover {
    color: var(--warning-color);
  }

  .detail-btn.danger:hover {
    color: var(--danger-color);
    border-color: color-mix(in srgb, var(--danger-color) 40%, transparent);
  }

  .detail-btn.restore {
    color: var(--success-color);
    border-color: color-mix(in srgb, var(--success-color) 40%, transparent);
  }

  .detail-btn.restore:hover {
    background: color-mix(in srgb, var(--success-color) 10%, var(--hover-bg));
  }

  .detail-btn.back {
    display: none;
  }

  @media (max-width: 720px) {
    .detail-btn.back {
      display: inline-flex;
    }
    .detail-head {
      align-items: center;
    }
  }

  .detail-tabs {
    display: flex;
    gap: 2px;
    padding: 8px 14px 0;
    border-bottom: 1px solid var(--border-subtle);
    flex: 0 0 auto;
  }

  .detail-tab {
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

  .detail-tab:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .detail-tab.active {
    color: var(--text-primary);
    border-bottom-color: var(--selection-color);
  }

  .detail-body {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 12px 14px 16px;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .fields-layout {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .fields-scroll {
    flex: 1 1 auto;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .notes-section {
    display: flex;
    flex-direction: column;
    flex: 0 1 auto;
    min-height: 0;
    padding-top: 12px;
  }

  .notes-divider {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 0 0 auto;
    margin-bottom: 8px;
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 560;
    letter-spacing: 0.08em;
  }

  .notes-divider::before,
  .notes-divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--border-subtle);
  }

  .notes-textarea {
    min-height: 64px;
    width: 100%;
    padding: 8px 10px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    color: var(--text-primary);
    font-family: inherit;
    font-size: 12px;
    line-height: 1.6;
    resize: none;
    word-break: break-word;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .notes-textarea::placeholder {
    color: var(--text-faint);
  }

  .notes-textarea:focus {
    outline: none;
    border-color: var(--selection-color);
  }

  .notes-view {
    min-height: 64px;
    width: 100%;
    padding: 8px 10px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    color: var(--text-primary);
    font-size: 12px;
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-word;
    overflow-y: auto;
    cursor: text;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .notes-view:focus {
    outline: none;
    border-color: var(--selection-color);
  }

  .notes-placeholder {
    color: var(--text-faint);
  }

  .notes-link {
    padding: 0;
    border: 0;
    background: transparent;
    font: inherit;
    color: var(--link-color);
    cursor: pointer;
    text-align: left;
    word-break: break-all;
  }

  .notes-link:hover {
    text-decoration: underline;
  }

  .notes-link:focus {
    outline: none;
    text-decoration: underline;
  }

  .field-block {
    min-width: 0;
    margin-bottom: 14px;
  }

  .field-label {
    display: block;
    margin-bottom: 4px;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    letter-spacing: 0.04em;
  }

  .protected-badge {
    display: inline-flex;
    vertical-align: middle;
    margin-left: 2px;
    color: var(--accent-color, var(--primary-color));
  }

  .field-value {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-width: 0;
    min-height: 28px;
    padding: 0 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .field-value.editing {
    border-color: var(--selection-color);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--selection-color) 25%, transparent);
  }

  .inline-edit {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: var(--text-primary);
    font-family: inherit;
    font-size: 12px;
    outline: none;
  }

  .inline-edit.mono {
    font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
    letter-spacing: 0.02em;
  }

  .field-text {
    flex: 1;
    min-width: 0;
    color: var(--text-primary);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field-text.expired-text {
    color: var(--danger-color);
  }

  .field-text.mono {
    font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
    letter-spacing: 0.02em;
  }

  .field-text.link {
    color: var(--link-color);
  }

  .field-text.contact {
    padding: 0;
    border: 0;
    background: transparent;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .field-text.faint {
    color: var(--text-faint);
  }

  .url-link {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    flex: 1;
    overflow: hidden;
    padding: 0;
    border: 0;
    color: inherit;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }

  .attachment-dropzone {
    display: flex;
    flex-direction: column;
    gap: 8px;
    border: 1px dashed var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    padding: 10px;
    transition:
      border-color 120ms ease,
      background 120ms ease;
  }

  .attachment-dropzone.dragging {
    border-color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 10%, transparent);
  }

  .attachment-drop-hint {
    margin: 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-align: center;
  }

  .attachment-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .attachment-item {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--hover-bg);
  }

  .attachment-name {
    flex: 1;
    min-width: 0;
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

  .copy-btn {
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

  .copy-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .copy-btn.danger:hover {
    color: var(--danger-color);
    background: color-mix(in srgb, var(--danger-color) 12%, transparent);
  }

  .uuid-text {
    font-size: var(--font-size-tiny, 10px);
    word-break: break-all;
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .history-item {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--hover-bg);
    color: var(--text-muted);
  }

  .history-item-main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .history-time {
    font-size: var(--font-size-tiny, 10px);
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }

  .history-title {
    font-size: 12px;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history-hint {
    margin: 8px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .tab-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 40px 0;
    color: var(--text-faint);
  }

  .tab-empty p {
    margin: 0;
    font-size: var(--font-size-secondary, 11px);
  }
</style>
