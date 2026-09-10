import type { Language } from "$lib/types/settings";
import { t } from "$lib/i18n";

export interface ImportGroupResolver<TState> {
  state: TState;
  baseUuid: string;
  groups: Map<string, Map<string, string>>;
}

/** Resolve or create a slash-delimited import group path. Every create call
 * receives the operation's captured session id, including later iterations
 * after earlier async creates have settled. */
export async function resolveImportGroupPath<TState>(options: {
  path: string;
  sessionId: string;
  lang: Language;
  resolver: ImportGroupResolver<TState>;
  createGroup: (sessionId: string, parentUuid: string, name: string) => Promise<TState>;
  findCreatedUuid: (state: TState, parentUuid: string, name: string) => string | null;
}): Promise<string> {
  const parts = options.path
    .split("/")
    .map((part) => part.trim())
    .filter(Boolean);
  let parentUuid = options.resolver.baseUuid;
  for (const name of parts) {
    const existingUuid = options.resolver.groups.get(parentUuid)?.get(name);
    if (existingUuid) {
      parentUuid = existingUuid;
      continue;
    }

    options.resolver.state = await options.createGroup(options.sessionId, parentUuid, name);
    const createdUuid = options.findCreatedUuid(options.resolver.state, parentUuid, name);
    if (!createdUuid) throw new Error(t(options.lang, "error.createGroupFailed"));

    let children = options.resolver.groups.get(parentUuid);
    if (!children) {
      children = new Map();
      options.resolver.groups.set(parentUuid, children);
    }
    children.set(name, createdUuid);
    parentUuid = createdUuid;
  }
  return parentUuid;
}
