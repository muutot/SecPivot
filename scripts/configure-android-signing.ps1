param(
    [string]$ProjectDir = (Resolve-Path (Join-Path $PSScriptRoot "..\src-tauri")).Path
)

# 配置 Android release 签名：从环境变量/秘密生成 keystore.properties，
# 并将 keystore 解码到 gen/android 目录。构建前须先执行 tauri android init。
#
# 需要环境变量：
#   ANDROID_KEYSTORE_BASE64    - keystore 文件 base64（可在任意有 JDK 的机器生成，见 docs/android.md）
#   ANDROID_KEYSTORE_PASSWORD  - keystore 存储密码
#   ANDROID_KEY_PASSWORD       - 密钥密码（通常同存储密码）
#   ANDROID_KEY_ALIAS          - 密钥别名

$ErrorActionPreference = "Stop"

if (-not $env:ANDROID_KEYSTORE_BASE64) {
    Write-Host "ANDROID_KEYSTORE_BASE64 not set - skipping Android signing setup."
    exit 0
}

$genDir = Join-Path $ProjectDir "gen\android"
$keystorePath = Join-Path $genDir "secpivot-release.jks"
$keystoreProps = Join-Path $genDir "keystore.properties"

$bytes = [Convert]::FromBase64String($env:ANDROID_KEYSTORE_BASE64)
[IO.File]::WriteAllBytes($keystorePath, $bytes)
Write-Host "Wrote keystore to $keystorePath"

$storeFile = $keystorePath -replace "\\", "\\"
@"
password=$env:ANDROID_KEYSTORE_PASSWORD
keyAlias=$env:ANDROID_KEY_ALIAS
storeFile=$storeFile
"@ | Set-Content -Path $keystoreProps -Encoding ASCII
Write-Host "Wrote keystore.properties"

$gradleFile = Join-Path $genDir "app\build.gradle.kts"
$content = Get-Content $gradleFile -Raw

if ($content -notmatch "signingConfigs") {
    $signing = @"

    signingConfigs {
        create("release") {
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            val keystoreProperties = Properties()
            if (keystorePropertiesFile.exists()) {
                keystoreProperties.load(keystorePropertiesFile.inputStream())
            }
            keyAlias = keystoreProperties["keyAlias"] as String
            keyPassword = keystoreProperties["password"] as String
            storeFile = file(keystoreProperties["storeFile"] as String)
            storePassword = keystoreProperties["password"] as String
        }
    }
"@
    $content = $content -replace "android \{", "android {$signing"
    Write-Host "Injected signingConfigs block into build.gradle.kts"
}

if ($content -notmatch "signingConfig = signingConfigs.getByName\(`"release`"\)") {
    $content = $content -replace "getByName\(`"release`"\) \{", "getByName(`"release`") {`r`n            signingConfig = signingConfigs.getByName(`"release`")"
    Write-Host "Wired release build type to signing config"
}

Set-Content -Path $gradleFile -Value $content -Encoding ASCII -NoNewline
Write-Host "Android signing configured."
