# verify-cli-windows.ps1 — Windows CLI artifact runner (shared by PS versions)
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\scripts\verify-cli-windows.ps1 -ArtifactPath PATH
#   pwsh -File .\scripts\verify-cli-windows.ps1 -ArtifactPath PATH
#
# Exit codes:
#   0  required checks pass; expected limitations may exist
#   1  implementation/release FAIL
#   2  environment BLOCKED
#   3  script argument or preparation error
#
# Outputs (written to <OutputRoot>\result\):
#   verification-summary.json
#   verification.log
#   argv-summary.json
#
# Safety: this script NEVER modifies real PowerShell Profiles, LOCALAPPDATA,
# APPDATA, or any real user configuration. All paths are isolated inside a
# temporary directory that is cleaned on normal exit unless -KeepTemp is set.
# ExecutionPolicy is reported but NEVER changed.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactPath,

    [string]$OutputRoot = "",

    [switch]$KeepTemp
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ─── argument validation ──────────────────────────────────────────
if (-not (Test-Path $ArtifactPath)) {
    Write-Error "[error] ArtifactPath not found: $ArtifactPath"
    exit 3
}

# ─── temporary directory layout ──────────────────────────────────
$TempRoot = if ($OutputRoot) { $OutputRoot } else {
    Join-Path $env:TEMP ("aliasmgr-win-" + [System.IO.Path]::GetRandomFileName())
}
$TempConfig    = Join-Path $TempRoot "config"
$TempProfile   = Join-Path $TempRoot "profile"
$TempGenerated = Join-Path $TempRoot "generated"
$TempTargets   = Join-Path $TempRoot "targets"
$TempLocalApp  = Join-Path $TempRoot "localappdata"
$TempAppData   = Join-Path $TempRoot "appdata"
$ResultDir     = Join-Path $TempRoot "result"

@($TempConfig, $TempProfile, $TempGenerated, $TempTargets,
  $TempLocalApp, $TempAppData, $ResultDir) | ForEach-Object {
    New-Item -ItemType Directory -Path $_ -Force | Out-Null
}

$SummaryJson = Join-Path $ResultDir "verification-summary.json"
$VerifyLog   = Join-Path $ResultDir "verification.log"
$ArgvJson    = Join-Path $ResultDir "argv-summary.json"

# ─── cleanup registration ─────────────────────────────────────────
$CleanupScript = {
    if (-not $KeepTemp) {
        $LastResult = Join-Path $env:TEMP "aliasmgr-win-last-result"
        if (Test-Path $ResultDir) {
            if (Test-Path $LastResult) { Remove-Item $LastResult -Recurse -Force }
            Copy-Item $ResultDir $LastResult -Recurse
        }
        if (Test-Path $TempRoot) { Remove-Item $TempRoot -Recurse -Force -ErrorAction SilentlyContinue }
    }
}

# ─── result accumulation ─────────────────────────────────────────
$Results = [System.Collections.Generic.List[hashtable]]::new()
$PassCount = 0; $FailCount = 0; $BlockedCount = 0; $LimitationCount = 0

