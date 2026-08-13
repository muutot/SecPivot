import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(
  new URL("../scripts/configure-android-signing.ps1", import.meta.url),
);
const powershell = process.platform === "win32" ? "powershell.exe" : "pwsh";
const requiredVariables = [
  "ANDROID_KEYSTORE_BASE64",
  "ANDROID_KEYSTORE_PASSWORD",
  "ANDROID_KEY_PASSWORD",
  "ANDROID_KEY_ALIAS",
];

const tauriBuildGradle = `import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

android {
    compileSdk = 36
    namespace = "com.secpivot.mobile"
    defaultConfig {
        applicationId = "com.secpivot.mobile"
        minSdk = 24
        targetSdk = 36
    }
    buildTypes {
        getByName("debug") {
            isDebuggable = true
        }
        getByName("release") {
            isMinifyEnabled = true
        }
    }
}

apply(from = "tauri.build.gradle.kts")
`;

function createProject() {
  const projectDir = mkdtempSync(join(tmpdir(), "secpivot-signing-test-"));
  const androidDir = join(projectDir, "gen", "android");
  const appDir = join(androidDir, "app");
  mkdirSync(appDir, { recursive: true });
  writeFileSync(join(appDir, "build.gradle.kts"), tauriBuildGradle, "utf-8");
  return { projectDir, androidDir, gradlePath: join(appDir, "build.gradle.kts") };
}

function runScript(projectDir, envOverrides = {}) {
  const env = { ...process.env };
  for (const name of requiredVariables) delete env[name];
  Object.assign(env, envOverrides);
  return spawnSync(powershell, ["-NoProfile", "-File", scriptPath, "-ProjectDir", projectDir], {
    encoding: "utf-8",
    env,
  });
}

test("Android signing configuration fails before writing when any secret is missing", () => {
  const project = createProject();
  try {
    const result = runScript(project.projectDir);
    assert.notEqual(result.status, 0);
    assert.match(
      `${result.stdout}\n${result.stderr}`,
      /Missing required Android signing variables/,
    );
    assert.equal(existsSync(join(project.androidDir, "secpivot-release.jks")), false);
    assert.equal(readFileSync(project.gradlePath, "utf-8"), tauriBuildGradle);
  } finally {
    rmSync(project.projectDir, { recursive: true, force: true });
  }
});

test("Android signing configuration patches the Tauri template once without persisting passwords", () => {
  const project = createProject();
  const fakeKeystore = Buffer.from("fixture-keystore");
  const env = {
    ANDROID_KEYSTORE_BASE64: fakeKeystore.toString("base64"),
    ANDROID_KEYSTORE_PASSWORD: "fixture-store-password",
    ANDROID_KEY_PASSWORD: "fixture-key-password",
    ANDROID_KEY_ALIAS: "fixture-release-key",
  };

  try {
    const first = runScript(project.projectDir, env);
    assert.equal(first.status, 0, `${first.stdout}\n${first.stderr}`);
    const firstContent = readFileSync(project.gradlePath, "utf-8");

    const second = runScript(project.projectDir, env);
    assert.equal(second.status, 0, `${second.stdout}\n${second.stderr}`);
    const secondContent = readFileSync(project.gradlePath, "utf-8");

    assert.equal(secondContent, firstContent);
    assert.equal((firstContent.match(/signingConfigs\s*\{/g) ?? []).length, 1);
    assert.equal(
      (firstContent.match(/signingConfig = signingConfigs\.getByName\("release"\)/g) ?? []).length,
      1,
    );
    assert.match(firstContent, /System\.getenv\(name\)/);
    assert.doesNotMatch(firstContent, /fixture-(?:store|key)-password/);
    assert.deepEqual(readFileSync(join(project.androidDir, "secpivot-release.jks")), fakeKeystore);
    assert.equal(existsSync(join(project.androidDir, "keystore.properties")), false);
  } finally {
    rmSync(project.projectDir, { recursive: true, force: true });
  }
});
