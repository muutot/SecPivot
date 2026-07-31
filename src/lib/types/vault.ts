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
}

export interface GroupInput {
  parentUuid: string | null;
  name: string;
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
