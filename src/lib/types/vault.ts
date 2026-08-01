export interface VaultEntry {
  uuid: string;
  groupUuid: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp?: string;
  icon?: number;
  created?: string;
  modified?: string;
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
  children: VaultGroup[];
  entries: VaultEntry[];
}

export interface VaultState {
  path: string;
  fileName: string;
  password: string;
  root: VaultGroup;
  dirty: boolean;
  modifiedAt: string;
}

export interface EntryInput {
  groupUuid: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp?: string;
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

export interface OpenVaultRequest {
  path: string;
  password: string;
}

export interface CreateVaultRequest {
  path: string;
  password: string;
  kdf: string;
  cipher: string;
  compression: string;
}

export const ROOT_GROUP_NAME = "Root";
