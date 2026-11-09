import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  chmodSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const workflow = readFileSync(
  new URL("../.github/workflows/release.yml", import.meta.url),
  "utf8",
).replaceAll("\r\n", "\n");
const bash = process.platform === "win32" ? "C:/Program Files/Git/bin/bash.exe" : "/bin/bash";
const posix = (path) =>
  path.replaceAll("\\", "/").replace(/^([A-Za-z]):/, (_, drive) => `/${drive.toLowerCase()}`);

function job(name) {
  const section = workflow.match(
    new RegExp(`^  ${name}:\\n[\\s\\S]*?(?=^  [a-z][a-z-]*:|$(?![\\s\\S]))`, "m"),
  );
  assert.ok(section, `required job ${name} must exist`);
  return section[0];
}

function signingScript() {
  const section = job("sign-android");
  const step = section
    .split("      - name: Sign and verify release APKs\n")[1]
    ?.split("\n      - ")[0];
  assert.ok(step, "dedicated signing step must exist");
  const body = step.split("        run: |\n")[1];
  assert.ok(body, "signing script must be inline, never downloaded build code");
  return body
    .split("\n")
    .filter((line) => line.startsWith("          ") || !line.trim())
    .map((line) => line.slice(10))
    .join("\n");
}

test("release signing and publishing do not execute build code or check out sources", () => {
  const builder = job("android");
  const signer = job("sign-android");
  assert.doesNotMatch(builder, /secrets\.|ANDROID_KEYSTORE|ANDROID_KEY_PASSWORD/);
  assert.doesNotMatch(signer, /actions\/checkout|npm |npx |cargo |\.\/scripts\//);
  assert.match(signer, /needs: \[validate, android\]/);
  assert.match(signer, /name: android-unsigned-apk/);
  assert.match(signer, /name: android-release-apk/);
  assert.doesNotMatch(job("publish"), /actions\/checkout/);
  assert.match(job("publish-android"), /needs\.sign-android\.result == 'success'/);
  for (const action of workflow.matchAll(/uses: ([^\s]+)@([^\s]+)/g)) {
    assert.match(action[2], /^[a-f0-9]{40}$/, `action ${action[1]} must be immutable`);
  }
});

function fixture() {
  const scriptBody = signingScript();
  const root = mkdtempSync(join(tmpdir(), "secpivot-release-isolation-"));
  const workspace = join(root, "workspace");
  const runnerTemp = join(root, "runner-temp");
  const androidHome = join(root, "sdk");
  const tools = join(androidHome, "build-tools", "35.0.0");
  const bin = join(root, "bin");
  for (const dir of [join(workspace, "dist", "android"), runnerTemp, tools, bin])
    mkdirSync(dir, { recursive: true });
  for (const abi of ["arm64-v8a", "x86_64"])
    writeFileSync(
      join(workspace, "dist", "android", `SecPivot-1.2.3-android-${abi}.apk`),
      "fixture-unsigned",
    );
  writeFileSync(
    join(tools, "apksigner"),
    `#!/bin/bash
set -euo pipefail
printf '%s\\n' "$@" >> "$PROBE_ARGS"
if [ "$1" = verify ]; then exit "$FAIL_VERIFY"; fi
while [ "$#" -gt 0 ]; do
  case "$1" in
    --ks) test -s "$2"; case "$2" in "$RUNNER_TEMP"/*) ;; *) exit 82 ;; esac ;;
    --out) output="$2" ;;
  esac
  shift
done
printf 'fixture-signed' > "$output"
`,
  );
  writeFileSync(
    join(bin, "unzip"),
    `#!/bin/bash
for apk; do :; done
case "$apk" in
  *arm64-v8a*) abi=arm64-v8a ;;
  *) abi=x86_64 ;;
esac
if [ "$BAD_ABI" = 1 ]; then abi=armeabi-v7a; fi
printf 'lib/%s/libsecpivot_desktop_lib.so\\n' "$abi"
if [ "$BAD_ABI" = 2 ]; then printf 'lib/armeabi-v7a/libforeign.so\\n'; fi
`,
  );
  chmodSync(join(tools, "apksigner"), 0o700);
  chmodSync(join(bin, "unzip"), 0o700);
  const script = join(root, "sign.sh");
  writeFileSync(script, 'export PATH="$PROBE_BIN:$PATH"\n' + scriptBody);
  const argsFile = join(root, "args.txt");
  const env = {
    ...process.env,
    ANDROID_HOME: posix(androidHome),
    RUNNER_TEMP: posix(runnerTemp),
    RELEASE_TAG: "v1.2.3",
    ANDROID_KEYSTORE_BASE64: Buffer.from("synthetic-keystore").toString("base64"),
    ANDROID_KEYSTORE_PASSWORD: "synthetic-store-pass",
    ANDROID_KEY_PASSWORD: "synthetic-key-pass",
    ANDROID_KEY_ALIAS: "fixture-alias",
    FAIL_VERIFY: "0",
    BAD_ABI: "0",
    PROBE_ARGS: posix(argsFile),
    PROBE_BIN: posix(bin),
    PATH: `${posix(bin)}:/usr/bin:/bin`,
  };
  return {
    workspace,
    runnerTemp,
    argsFile,
    run: (overrides = {}) =>
      spawnSync(bash, ["--noprofile", "--norc", posix(script)], {
        cwd: workspace,
        env: { ...env, ...overrides },
        encoding: "utf8",
      }),
    close: () => rmSync(root, { recursive: true, force: true }),
  };
}

test(
  "signing uses environment password references and cleans its private keystore",
  { skip: !existsSync(bash) },
  () => {
    const probe = fixture();
    try {
      const result = probe.run();
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(readdirSync(probe.runnerTemp), []);
      const args = readFileSync(probe.argsFile, "utf8");
      assert.match(args, /env:ANDROID_KEYSTORE_PASSWORD/);
      assert.match(args, /env:ANDROID_KEY_PASSWORD/);
      assert.doesNotMatch(args, /synthetic-store-pass|synthetic-key-pass/);
      for (const abi of ["arm64-v8a", "x86_64"])
        assert.equal(
          readFileSync(
            join(probe.workspace, "dist", "android", `SecPivot-1.2.3-android-${abi}.apk`),
            "utf8",
          ),
          "fixture-signed",
        );
    } finally {
      probe.close();
    }
  },
);

test(
  "failed signature verification removes keystore and temporary signed output",
  { skip: !existsSync(bash) },
  () => {
    const probe = fixture();
    try {
      const result = probe.run({ FAIL_VERIFY: "81" });
      assert.equal(result.status, 81, result.stderr);
      assert.deepEqual(readdirSync(probe.runnerTemp), []);
      assert.ok(
        readdirSync(join(probe.workspace, "dist", "android")).every((name) =>
          name.endsWith(".apk"),
        ),
      );
    } finally {
      probe.close();
    }
  },
);

test(
  "missing secrets and incorrect ABI content fail before publishing artifacts",
  { skip: !existsSync(bash) },
  () => {
    const probe = fixture();
    try {
      assert.notEqual(probe.run({ ANDROID_KEY_PASSWORD: "" }).status, 0);
      assert.equal(existsSync(probe.argsFile), false, "missing secrets must not invoke signing");
      assert.equal(probe.run({ BAD_ABI: "1" }).status, 1);
      assert.equal(probe.run({ BAD_ABI: "2" }).status, 1);
      assert.deepEqual(readdirSync(probe.runnerTemp), []);
    } finally {
      probe.close();
    }
  },
);
