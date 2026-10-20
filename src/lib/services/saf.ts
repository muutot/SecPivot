import { invoke } from "@tauri-apps/api/core";

export interface SafDocument {
  uri: string;
  name?: string | null;
}

/** `.kdbx` files are commonly reported as generic binary data on Android. */
const KDBX_MIME_TYPES = ["application/octet-stream", "application/x-keepass2"];

/** Pick an existing document with a persistable read/write grant. */
export async function openDocument(): Promise<SafDocument | null> {
  const picked = await invoke<SafDocument | null>("plugin:saf|open_document", {
    mimeTypes: KDBX_MIME_TYPES,
  });
  return picked ?? null;
}

/** Pick a destination for a new document with a persistable read/write grant. */
export async function createDocument(fileName: string): Promise<SafDocument | null> {
  const picked = await invoke<SafDocument | null>("plugin:saf|create_document", {
    fileName,
    mimeTypes: KDBX_MIME_TYPES,
  });
  return picked ?? null;
}
