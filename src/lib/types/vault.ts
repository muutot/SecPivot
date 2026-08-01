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
  totp?: string;
  icon?: number;
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
  size: number;
  data?: string;
}

export interface VaultGroup {
  uuid: string;
  parentUuid: string | null;
  name: string;
  icon?: number;
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
  customFields?: CustomField[];
  attachments?: AttachmentInput[];
}

export interface GroupInput {
  parentUuid: string | null;
  name: string;
}

export interface TotpCode {
  code: string;
  validFor: number;
  period: number;
}

export interface HistoryVersion {
  index: number;
  modified: string | null;
  title: string;
  username: string;
  url: string;
  notes: string;
  password: string;
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
 * mirrors the file under `Storage/remote/<local_dir>` with rotated backups. */
export type RemoteMode = "memory" | "local";

export const ROOT_GROUP_NAME = "Root";
