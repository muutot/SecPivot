// Resolve hook mapping the `$lib/*` SvelteKit alias to the real source tree so
// plain `node --test` can execute service modules directly (registered from
// tests that need it via `node:module#register`; see vault-service.test.mjs).
// SvelteKit resolves extensionless imports; Node needs the explicit `.ts`.
import { statSync } from "node:fs";

function isFile(url) {
  try {
    return statSync(new URL(url)).isFile();
  } catch {
    return false;
  }
}

export async function resolve(specifier, context, next) {
  if (specifier.startsWith("$lib/")) {
    const base = new URL(`../../src/lib/${specifier.slice("$lib/".length)}`, import.meta.url);
    for (const candidate of [base.href, `${base.href}.ts`, `${base.href}/index.ts`]) {
      if (isFile(candidate)) return next(candidate, context);
    }
    return next(base.href, context);
  }
  // Same extensionless fallback for relative imports between source modules
  // (e.g. `../i18n/index.ts` importing `./en`); only applies when the
  // importer is a TypeScript source file.
  if (
    (specifier.startsWith("./") || specifier.startsWith("../")) &&
    context.parentURL?.endsWith(".ts")
  ) {
    const base = new URL(specifier, context.parentURL);
    for (const candidate of [`${base.href}.ts`, `${base.href}/index.ts`]) {
      if (isFile(candidate)) return next(candidate, context);
    }
  }
  return next(specifier, context);
}
