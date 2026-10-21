param(
    [string]$ProjectDir = (Resolve-Path (Join-Path $PSScriptRoot "..\src-tauri")).Path
)

# Patch the generated Tauri Android Gradle project so dependency resolution no
# longer depends on a single repository mirror. Must run after
# `tauri android init` (gen/android is gitignored) and before any gradlew call.
#
# Background: the CI failure resolved `org.jetbrains.kotlin:*:2.0.21`
# (Gradle 8.14.3 Kotlin DSL dependency) from `plugins.gradle.org/m2` only — the
# plugin POM parsed but its parent BOM was absent there, and no
# `mavenCentral()` fallback was consulted. Ensuring `google()` + `mavenCentral()`
# + `gradlePluginPortal()` in every generated repositories block fixes this
# class of failure deterministically.
#
# The patch is idempotent: already-present repository entries are never
# duplicated, and files/blocks that do not exist are left untouched (except
# buildSrc, which is required).
#
# Blocks are matched by brace pairing, never by a character window: a window
# that simply looks ahead sees the repositories of the *next* block and
# wrongly concludes that the current one is already complete — which is
# exactly the failure this script exists to prevent.

$ErrorActionPreference = "Stop"

$genDir = Join-Path $ProjectDir "gen" | Join-Path -ChildPath "android"
$buildSrcFile = Join-Path $genDir "buildSrc" | Join-Path -ChildPath "build.gradle.kts"
$settingsFile = Join-Path $genDir "settings.gradle.kts"
$rootBuildFile = Join-Path $genDir "build.gradle.kts"

$requiredRepos = @("google()", "mavenCentral()", "gradlePluginPortal()")

# Body of the block that starts at the given '{' (inclusive index), or $null
# when the block is not closed. Only the block's own content is inspected, so
# a sibling block can never be mistaken for a complete one.
function Get-BlockBody {
    param(
        [string]$Content,
        [int]$OpenBraceIndex
    )
    $depth = 0
    for ($i = $OpenBraceIndex; $i -lt $Content.Length; $i++) {
        $char = $Content[$i]
        if ($char -eq '{') {
            $depth++
        }
        elseif ($char -eq '}') {
            $depth--
            if ($depth -eq 0) {
                return $Content.Substring($OpenBraceIndex + 1, $i - $OpenBraceIndex - 1)
            }
        }
    }
    return $null
}

# Index of the '{' that opens the first block matching the header pattern, or
# -1 when the pattern does not occur.
function Find-BlockOpenBrace {
    param(
        [string]$Content,
        [string]$HeaderPattern
    )
    $match = [regex]::Match($Content, $HeaderPattern)
    if (-not $match.Success) {
        return -1
    }
    $openBrace = $Content.IndexOf('{', $match.Index)
    if ($openBrace -lt 0) {
        return -1
    }
    return $openBrace
}