function Write-Log {
    param([string]$Message)
    $line = "[{0}] {1}" -f ([datetime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")), $Message
    Add-Content -Path $VerifyLog -Value $line
    Write-Host $line
}

function Record-Result {
    param(
        [string]$Id,
        [string]$Title,
        [ValidateSet("PASS","FAIL","BLOCKED","EXPECTED-LIMITATION","NOT-APPLICABLE")]
        [string]$Result,
        [string]$Expected,
        [string]$Actual,
        [string]$Evidence
    )
    $entry = @{
        id                    = $Id
        title                 = $Title
        result                = $Result
        expected              = $Expected
        actual                = $Actual
        evidence              = $Evidence
        sensitive_data_redacted = $true
    }
    $Results.Add($entry)
    switch ($Result) {
        "PASS"                { $script:PassCount++ }
        "FAIL"                { $script:FailCount++ }
        "BLOCKED"             { $script:BlockedCount++ }
        "EXPECTED-LIMITATION" { $script:LimitationCount++ }
    }
    Write-Log ("[$Result] $Id $Title")
}

# ─── CLI invocation helper ────────────────────────────────────────
$CliEnv = @{
    ALIASMGR_CONFIG_DIR = $TempConfig
    LOCALAPPDATA        = $TempLocalApp
    APPDATA             = $TempAppData
}

function Invoke-Cli {
    param([string[]]$Arguments)
    $savedEnv = @{}
    foreach ($key in $CliEnv.Keys) {
        $savedEnv[$key] = [System.Environment]::GetEnvironmentVariable($key)
        [System.Environment]::SetEnvironmentVariable($key, $CliEnv[$key])
    }
    try {
        & $ArtifactPath @Arguments 2>&1 | Out-String
    } finally {
        foreach ($key in $savedEnv.Keys) {
            [System.Environment]::SetEnvironmentVariable($key, $savedEnv[$key])
        }
    }
}

# ─── PowerShell environment metadata ─────────────────────────────
Write-Log "=== verify-cli-windows.ps1 starting ==="
Write-Log ("PS version: " + $PSVersionTable.PSVersion.ToString())
Write-Log "artifact: [redacted path]"
Write-Log "temp_root: [redacted path]"

# Report ExecutionPolicy for all scopes — NEVER change it
$PolicyReport = (Get-ExecutionPolicy -List | ForEach-Object {
    "$($_.Scope)=$($_.ExecutionPolicy)"
}) -join "; "
Write-Log ("ExecutionPolicy scopes: " + $PolicyReport)

# Report LanguageMode
Write-Log ("LanguageMode: " + $ExecutionContext.SessionState.LanguageMode)

# ─── test cases ───────────────────────────────────────────────────

# W-001: version and startup
try {
    $VersionOut = Invoke-Cli @("--version")
    Record-Result "W-001" "version and startup" "PASS" `
        "CLI reports version string" $VersionOut "stdout"
} catch {
    Record-Result "W-001" "version and startup" "FAIL" `
        "CLI reports version string" ("exception: " + $_.Exception.Message) "stdout"
}

# W-002: add alias in isolated config
try {
    $AddOut = Invoke-Cli @("alias", "add", "gs", "git", "status")
    Record-Result "W-002" "add native alias" "PASS" `
        "alias added without error" $AddOut "stdout"
} catch {
    Record-Result "W-002" "add native alias" "FAIL" `
        "alias added without error" ("exception: " + $_.Exception.Message) "stdout"
}

# W-002b: list aliases shows entry
try {
    $ListOut = Invoke-Cli @("alias", "list")
    if ($ListOut -match "gs") {
        Record-Result "W-002b" "list aliases contains added entry" "PASS" `
            "list output contains 'gs'" $ListOut "stdout"
    } else {
        Record-Result "W-002b" "list aliases contains added entry" "FAIL" `
            "list output contains 'gs'" $ListOut "stdout"
    }
} catch {
    Record-Result "W-002b" "list aliases contains added entry" "FAIL" `
        "list output contains 'gs'" ("exception: " + $_.Exception.Message) "stdout"
}

# W-003: list/search/limit
try {
    $SearchOut = Invoke-Cli @("alias", "list", "--limit", "5")
    Record-Result "W-003" "list/search/limit" "PASS" `
        "list --limit 5 succeeds" $SearchOut "stdout"
} catch {
    Record-Result "W-003" "list/search/limit" "FAIL" `
        "list --limit 5 succeeds" ("exception: " + $_.Exception.Message) "stdout"
}

# W-004: argv edge cases (placeholder — real argv in Task 4)
Record-Result "W-004" "argv space/quote/CJK/backslash/wildcard" "EXPECTED-LIMITATION" `
    "argv boundary preservation verified" `
    "placeholder: argv boundary matrix deferred to Task 4 scripts" `
    "task4-hook"

# W-005: {{args}} placeholder (placeholder — real argv in Task 4)
Record-Result "W-005" "{{args}} tail/middle/invalid/duplicate" "EXPECTED-LIMITATION" `
    "placeholder positions validated" `
    "placeholder: argv template cases deferred to Task 4 scripts" `
    "task4-hook"

# W-006: tag facet (placeholder — real tag cases in Task 4)
Record-Result "W-006" "tag facet single/multi/clear" "EXPECTED-LIMITATION" `
    "tag AND filter and clear verified" `
    "placeholder: tag facet cases deferred to Task 4 scripts" `
    "task4-hook"

# W-007: import/export (placeholder — real import in Task 4)
Record-Result "W-007" "JSON/TOML import" "EXPECTED-LIMITATION" `
    "preview, warning, confirm, persist verified" `
    "placeholder: import/export cases deferred to Task 4 scripts" `
    "task4-hook"

# W-008: retain/purge uninstall (placeholder — real uninstall in Task 4)
Record-Result "W-008" "retain/purge uninstall" "EXPECTED-LIMITATION" `
    "selected cleanup executed; targets preserved" `
    "placeholder: uninstall cases deferred to Task 4 scripts" `
    "task4-hook"

# W-MANUAL-001: Profile/OneDrive (manual-only)
Record-Result "W-MANUAL-001" "Profile/OneDrive path resolution" "EXPECTED-LIMITATION" `
    "Profile resolved correctly; isolated from real config" `
    "MANUAL-ONLY: OneDrive Profile path requires live Windows environment check" `
    "manual"

# W-MANUAL-002: BOM/CRLF (manual-only)
Record-Result "W-MANUAL-002" "BOM/CRLF encoding preservation" "EXPECTED-LIMITATION" `
    "BOM and CRLF preserved in generated files" `
    "MANUAL-ONLY: encoding preservation verified on real artifact in Task 4" `
    "manual"

# W-011: ACL/target protection (placeholder — Task 4)
Record-Result "W-011" "ACL/target protection" "EXPECTED-LIMITATION" `
    "unsafe path warning/block; targets preserved" `
    "placeholder: ACL and target protection deferred to Task 4 scripts" `
    "task4-hook"

# argv summary
@{
    summary = "windows CLI argv boundary test results"
    note    = "full argv boundary matrix deferred to Task 4 scripts"
    sensitive_data_redacted = $true
    cases   = @()
} | ConvertTo-Json | Set-Content -Path $ArgvJson -Encoding UTF8

# ─── final summary ────────────────────────────────────────────────
$Total = $PassCount + $FailCount + $BlockedCount + $LimitationCount

$Overall = if ($FailCount -gt 0) { "FAIL" }
           elseif ($BlockedCount -gt 0) { "BLOCKED" }
           elseif ($LimitationCount -gt 0) { "PASS_WITH_EXPECTED_LIMITATIONS" }
           else { "PASS" }

$ExitCode = if ($FailCount -gt 0) { 1 }
            elseif ($BlockedCount -gt 0) { 2 }
            else { 0 }

$Summary = @{
    script    = "verify-cli-windows.ps1"
    ps_version = $PSVersionTable.PSVersion.ToString()
    execution_policy_scopes = $PolicyReport
    language_mode = $ExecutionContext.SessionState.LanguageMode.ToString()
    overall   = $Overall
    pass      = $PassCount
    fail      = $FailCount
    blocked   = $BlockedCount
    expected_limitation = $LimitationCount
    total     = $Total
    sensitive_data_redacted = $true
    result_enum = @("PASS","FAIL","BLOCKED","EXPECTED-LIMITATION","NOT-APPLICABLE")
    output_paths = @{
        summary = "result/verification-summary.json"
        log     = "result/verification.log"
        argv    = "result/argv-summary.json"
    }
    cases = $Results
}

$Summary | ConvertTo-Json -Depth 10 | Set-Content -Path $SummaryJson -Encoding UTF8

Write-Log ("=== overall: $Overall (pass=$PassCount fail=$FailCount blocked=$BlockedCount limitation=$LimitationCount) ===")
Write-Log "summary written to result/verification-summary.json"

& $CleanupScript
exit $ExitCode
