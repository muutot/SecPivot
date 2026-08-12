export interface VaultEntry {
  uuid: string;
  groupUuid: string;
  title: string;
  username: string;
  /** Absent in the Tauri runtime (fetched on demand via `get_entry_password`);
   * present only in the browser demo fallback. */
  password?: string;
  url: string;
  notes: string;
  /** Whether the entry carries a TOTP seed. The seed itself is never part of
   * the snapshot: fetch codes via `vault.totpCode` or the seed on demand via
   * `vault.getEntryTotp`; present only in the browser demo fallback. */
  hasTotp: boolean;
  totp?: string;
  icon?: number;
  /** UUID of the database custom icon (favicon stored in the KDBX Meta
   * section); the image data sits in `VaultState.customIcons`. */
  customIcon?: string;
  /** `#RRGGBB` background color tag. */
  color?: string;
  /** KeePass `OverrideURL`: honored for matching only (bridge/RPC/auto-type),
   * never shown as the display URL. */
  overrideUrl?: string;
  /** KeePass `ForegroundColor` (`#RRGGBB`). */
  foregroundColor?: string;
  created?: string;
  modified?: string;
  expires?: string;
  expired?: boolean;
  tags?: string;
  favorite?: boolean;
  /** KeePass per-entry password-quality check flag. When false, the entry is
   * excluded from the security report's weak-password findings. */
  qualityCheck?: boolean;
  /** KDBX `CustomData` map items (plugin metadata from other KeePass clients),
   * sorted by key. Read-only — SecPivot never writes these. */
  customData?: CustomDataEntry[];
  customFields?: CustomField[];
  attachments?: AttachmentInfo[];
  /** Entry-level Auto-Type configuration, if stored. */
  autoType?: EntryAutoTypeConfig;
}

export interface CustomDataEntry {
  key: string;
  /** String value; absent when the item holds binary data. */
  value?: string;
  /** Base64-encoded binary value; present only for binary items. */
  binary?: string;
  modified?: string;
}

export interface CustomField {
  name: string;
  value: string;
  /** KDBX protected string. In the Tauri runtime the value is absent from
   * `VaultEntry` snapshots (resolved on demand via `vault.getCustomFieldValue`);
   * the browser demo fallback keeps it inline. */
  protected?: boolean;
}

export interface AttachmentInfo {
  name: string;
  size: number;
}

/** In-memory attachment preview: `kind` text/image/binary; `data` holds utf8
 *  text or a `data:` image URL; `truncated` marks the 2 MiB preview cap. */
export interface AttachmentPreview {
  kind: "text" | "image" | "binary";
  data: string;
  size: number;
  truncated: boolean;
}

/** Reference to an attachment extracted into the controlled temp directory
 *  for external viewing; `token` removes the file on discard/close. */
export interface TempAttachmentRef {
  token: string;
  path: string;
  name: string;
}

/** One Auto-Type window association (`*` wildcards allowed). */
export interface AutoTypeAssociation {
  window: string;
  sequence: string;
}

/** Entry-level Auto-Type configuration. */
export interface EntryAutoTypeConfig {
  enabled: boolean;
  defaultSequence?: string;
  associations: AutoTypeAssociation[];
}

/** Group-level Auto-Type configuration; absent fields inherit. */
export interface GroupAutoTypeConfig {
  enabled?: boolean;
  defaultSequence?: string;
}

/** Attachment payload sent when saving an entry. `data` (base64) is present
 * only for new or replaced attachments; existing ones are kept by name. */
export interface AttachmentInput {
  name: string;
  data?: string;
}

export interface VaultGroup {
  uuid: string;
  parentUuid: string | null;
  name: string;
  icon?: number;
  /** UUID of the database custom icon used by this group, if any. */
  customIcon?: string;
  isRecycleBin: boolean;
  /** KeePass group option: whether this group's own entries are searchable. */
  enableSearching: boolean;
  /** KeePass group notes. Read-only — SecPivot surfaces but does not edit. */
  notes?: string;
  /** KeePass group tags, comma-separated. Read-only for now. */
  tags?: string;
  /** KeePass group expand flag — persisted via `set_group_expanded` or the
   * batch `set_groups_expanded` command so the tree restores its open state. */
  isExpanded: boolean;
  /** KDBX `CustomData` map items (plugin metadata from other KeePass clients),
   * sorted by key. Read-only — SecPivot never writes these. */
  customData?: CustomDataEntry[];
  /** Group-level Auto-Type configuration, if stored. */
  autoType?: GroupAutoTypeConfig;
  children: VaultGroup[];
  entries: VaultEntry[];
}

