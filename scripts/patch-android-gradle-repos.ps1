param(
    [string]$ProjectDir = (Resolve-Path (Join-Path $PSScriptRoot "..\src-tauri")).Path
)

# Patch the generated Tauri Android Gradle project so dependency resolution no
# longer depends on a single repository mirror. Must run after
# `tauri android init` (gen/android is gitignored) and before any gradlew call.
#
# Background: the CI failure resolved `org.jetbrains.kotlin:*:2.0.21`
# (Gradle 8.14.3 Kotlin DSL dependency) from `plugins.gradle.org/m2` only —
# the plugin POM parsed but its parent `kotlin-gradle-plugins-bom` was absent
# there, and no `mavenCentral()` fallback was consulted. Ensuring
# `google()` + `mavenCentral()` + `gradlePluginPortal()` in every generated
# repositories block fixes this class of failure deterministically.
#
# The patch is idempotent: already-present repository entries are never
# duplicated, and files/blocks that do not exist are left untouched (except
# buildSrc, which is required).

$ErrorActionPreference = "Stop"

$genDir = Join-Path $ProjectDir "gen" | Join-Path -ChildPath "android"
$buildSrcFile = Join-Path $genDir "buildSrc" | Join-Path -ChildPath "build.gradle.kts"
$settingsFile = Join-Path $genDir "settings.gradle.kts"
$rootBuildFile = Join-Path $genDir "build.gradle.kts"

$requiredRepos = @("google()", "mavenCentral()", "gradlePluginPortal()")

function Add-MissingReposToBlock {
    param(
        [string]$Content,
        [string]$BlockPattern,
        [string]$BlockLabel
    )
    $match = [regex]::Match($Content, $BlockPattern)
    if (-not $match.Success) {
        return @{ Content = $Content; Changed = $false }
    }
    # Look at a bounded window after the block opening; Gradle blocks are
    # shallow here and this avoids fragile brace matching.
    $windowLength = [Math]::Min(1200, $Content.Length - ($match.Index + $match.Length))
    $window = $Content.Substring($match.Index + $match.Length, $windowLength)
    $missing = @($requiredRepos | Where-Object { $window -notmatch [regex]::Escape($_) })
    if ($missing.Count -eq 0) {
        return @{ Content = $Content; Changed = $false }
    }
    $injection = ($missing | ForEach-Object { "            $_" }) -join "`r`n"
    $replacement = $match.Value + "`r`n" + $injection
    $patched = $Content.Substring(0, $match.Index) + $replacement + $Content.Substring($match.Index + $match.Length)
    Write-Host "Patched $BlockLabel (added $($missing -join ', '))."
    return @{ Content = $patched; Changed = $true }
}

function Ensure-ReposInFile {
    param(
        [string]$FilePath,
        [string[]]$BlockPatterns,
        [string]$Label,
        [bool]$Required
    )
    if (-not (Test-Path -LiteralPath $FilePath -PathType Leaf)) {
        if ($Required) {
            throw "Android Gradle file not found at $FilePath; run tauri android init first."
        }
        Write-Host "Skipped $Label (file not present)."
        return
    }
    $content = Get-Content -LiteralPath $FilePath -Raw
    $changed = $false
    foreach ($pattern in $BlockPatterns) {
        $result = Add-MissingReposToBlock -Content $content -BlockPattern $pattern -BlockLabel $Label
        $content = $result.Content
        if ($result.Changed) { $changed = $true }
    }
    if ($changed) {
        [IO.File]::WriteAllText($FilePath, $content, [Text.UTF8Encoding]::new($false))
    } else {
        Write-Host "No change needed for $Label."
    }
}

# 1. buildSrc classpath: the exact block behind the CI failure. Required.
Ensure-ReposInFile -FilePath $buildSrcFile -Label "buildSrc repositories" -Required $true -BlockPatterns @(
    "repositories\s*\{"
)

# 2. settings: plugin resolution and dependency resolution each consult their
# own repositories block; patch both when present.
Ensure-ReposInFile -FilePath $settingsFile -Label "settings pluginManagement" -Required $false -BlockPatterns @(
    "pluginManagement\s*\{[\s\S]*?repositories\s*\{"
)
Ensure-ReposInFile -FilePath $settingsFile -Label "settings dependencyResolutionManagement" -Required $false -BlockPatterns @(
    "dependencyResolutionManagement\s*\{[\s\S]*?repositories\s*\{"
)

# 3. Root build file: best effort, some templates declare repositories here.
Ensure-ReposInFile -FilePath $rootBuildFile -Label "root repositories" -Required $false -BlockPatterns @(
    "repositories\s*\{"
)

Write-Host "Android Gradle repositories patched."
