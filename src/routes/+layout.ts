import { appSettings } from "$lib/services/settings";

export const ssr = false;

export async function load(): Promise<object> {
  await appSettings.initialize();
  return {};
}
