import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(
  new URL("../scripts/patch-android-gradle-repos.ps1", import.meta.url),
);
const powershell = process.platform === "win32" ? "powershell.exe" : "pwsh";

/** The shape `tauri android init` generates: plugin resolution and
 *  dependency resolution each own a repositories block. */
const settingsGradle = `pluginManagement {
    repositories {
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}
`;

const buildSrcGradle = `repositories { gradlePluginPortal() }
dependencies { }
`;

function runPatch(projectDir) {
  return spawnSync(
    powershell,
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", scriptPath, "-ProjectDir", projectDir],
    { encoding: "utf-8" },
  );
}

function fixture() {
  const root = mkdtempSync(join(tmpdir(), "secpivot-gradle-repos-"));
  const gen = join(root, "src-tauri", "gen", "android");
  mkdirSync(join(gen, "buildSrc"), { recursive: true });
  writeFileSync(join(gen, "buildSrc", "build.gradle.kts"), buildSrcGradle, "utf-8");
  writeFileSync(join(gen, "settings.gradle.kts"), settingsGradle, "utf-8");
  return { root, gen };
}

test(
  "the pluginManagement block is patched with google() and mavenCentral()",
  { skip: process.platform !== "win32" },
  () => {
    const { root, gen } = fixture();
    try {
      const result = runPatch(join(root, "src-tauri"));
      assert.equal(result.status, 0, result.stdout + result.stderr);
      const patched = readFileSync(join(gen, "settings.gradle.kts"), "utf-8");
      // The plugin block must carry the fallbacks itself; reading them from the
      // sibling dependency-resolution block is what made the old script a no-op.
      const pluginBlock = patched.slice(
        patched.indexOf("pluginManagement"),
        patched.indexOf("dependencyResolutionManagement"),
      );
      assert.match(pluginBlock, /google\(\)/);
      assert.match(pluginBlock, /mavenCentral\(\)/);
      assert.match(pluginBlock, /gradlePluginPortal\(\)/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);

test(
  "a single-line repositories block stays valid Kotlin",
  { skip: process.platform !== "win32" },
  () => {
    const { root, gen } = fixture();
    try {
      assert.equal(runPatch(join(root, "src-tauri")).status, 0);
      const patched = readFileSync(join(gen, "buildSrc", "build.gradle.kts"), "utf-8");
      assert.match(patched, /repositories \{[^}]*google\(\)[^}]*mavenCentral\(\)[^}]*\}/);
      // The entries must stay inside the block, not spill onto the next line.
      const repositoriesLine = patched.split("\n").find((line) => line.includes("repositories"));
      assert.ok(
        repositoriesLine.includes("}"),
        "the repositories block must be closed on its line",
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);

test("the patch is idempotent", { skip: process.platform !== "win32" }, () => {
  const { root, gen } = fixture();
  try {
    assert.equal(runPatch(join(root, "src-tauri")).status, 0);
    const first = readFileSync(join(gen, "settings.gradle.kts"), "utf-8");
    assert.equal(runPatch(join(root, "src-tauri")).status, 0);
    const second = readFileSync(join(gen, "settings.gradle.kts"), "utf-8");
    assert.equal(first, second, "a second run must not change the file again");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test(
  "the script fails closed when the required buildSrc file is missing",
  { skip: process.platform !== "win32" },
  () => {
    const root = mkdtempSync(join(tmpdir(), "secpivot-gradle-repos-"));
    try {
      const result = runPatch(join(root, "src-tauri"));
      assert.notEqual(result.status, 0);
      assert.match(result.stdout + result.stderr, /tauri android init first/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);

test("the script exists in the repository", () => {
  assert.ok(existsSync(scriptPath));
});
