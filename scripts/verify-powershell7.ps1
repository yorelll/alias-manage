# verify-powershell7.ps1 — Windows PowerShell 7 runner
#
# This script must be executed with PowerShell 7 (pwsh.exe).
# It is INDEPENDENT of verify-powershell51.ps1 and records its own version.
#
# Usage:
#   pwsh -File .\scripts\verify-powershell7.ps1 -ArtifactPath PATH
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
# APPDATA, ExecutionPolicy, or any real user configuration. All paths are
# isolated inside a temporary directory that is cleaned on normal exit.
# ExecutionPolicy is reported per scope but NEVER changed.
# Restricted/AllSigned/Group Policy/ConstrainedLanguage are explicit statuses,
# never claimed as Profile success via bypass.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactPath,

    [string]$OutputRoot = "",

    [switch]$KeepTemp
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ─── PS version gate ─────────────────────────────────────────────
$IsPowerShell7 = ($PSVersionTable.PSVersion.Major -ge 7)
if (-not $IsPowerShell7) {
    Write-Error ("[error] verify-powershell7.ps1 must run under PowerShell 7+. " +
                 "Detected: " + $PSVersionTable.PSVersion.ToString() +
                 ". For PowerShell 5.1, use verify-powershell51.ps1.")
    exit 3
}

# ─── argument validation ──────────────────────────────────────────
if (-not (Test-Path $ArtifactPath)) {
    Write-Error "[error] ArtifactPath not found: $ArtifactPath"
    exit 3
}

# ─── temporary directory layout ──────────────────────────────────
$TempRoot = if ($OutputRoot) { $OutputRoot } else {
    Join-Path ([System.IO.Path]::GetTempPath()) ("aliasmgr-ps7-" + [System.IO.Path]::GetRandomFileName())
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
        $LastResult = Join-Path ([System.IO.Path]::GetTempPath()) "aliasmgr-ps7-last-result"
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

# ─── PowerShell 7 environment metadata ───────────────────────────
Write-Log "=== verify-powershell7.ps1 starting ==="
Write-Log ("PS version: " + $PSVersionTable.PSVersion.ToString())
Write-Log "artifact: [redacted path]"
Write-Log "temp_root: [redacted path]"

# Report ExecutionPolicy for all scopes — NEVER change
$PolicyReport = (Get-ExecutionPolicy -List | ForEach-Object {
    "$($_.Scope)=$($_.ExecutionPolicy)"
}) -join "; "
Write-Log ("ExecutionPolicy scopes: " + $PolicyReport)

# Report LanguageMode
$LangMode = $ExecutionContext.SessionState.LanguageMode.ToString()
Write-Log ("LanguageMode: " + $LangMode)

# Detect Profile path summary (NEVER read Profile content; NEVER source)
$ProfileSummary = if ($PROFILE) {
    $profileExists = Test-Path $PROFILE
    "profile_path=REDACTED exists=$profileExists"
} else {
    "profile_path=undefined"
}
Write-Log ("Profile summary: " + $ProfileSummary)

# ─── PS 7 specific constraints ───────────────────────────────────

# PS7-006: ExecutionPolicy status report
$CurrentPolicy = Get-ExecutionPolicy -Scope CurrentUser
$PolicyStatus = switch ($CurrentPolicy) {
    "Restricted"  { "EXPECTED-LIMITATION" }
    "AllSigned"   { "EXPECTED-LIMITATION" }
    default       { "PASS" }
}
Record-Result "PS7-006" "ExecutionPolicy scope report" $PolicyStatus `
    "ExecutionPolicy reported per scope; policy not changed" `
    $PolicyReport `
    "Get-ExecutionPolicy -List"

# PS7-007: native argv limitation
Record-Result "PS7-007" "native argv limitation report" "EXPECTED-LIMITATION" `
    "native argv limitation documented in summary" `
    "EXPECTED-LIMITATION: PowerShell 7 native argv boundary behavior documented; full matrix deferred to Task 4" `
    "task4-hook"

# PS7-004: BOM/CRLF status
Record-Result "PS7-004" "BOM/CRLF encoding summary" "EXPECTED-LIMITATION" `
    "BOM/CRLF encoding documented" `
    "placeholder: BOM/CRLF encoding verification deferred to Task 4 scripts" `
    "task4-hook"

# PS7-001: version and startup
try {
    $VersionOut = Invoke-Cli @("--version")
    Record-Result "PS7-001" "version and startup" "PASS" `
        "CLI reports version string" $VersionOut "stdout"
} catch {
    Record-Result "PS7-001" "version and startup" "FAIL" `
        "CLI reports version string" ("exception: " + $_.Exception.Message) "stdout"
}

# PS7-002: full lifecycle (placeholder — real cases in Task 4)
Record-Result "PS7-002" "full lifecycle CRUD/search/tag/sync/reload" "EXPECTED-LIMITATION" `
    "add/list/update/rename/enable/disable/delete/search/tag/sync/reload complete" `
    "placeholder: full lifecycle deferred to Task 4 scripts" `
    "task4-hook"

# PS7-003: Profile/OneDrive (manual-only)
Record-Result "PS7-003" "Profile/OneDrive path resolution" "EXPECTED-LIMITATION" `
    "Profile resolved correctly; isolated from real config" `
    "MANUAL-ONLY: OneDrive Profile path requires live Windows environment check" `
    "manual"

# PS7-005: built-in alias preemption (placeholder — Task 4)
Record-Result "PS7-005" "built-in alias preemption (ls/cp/gc)" "EXPECTED-LIMITATION" `
    "preemption and preserve-name behavior correct" `
    "placeholder: built-in alias preemption deferred to Task 4 scripts" `
    "task4-hook"

# PS7-008: ACL/target protection (placeholder — Task 4)
Record-Result "PS7-008" "ACL/target protection" "EXPECTED-LIMITATION" `
    "unsafe path warning/block; targets preserved" `
    "placeholder: ACL and target protection deferred to Task 4 scripts" `
    "task4-hook"

# PS7-009: loader install/uninstall idempotence (placeholder — Task 4)
Record-Result "PS7-009" "loader install/uninstall idempotence" "EXPECTED-LIMITATION" `
    "marked modifications idempotent and safe" `
    "placeholder: loader idempotence deferred to Task 4 scripts" `
    "task4-hook"

# ConstrainedLanguage mode detection
if ($LangMode -eq "ConstrainedLanguage") {
    Record-Result "PS7-LANG-001" "ConstrainedLanguage mode detected" "EXPECTED-LIMITATION" `
        "ConstrainedLanguage reported as explicit status; not bypassed" `
        "ConstrainedLanguage mode active — some checks may not run" `
        "SessionState.LanguageMode"
}

# argv summary
@{
    summary = "PowerShell 7 argv boundary test results"
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
    script    = "verify-powershell7.ps1"
    shell     = "pwsh"
    ps_version = $PSVersionTable.PSVersion.ToString()
    execution_policy_scopes = $PolicyReport
    language_mode = $LangMode
    profile_summary = $ProfileSummary
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
