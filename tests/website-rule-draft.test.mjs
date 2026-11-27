import assert from "node:assert/strict";
import test from "node:test";
import { register } from "node:module";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { WebsiteRulesDraft } = await import("../src/lib/utils/website-rule-draft.ts");

const policy = {
  version: 1,
  rules: [{ url: "https://synthetic.example:443/A?x=%2f#b", mode: "exact" }],
};

test("Bitwarden profiles survive load, edits and template replacement with detached settings", async () => {
  const source = {
    version: 2,
    rules: [{ url: "  example.com:443/A  ", mode: "host" }],
    bitwarden: { defaultMatch: 0, equivalentDomains: [["example.com", "example.net"]] },
  };
  const f = fixture(true);
  await f.draft.load(async () => source);
  source.bitwarden.equivalentDomains[0][0] = "changed source";
  f.state().bitwarden.equivalentDomains[0][1] = "changed publication";
  f.draft.edit(0, { mode: "site" });
  const change = f.draft.change();
  assert.deepEqual(change.rules.bitwarden, {
    defaultMatch: 0,
    equivalentDomains: [["example.com", "example.net"]],
  });
  assert.equal(change.rules.rules[0].url, "  example.com:443/A  ");
  change.rules.bitwarden.equivalentDomains[0][0] = "changed payload";
  assert.equal(f.draft.change().rules.bitwarden.equivalentDomains[0][0], "example.com");
  f.draft.replace({ version: 2, rules: [], bitwarden: { defaultMatch: 5, equivalentDomains: [] } });
  assert.deepEqual(f.draft.change().rules.bitwarden, { defaultMatch: 5, equivalentDomains: [] });
  const before = f.draft.change();
  assert.throws(() =>
    f.draft.replace({
      version: 1,
      rules: [],
      bitwarden: { defaultMatch: 0, equivalentDomains: [] },
    }),
  );
  assert.deepEqual(f.draft.change(), before);
  f.draft.replace(null);
  assert.deepEqual(f.draft.change(), { mode: "clear" });
});
function fixture(enabled = false) {
  let state;
  let current = true;
  const draft = new WebsiteRulesDraft(
    enabled,
    (value) => {
      state = value;
    },
    () => current,
  );
  return {
    draft,
    state: () => state,
    obsolete: () => {
      current = false;
    },
  };
}
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test("absent policy and blank templates omit edits, while removing a baseline policy clears", async () => {
  const blank = fixture();
  assert.equal(blank.draft.change(), null);
  blank.draft.replace(null);
  assert.equal(blank.draft.change(), null);
  blank.draft.replace(policy);
  blank.draft.enable(false, "");
  assert.equal(blank.draft.change(), null);
  const existing = fixture(true);
  await existing.draft.load(async () => policy);
  assert.equal(existing.draft.change(), null);
  existing.draft.enable(false, "");
  assert.deepEqual(existing.draft.change(), { mode: "clear" });
});

test("failed reads never become empty rules; explicit removal cancels a pending retry", async () => {
  const f = fixture(true);
  await f.draft.load(async () => {
    throw new Error("Synthetic read refusal");
  });
  assert.equal(f.state().error, true);
  assert.throws(() => f.draft.change());
  f.draft.enable(false, "");
  assert.throws(() => f.draft.change());
  const pending = deferred();
  const load = f.draft.load(() => pending.promise);
  f.draft.replace(null);
  pending.resolve(policy);
  await load;
  assert.deepEqual(f.draft.change(), { mode: "clear" });
  assert.equal(f.state().enabled, false);
});

test("only the latest rule load can publish and sets the current baseline", async () => {
  const f = fixture(true);
  const first = deferred(),
    second = deferred();
  const a = f.draft.load(() => first.promise),
    b = f.draft.load(() => second.promise);
  second.resolve(null);
  await b;
  first.resolve(policy);
  await a;
  assert.equal(f.state().enabled, false);
  f.draft.replace(null);
  assert.equal(f.draft.change(), null);
});

test("disposed and switched editors cannot publish pending loads or edits", async () => {
  for (const invalidate of [(f) => f.draft.dispose(), (f) => f.obsolete()]) {
    const f = fixture(true),
      pending = deferred();
    const load = f.draft.load(() => pending.promise);
    const before = f.state();
    invalidate(f);
    pending.resolve(policy);
    await load;
    f.draft.replace(null);
    assert.equal(f.state(), before);
    assert.throws(() => f.draft.change());
  }
});

test("host default, URL bytes, empty sets and detached publications preserve draft intent", () => {
  const f = fixture();
  f.draft.enable(true, "https://synthetic.example");
  assert.equal(f.state().rules[0].mode, "host");
  f.draft.edit(0, policy.rules[0]);
  const change = f.draft.change();
  assert.deepEqual(change, { mode: "set", rules: policy });
  change.rules.rules[0].url = "Mutated returned payload";
  f.state().rules[0].url = "Mutated publication";
  assert.equal(f.draft.change().rules.rules[0].url, policy.rules[0].url);
  f.draft.remove(0);
  assert.deepEqual(f.draft.change(), { mode: "set", rules: { version: 1, rules: [] } });
});

test("disabled/loading drafts reject mutations and the rule limit is enforced", async () => {
  const f = fixture();
  f.draft.add();
  assert.equal(f.draft.change(), null);
  f.draft.enable(true, "");
  for (let i = 0; i < 129; i++) f.draft.add();
  assert.equal(f.state().rules.length, 128);
  const pending = deferred(),
    load = f.draft.load(() => pending.promise);
  f.draft.remove(0);
  f.draft.edit(0, { mode: "never" });
  assert.equal(f.state().rules.length, 128);
  assert.equal(f.state().rules[0].mode, "host");
  assert.throws(() => f.draft.change());
  pending.resolve(policy);
  await load;
  assert.equal(f.draft.change(), null);
});

test("pattern edits upgrade the schema and version-two templates never silently downgrade", async () => {
  const f = fixture();
  f.draft.enable(true, "https://synthetic.example");
  f.draft.edit(0, { mode: "prefix" });
  assert.equal(f.draft.change().rules.version, 2);
  f.draft.edit(0, { mode: "regex", url: "^https://synthetic\\.example/" });
  assert.deepEqual(f.draft.change().rules.rules[0], {
    mode: "regex",
    url: "^https://synthetic\\.example/",
  });
  const old = fixture(true);
  await old.draft.load(async () => ({
    version: 2,
    rules: [{ mode: "regex", url: "(?<=synthetic)login" }],
  }));
  assert.equal(old.draft.change(), null);
  old.draft.edit(0, { mode: "never", url: "https://synthetic.example" });
  assert.equal(old.draft.change().rules.version, 2);
  old.draft.replace({ version: 2, rules: [] });
  assert.deepEqual(old.draft.change(), { mode: "set", rules: { version: 2, rules: [] } });
});
