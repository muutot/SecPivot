import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import test from "node:test";
import ts from "typescript";
const source = readFileSync("src/lib/components/settings/MasterKeyExpiryRuleCard.svelte", "utf8");
const handler = source.match(/^  async function applyRule\([^]*?^  \}/m)[0];
const executable = ts.transpileModule(handler + "\nexports.applyRule=applyRule;", {
  compilerOptions: { target: ts.ScriptTarget.ES2022 },
}).outputText;
function fixture() {
  const calls = [],
    writes = [];
  const context = {
    exports: {},
    value: "P90D",
    setting: "masterKeyExpiryForce",
    generation: 0,
    busy: false,
    error: "",
    lang: "en",
    isTauriRuntime: () => true,
    t: (_, key) => key,
    String,
    invoke: (command, args) =>
      new Promise((resolve, reject) => calls.push({ command, args, resolve, reject })),
    appSettings: { updateSecurity: (key, value) => writes.push({ key, value }) },
  };
  runInNewContext(executable, context);
  return { context, calls, writes };
}
test("rule card validates the exact native rule before publishing any preference", async () => {
  const f = fixture();
  const run = f.context.exports.applyRule();
  assert.deepEqual(JSON.parse(JSON.stringify(f.calls[0].args)), { rule: "P90D" });
  assert.equal(f.calls[0].command, "validate_master_key_expiry_rule");
  assert.deepEqual(f.writes, []);
  f.calls[0].resolve(false);
  await run;
  assert.equal(f.context.error, "security.keyExpiryInvalid");
  assert.deepEqual(f.writes, []);
  f.context.value = "  P30D  ";
  const accepted = f.context.exports.applyRule();
  f.calls[1].resolve(true);
  await accepted;
  assert.deepEqual(f.writes, [{ key: "masterKeyExpiryForce", value: "  P30D  " }]);
});
test("duplicate validation and disposed/changed rule-card replies cannot apply another draft", async () => {
  for (const changedKind of [false, true]) {
    const f = fixture();
    const run = f.context.exports.applyRule();
    await f.context.exports.applyRule();
    assert.equal(f.calls.length, 1);
    if (changedKind) f.context.setting = "masterKeyExpiryRec";
    else f.context.generation++;
    f.calls[0].resolve(true);
    await run;
    assert.deepEqual(f.writes, []);
  }
});