export interface VaultState {
  path: string;
  fileName: string;
  root: VaultGroup;
  dirty: boolean;
  modifiedAt: string;
  /** Monotonic session edit revision; every mutation bumps it. */
  revision: number;
  /** Database custom icons (favicons) keyed by custom-icon UUID; values are
   * `data:` URLs ready for `<img>`. Present only when the DB carries icons. */
  customIcons?: Record<string, string>;
  /** Database-meta-level KDBX `CustomData` map items, sorted by key.
   * Read-only — SecPivot never writes these. */
  metaCustomData?: CustomDataEntry[];
  /** KDBX `Meta.DatabaseName`. Editable via `vault.updateDbMeta`. */
  databaseName?: string;
  /** KDBX `Meta.DatabaseDescription`. Editable via `vault.updateDbMeta`. */
  databaseDescription?: string;
}

/** Result of an open/create command: the registry id of the newly active
 *  session plus its authoritative state. The renderer keeps `sessionId` so
 *  later commands can address this session (tabs); commands that omit it
 *  default to the active session. */
export interface VaultOpenResult {
  sessionId: string;
  state: VaultState;
}

/** One open session shown by the tab bar (active first, then parked). */
export interface SessionInfo {
  sessionId: string;
  fileName: string;
  path: string;
  dirty: boolean;
}

/** Lightweight mutation result for small state changes: the renderer applies
 *  the delta against its cached `VaultState` instead of receiving a rebuilt
 *  tree. `revision` matches the backend session revision after the mutation. */
export type MutationDelta =
  | { kind: "favorite"; revision: number; uuid: string; favorite: boolean }
  | { kind: "groupsExpanded"; revision: number; groups: Record<string, boolean> };

/** One entry offered by the global-hotkey auto-type picker. */
export interface AutotypeCandidate {
  uuid: string;
  title: string;
  username: string;
}

/** Read-only storage settings of the open database. */
export interface DatabaseSettings {
  kdf: "Aes" | "Argon2" | "Argon2id";
  cipher: "Aes256" | "Twofish" | "ChaCha20";
  compression: "None" | "Gzip";
  historyMaxItems: number | null;
  historyMaxSize: number | null;
  recycleBinEnabled: boolean;
  entryTemplatesGroup: string | null;
}

/** Partial database-settings write; omitted fields are kept, `null` resets. */
export interface DatabaseSettingsPatch {
  kdf?: "Aes" | "Argon2" | "Argon2id";
  cipher?: "Aes256" | "Twofish" | "ChaCha20";
  compression?: "None" | "Gzip";
  historyMaxItems?: number | null;
  historyMaxSize?: number | null;
  recycleBinEnabled?: boolean | null;
  entryTemplatesGroup?: string | null;
}

/** Server-side security analysis; passwords never cross into the report. */
export interface SecurityReport {
  total: number;
  empty: string[];
  weak: WeakEntry[];
  duplicates: DuplicatePasswords[];
}

export interface WeakEntry {
  uuid: string;
  bits: number;
}

export interface DuplicatePasswords {
  count: number;
  uuids: string[];
}

/** One entry in a similar-password group (passwords never leave the session). */
export interface SimilarEntry {
  uuid: string;
  title: string;
  username: string;
}

/** A cluster of entries whose passwords are similar (at most two edits). */
export interface SimilarPasswordGroup {
  entries: SimilarEntry[];
}

/** Result of `download_favicons` (KeePass "Download Favicons"). */
export interface FaviconReport {
  attempted: number;
  downloaded: number;
}

/** `favicon-progress` event payload emitted during a favicon download run. */
export interface FaviconProgress {
  done: number;
  total: number;
}

export interface EntryInput {
  groupUuid: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp?: string;
  /** ISO-8601 expiry datetime; empty/absent disables expiry. */
  expires?: string;
  /** Built-in KeePass icon index (0-68); `null` resets to the default icon,
   * and an absent value keeps the entry's current icon (custom favicons
   * survive content-only edits). */
  icon?: number | null;
  /** `#RRGGBB` background color; empty/absent clears it. */
  color?: string;
  /** Comma-separated tags; absent keeps the current tags, empty clears them. */
  tags?: string;
  customFields?: CustomField[];
  attachments?: AttachmentInput[];
}