# Ensure the repositories block that opens at $OpenBraceIndex lists every
# required repository. Missing entries are inserted after the opening line,
# using the indentation of the entries already present.
function Ensure-ReposInBlock {
    param(
        [string]$Content,
        [int]$OpenBraceIndex,
        [string]$Label
    )
    if ($OpenBraceIndex -lt 0) {
        return @{ Content = $Content; Changed = $false }
    }
    $body = Get-BlockBody -Content $Content -OpenBraceIndex $OpenBraceIndex
    if ($null -eq $body) {
        return @{ Content = $Content; Changed = $false }
    }
    $missing = @($requiredRepos | Where-Object { $body -notmatch [regex]::Escape($_) })
    if ($missing.Count -eq 0) {
        return @{ Content = $Content; Changed = $false }
    }
    # Indentation of the first existing entry, so the injected lines match the
    # surrounding style.
    $indent = "        "
    $firstEntry = [regex]::Match($body, "(?m)^([ \t]+)\S")
    if ($firstEntry.Success) {
        $indent = $firstEntry.Groups[1].Value
    }
    $injection = ($missing | ForEach-Object { "$indent$_" }) -join "`r`n"
    # A single-line block (`repositories { gradlePluginPortal() }`) must get its
    # entries on that line; a multi-line block gets them after the opening
    # line.
    $lineEnd = $Content.IndexOf("`n", $OpenBraceIndex)
    if ($lineEnd -lt 0) {
        $lineEnd = $Content.Length
    }
    $openLine = $Content.Substring($OpenBraceIndex, $lineEnd - $OpenBraceIndex)
    $closeInLine = $openLine.LastIndexOf('}')
    if ($closeInLine -ge 0) {
        $inline = ($missing | ForEach-Object { "$_" }) -join " "
        $insertAt = $OpenBraceIndex + $closeInLine
        $patched = $Content.Substring(0, $insertAt) + "$inline " + $Content.Substring($insertAt)
    } else {
        # Insert after the opening brace's line break.
        $afterBrace = $OpenBraceIndex + 1
        $rest = $Content.Substring($afterBrace)
        $newline = $rest.IndexOf("`n")
        if ($newline -lt 0) {
            $insertAt = $afterBrace
        } else {
            $insertAt = $afterBrace + $newline + 1
        }
        $patched = $Content.Substring(0, $insertAt) + $injection + "`r`n" + $Content.Substring($insertAt)
    }
    Write-Host "Patched $Label (added $($missing -join ', '))."
    return @{ Content = $patched; Changed = $true }
}

# Ensure a top-level repositories block (buildSrc, root build file) lists
# every required repository.
function Ensure-ReposInFile {
    param(
        [string]$FilePath,
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
    $openBrace = Find-BlockOpenBrace -Content $content -HeaderPattern "repositories\s*\{"
    $result = Ensure-ReposInBlock -Content $content -OpenBraceIndex $openBrace -Label $Label
    if ($result.Changed) {
        [IO.File]::WriteAllText($FilePath, $result.Content, [Text.UTF8Encoding]::new($false))
    } else {
        Write-Host "No change needed for $Label."
    }
}

# 1. buildSrc classpath: the exact block behind the CI failure. Required.
Ensure-ReposInFile -FilePath $buildSrcFile -Label "buildSrc repositories" -Required $true

# 2. settings: plugin resolution and dependency resolution each consult their
# own repositories block; patch both when present. Each block is located
# inside its own scope, so the dependency-resolution repositories can never be
# mistaken for the plugin ones.
if (Test-Path -LiteralPath $settingsFile -PathType Leaf) {
    $content = Get-Content -LiteralPath $settingsFile -Raw
    $changed = $false
    foreach ($scope in @(
            @{ Pattern = "pluginManagement\s*\{"; Label = "settings pluginManagement" },
            @{ Pattern = "dependencyResolutionManagement\s*\{"; Label = "settings dependencyResolutionManagement" }
        )) {
        $scopeOpen = Find-BlockOpenBrace -Content $content -HeaderPattern $scope.Pattern
        if ($scopeOpen -lt 0) {
            continue
        }
        $scopeBody = Get-BlockBody -Content $content -OpenBraceIndex $scopeOpen
        if ($null -eq $scopeBody) {
            continue
        }
        $reposInScope = [regex]::Match($scopeBody, "repositories\s*\{")
        if (-not $reposInScope.Success) {
            continue
        }
        # Absolute index of the repositories '{' inside this scope.
        $reposOpen = $scopeOpen + 1 + $scopeBody.IndexOf('{', $reposInScope.Index)
        $result = Ensure-ReposInBlock -Content $content -OpenBraceIndex $reposOpen -Label $scope.Label
        $content = $result.Content
        if ($result.Changed) {
            $changed = $true
        }
    }
    if ($changed) {
        [IO.File]::WriteAllText($settingsFile, $content, [Text.UTF8Encoding]::new($false))
    } else {
        Write-Host "No change needed for settings."
    }
} else {
    Write-Host "Skipped settings (file not present)."
}

# 3. Root build file: best effort, some templates declare repositories here.
Ensure-ReposInFile -FilePath $rootBuildFile -Label "root repositories" -Required $false

Write-Host "Android Gradle repositories patched."
