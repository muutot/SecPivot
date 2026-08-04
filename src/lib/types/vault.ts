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
  created?: string;
  modified?: string;
  expires?: string;
  expired?: boolean;
  tags?: string;
  favorite?: boolean;
  customFields?: CustomField[];
  attachments?: AttachmentInfo[];
}

export interface CustomField {
  name: string;
  value: string;
}

export interface AttachmentInfo {
  name: string;
  size: number;
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
  children: VaultGroup[];
  entries: VaultEntry[];
}

export interface VaultState {
  path: string;
  fileName: string;
  root: VaultGroup;
  dirty: boolean;
  modifiedAt: string;
  /** Database custom icons (favicons) keyed by custom-icon UUID; values are
   * `data:` URLs ready for `<img>`. Present only when the DB carries icons. */
  customIcons?: Record<string, string>;
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
}

export interface GroupInput {
  parentUuid: string | null;
  name: string;
  /** Built-in KeePass icon index; absent = default icon. */
  icon?: number;
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
  customFields: CustomField[];
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

/** Remote save semantics: `memory` uploads back to S3 only; `local` also
 * mirrors the file under `Storage/remote/<sanitized profile name>` with
 * rotated backups. */
export type RemoteMode = "memory" | "local";

export const ROOT_GROUP_NAME = "Root";