/** Partial entry update applied to several entries at once (batch editing).
 * An absent field leaves every target entry untouched — passwords of
 * unchanged fields are never fetched or re-sent. */
export interface EntryPatch {
  title?: string;
  username?: string;
  password?: string;
  url?: string;
  notes?: string;
  /** TOTP seed to set; empty string clears the existing seed. */
  totp?: string;
  /** New ISO-8601 expiry; set `clearExpires` instead to remove it. */
  expires?: string;
  clearExpires?: boolean;
  /** Built-in KeePass icon index; set `clearIcon` instead to reset it. */
  icon?: number;
  clearIcon?: boolean;
  /** `#RRGGBB` background color; set `clearColor` instead to remove it. */
  color?: string;
  clearColor?: boolean;
  /** Comma-separated tags to set; an empty string clears all tags. */
  tags?: string;
}

/** Flags updated separately from stored fields (see `update_entry_flags`):
 *  `overrideUrl` absent = keep, empty string = clear, non-empty = set;
 *  `qualityCheck` absent = keep, present = set. */
export interface EntryFlags {
  overrideUrl?: string;
  qualityCheck?: boolean;
  /** `#RRGGBB` foreground color; absent = keep, empty string = clear. */
  foregroundColor?: string;
}

export interface GroupInput {
  parentUuid: string | null;
  name: string;
  /** Built-in KeePass icon index; absent = default icon. */
  icon?: number;
}

/** Group metadata patch: `notes`/`tags` absent = keep, empty string = clear;
 *  `enableSearching` absent = keep, present = set. */
export interface GroupMeta {
  notes?: string;
  tags?: string;
  enableSearching?: boolean;
}

/** One normalized import row produced by the backend import parsers. */
export interface ImportRow {
  group: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp?: string;
  customFields: { name: string; value: string }[];
}

export interface TotpCode {
  code: string;
  /** `"totp"` (RFC 6238), `"hotp"` (RFC 4226 counter), or `"steam"` guard. */
  kind: "totp" | "hotp" | "steam";
  /** Seconds until this code expires (1..=period; 0 for HOTP). */
  validFor: number;
  /** Total period in seconds (usually 30; 0 for HOTP). */
  period: number;
  /** The moving factor that produced this code (HOTP only). */
  counter?: number;
}

export interface HistoryVersion {
  index: number;
  modified: string | null;
  title: string;
  username: string;
  url: string;
  notes: string;
  expires: string | null;
  /** Whether this snapshot carried a TOTP seed. */
  hasTotp: boolean;
  /** Built-in KeePass icon index the snapshot used, if any. */
  icon?: number;
  /** UUID of the database custom icon the snapshot used, if any. */
  customIcon?: string;
  tags?: string;
  /** `#RRGGBB` background color the snapshot carried, if any. */
  color?: string;
  favorite: boolean;
  qualityCheck: boolean;
  customData?: CustomDataEntry[];
  customFields: CustomField[];
  attachments: AttachmentInfo[];
}

/** Byte-size breakdown of everything an entry stores. */
export interface EntryStorage {
  /** Bytes of the entry's own field values (including the password). */
  fields: number;
  /** Bytes of the entry's own attachment data. */
  attachments: number;
  /** Bytes of all historical snapshots (their fields + attachments). */
  history: number;
  /** `fields + attachments + history`. */
  total: number;
}

export interface OpenVaultRequest {
  path: string;
  password: string;
  /** Optional keyfile path; only meaningful in the Tauri runtime. */
  keyfile?: string;
}

export interface CreateVaultRequest {
  path: string;
  password: string;
  kdf: string;
  cipher: string;
  compression: string;
  /** Optional keyfile path; only meaningful in the Tauri runtime. */
  keyfile?: string;
}

/** S3 object listed by the remote browser (`s3_list_objects`). */
export interface RemoteObject {
  key: string;
  size: number;
  modified?: string;
}

/** Remote save semantics: `memory` uploads back through the configured
 * transport; `local` also mirrors the file under
 * `Storage/remote/<kind>/<sanitized profile name>` with rotated backups. */
export type RemoteMode = "memory" | "local";

export const ROOT_GROUP_NAME = "Root";
