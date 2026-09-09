// One-click regression gate: runs `npm run verify`, then prints the manual
// checklist parsed from docs/PITFALLS.md ("## Regression checklist").
// The checklist lives in PITFALLS.md as the single source of truth — edit
// the list there, not here.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

function manualChecklist() {
  const text = readFileSync(path.join(ROOT, "docs", "PITFALLS.md"), "utf8").replace(/^\uFEFF/, "");
  const section = text.split("## Regression checklist")[1];
  if (!section) {
    console.error("regression: ## Regression checklist not found in docs/PITFALLS.md");
    process.exit(2);
  }
  const items = section
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.startsWith("- [ ] (manual)"))
    .map((line) => line.replace("- [ ] (manual)", "").trim());
  return items;
}

console.log("regression: running npm run verify ...");
// Prefer invoking npm's CLI through node directly (argument array, no
// shell) so no shell-quoting warning is emitted; fall back to a shell
// lookup when the bundled layout is unavailable.
const npmCli = path.join(
  path.dirname(process.execPath),
  "node_modules",
  "npm",
  "bin",
  "npm-cli.js",
);
const [verifyCommand, verifyArgs, verifyShell] = existsSync(npmCli)
  ? [process.execPath, [npmCli, "run", "verify"], false]
  : ["npm", ["run", "verify"], true];
const verify = spawnSync(verifyCommand, verifyArgs, {
  cwd: ROOT,
  stdio: "inherit",
  shell: verifyShell,
});
if (verify.status !== 0) {
  console.error(
    `regression: verify failed (exit ${verify.status}); fix it before the manual pass.`,
  );
  process.exit(verify.status ?? 1);
}

const items = manualChecklist();
console.log("\nregression: automated gate passed. Manual checklist (walk through by hand):\n");
for (const item of items) {
  console.log(`[ ] ${item}`);
}
console.log(`\nregression: ${items.length} manual items listed.`);
