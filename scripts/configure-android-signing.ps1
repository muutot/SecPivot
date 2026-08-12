param(
    [string]$ProjectDir = (Resolve-Path (Join-Path $PSScriptRoot "..\src-tauri")).Path
)

# 配置 Android release 签名：从环境变量/秘密解码 keystore，并让 Gradle
# 在构建时直接读取密码环境变量。构建前须先执行 tauri android init。
#
# 需要环境变量：
#   ANDROID_KEYSTORE_BASE64    - keystore 文件 base64（可在任意有 JDK 的机器生成，见 docs/android.md）
#   ANDROID_KEYSTORE_PASSWORD  - keystore 存储密码
#   ANDROID_KEY_PASSWORD       - 密钥密码（通常同存储密码）
#   ANDROID_KEY_ALIAS          - 密钥别名

$ErrorActionPreference = "Stop"

$requiredVariables = @(
    "ANDROID_KEYSTORE_BASE64",
    "ANDROID_KEYSTORE_PASSWORD",
    "ANDROID_KEY_PASSWORD",
    "ANDROID_KEY_ALIAS"
)
$missingVariables = @(
    $requiredVariables | Where-Object {
        [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($_))
    }
)
if ($missingVariables.Count -gt 0) {
    throw "Missing required Android signing variables: $($missingVariables -join ', ')."
}

$genDir = Join-Path $ProjectDir "gen\android"
$keystorePath = Join-Path $genDir "secpivot-release.jks"
$legacyKeystoreProperties = Join-Path $genDir "keystore.properties"
$gradleFile = Join-Path $genDir "app\build.gradle.kts"

if (-not (Test-Path -LiteralPath $gradleFile -PathType Leaf)) {
    throw "Android Gradle project not found at $gradleFile; run tauri android init first."
}

try {
    $bytes = [Convert]::FromBase64String($env:ANDROID_KEYSTORE_BASE64)
} catch {
    throw "ANDROID_KEYSTORE_BASE64 is not valid base64."
}
if ($bytes.Length -eq 0) {
    throw "ANDROID_KEYSTORE_BASE64 decoded to an empty keystore."
}
[IO.File]::WriteAllBytes($keystorePath, $bytes)
Write-Host "Wrote Android release keystore."

if (Test-Path -LiteralPath $legacyKeystoreProperties -PathType Leaf) {
    Remove-Item -LiteralPath $legacyKeystoreProperties -Force
    Write-Host "Removed legacy Android signing properties."
}

$content = Get-Content -LiteralPath $gradleFile -Raw

if ($content -notmatch "signingConfigs") {
    if ($content -notmatch "android\s*\{") {
        throw "Android Gradle file does not contain an android block."
    }
    $signing = @'

    signingConfigs {
        create("release") {
            val requireSigningEnv = { name: String ->
                System.getenv(name) ?: error("$name is required for Android release signing")
            }
            keyAlias = requireSigningEnv("ANDROID_KEY_ALIAS")
            keyPassword = requireSigningEnv("ANDROID_KEY_PASSWORD")
            storeFile = rootProject.file("secpivot-release.jks")
            storePassword = requireSigningEnv("ANDROID_KEYSTORE_PASSWORD")
        }
    }
'@
    $content = $content -replace "android\s*\{", "android {$signing"
    Write-Host "Injected signingConfigs block into build.gradle.kts"
}

if ($content -notmatch "signingConfig = signingConfigs.getByName\(`"release`"\)") {
    if ($content -notmatch "getByName\(`"release`"\)\s*\{") {
        throw "Android Gradle file does not contain a release build type."
    }
    $content = $content -replace "getByName\(`"release`"\)\s*\{", "getByName(`"release`") {`r`n            signingConfig = signingConfigs.getByName(`"release`")"
    Write-Host "Wired release build type to signing config"
}

[IO.File]::WriteAllText($gradleFile, $content, [Text.UTF8Encoding]::new($false))
Write-Host "Android signing configured."
