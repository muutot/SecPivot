// Resolve hook mapping the `$lib/*` SvelteKit alias to the real source tree so
// plain `node --test` can execute service modules directly (registered from
// tests that need it via `node:module#register`; see vault-service.test.mjs).
// SvelteKit resolves extensionless imports; Node needs the explicit `.ts`.
import { existsSync } from "node:fs";

export async function resolve(specifier, context, next) {
  if (specifier.startsWith("$lib/")) {
    const base = new URL(`../../src/lib/${specifier.slice("$lib/".length)}`, import.meta.url);
    for (const candidate of [base.href, `${base.href}.ts`, `${base.href}/index.ts`]) {
      if (existsSync(new URL(candidate))) return next(candidate, context);
    }
    return next(base.href, context);
  }
  return next(specifier, context);
}
