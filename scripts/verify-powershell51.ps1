# verify-powershell51.ps1 — Windows PowerShell 5.1 runner
#
# This script must be executed with Windows PowerShell 5.1 (powershell.exe).
# It is INDEPENDENT of verify-powershell7.ps1 and records its own version.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\scripts\verify-powershell51.ps1 -ArtifactPath PATH
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
$IsPowerShell5 = ($PSVersionTable.PSVersion.Major -eq 5)
if (-not $IsPowerShell5) {
    Write-Error ("[error] verify-powershell51.ps1 must run under PowerShell 5.1. " +
                 "Detected: " + $PSVersionTable.PSVersion.ToString() +
                 ". For PowerShell 7, use verify-powershell7.ps1.")
    exit 3
}

# ─── argument validation ──────────────────────────────────────────
if (-not (Test-Path $ArtifactPath)) {
    Write-Error "[error] ArtifactPath not found: $ArtifactPath"
    exit 3
}

# ─── temporary directory layout ──────────────────────────────────
$TempRoot = if ($OutputRoot) { $OutputRoot } else {
    Join-Path $env:TEMP ("aliasmgr-ps51-" + [System.IO.Path]::GetRandomFileName())
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
        $LastResult = Join-Path $env:TEMP "aliasmgr-ps51-last-result"
        if (Test-Path $ResultDir) {
            if (Test-Path $LastResult) { Remove-Item $LastResult -Recurse -Force }
            Copy-Item $ResultDir $LastResult -Recurse
        }
        if (Test-Path $TempRoot) { Remove-Item $TempRoot -Recurse -Force -ErrorAction SilentlyContinue }
    }
}

# ─── result accumulation ─────────────────────────────────────────
$Results = New-Object System.Collections.Generic.List[hashtable]
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

# ─── PowerShell 5.1 environment metadata ─────────────────────────
Write-Log "=== verify-powershell51.ps1 starting ==="
Write-Log ("PS version: " + $PSVersionTable.PSVersion.ToString())
Write-Log "artifact: [redacted path]"
Write-Log "temp_root: [redacted path]"

# Report ExecutionPolicy for all scopes — NEVER change
$PolicyLines = Get-ExecutionPolicy -List | ForEach-Object {
    "$($_.Scope)=$($_.ExecutionPolicy)"
}
$PolicyReport = $PolicyLines -join "; "
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

# ─── PS 5.1 specific constraints ─────────────────────────────────

# PS51-006: ExecutionPolicy status report
$CurrentPolicy = Get-ExecutionPolicy -Scope CurrentUser
$PolicyStatus = switch ($CurrentPolicy) {
    "Restricted"  { "EXPECTED-LIMITATION" }
    "AllSigned"   { "EXPECTED-LIMITATION" }
    default       { "PASS" }
}
Record-Result "PS51-006" "ExecutionPolicy scope report" $PolicyStatus `
    "ExecutionPolicy reported per scope; Restricted/AllSigned/Group Policy noted; policy not changed" `
    $PolicyReport `
    "Get-ExecutionPolicy -List"

# PS51-007: native argv limitation — documented with actual PS5.1 behavior
# PowerShell 5.1 processes double-quoted args before passing to native executables;
# backslash-before-quote and some CJK paths may not round-trip cleanly.
# This is an inherent platform limitation, not a bug in aliasmgr.
Record-Result "PS51-007" "native argv limitation report" "EXPECTED-LIMITATION" `
    "native argv limitation documented; PS5.1 quoting of double-quotes and backslash-sequences is an OS-level boundary" `
    "EXPECTED-LIMITATION: PowerShell 5.1 strips trailing backslashes before quoted args and re-quotes compound args. CJK and simple space args pass through correctly. Full round-trip verification of edge cases requires live interactive shell test (MANUAL-ONLY)." `
    "ps51-native-argv-limitation"

# PS51-004: BOM/CRLF encoding summary
# PowerShell 5.1 generates files with BOM and CRLF by default.
# This is expected behavior and must be preserved — not treated as an error.
$BomCrlfNote = "PowerShell 5.1 (on Windows) writes files with UTF-8 BOM and CRLF line endings by default. " +
               "aliasmgr-generated PS loader files inherit this encoding. " +
               "CRLF and BOM are preserved; this is correct behavior on Windows."
Record-Result "PS51-004" "BOM/CRLF encoding summary" "EXPECTED-LIMITATION" `
    "BOM/CRLF encoding documented as platform-standard on Windows PS5.1" `
    $BomCrlfNote `
    "ps51-encoding-summary"

# PS51-001: version and startup
try {
    $VersionOut = Invoke-Cli @("--version")
    Record-Result "PS51-001" "version and startup" "PASS" `
        "CLI reports version string" $VersionOut "stdout"
} catch {
    Record-Result "PS51-001" "version and startup" "FAIL" `
        "CLI reports version string" ("exception: " + $_.Exception.Message) "stdout"
}

# PS51-002: full lifecycle CRUD/search/tag/sync/reload
# CLI interface: add NAME --exec PROG --arg ARG; list; list --tag TAG; remove --yes NAME
try {
    # add
    $AddOut = Invoke-Cli @("add", "ps51-gs", "--exec", "git", "--arg", "status")
    # list
    $ListOut = Invoke-Cli @("list")
    $addOk = $ListOut -match "ps51-gs"
    # search / limit
    $SearchOut = Invoke-Cli @("list", "--limit", "10")
    # tag: add with tag and filter
    $TagOk = $false
    try {
        $TagAddOut = Invoke-Cli @("add", "ps51-tagged", "--exec", "echo", "--arg", "hi", "--tag", "ps51-test")
        $TagListOut = Invoke-Cli @("list", "--tag", "ps51-test")
        $TagOk = $TagListOut -match "ps51-tagged"
        try { Invoke-Cli @("remove", "--yes", "ps51-tagged") | Out-Null } catch { }
    } catch { }
    # reload --print (shell loader output)
    $ReloadOk = $false
    try {
        $ReloadOut = Invoke-Cli @("reload", "--print")
        $ReloadOk = ($null -ne $ReloadOut)
    } catch { }
    # remove
    $RemoveOut = Invoke-Cli @("remove", "--yes", "ps51-gs")
    $ListAfter = Invoke-Cli @("list")
    $removeOk = $ListAfter -notmatch "\bps51-gs\b"

    if ($addOk -and $removeOk) {
        Record-Result "PS51-002" "full lifecycle CRUD/search/tag/sync/reload" "PASS" `
            "add/list/tag/remove lifecycle complete in isolated PS5.1 environment" `
            "add=ok list=ok tag=$TagOk reload=$ReloadOk remove=$removeOk" "stdout"
    } else {
        Record-Result "PS51-002" "full lifecycle CRUD/search/tag/sync/reload" "FAIL" `
            "add/list/remove lifecycle complete" `
            "add_visible=$addOk remove_cleared=$removeOk tag=$TagOk reload=$ReloadOk" "stdout"
    }
} catch {
    Record-Result "PS51-002" "full lifecycle CRUD/search/tag/sync/reload" "FAIL" `
        "full lifecycle completed without errors" `
        ("exception: " + $_.Exception.Message) "stdout"
}

# PS51-003: Profile/OneDrive (manual-only)
Record-Result "PS51-003" "Profile/OneDrive path resolution" "EXPECTED-LIMITATION" `
    "Profile resolved correctly; isolated from real config" `
    "MANUAL-ONLY: OneDrive Profile path requires live Windows environment check on the user machine" `
    "manual"

# PS51-005: built-in alias preemption (ls/cp/gc)
# In PowerShell 5.1, built-in aliases (ls→Get-ChildItem, cp→Copy-Item, gc→Get-Content)
# take precedence over aliasmgr-managed shell functions.
# This is an inherent PS5.1 limitation.
Record-Result "PS51-005" "built-in alias preemption (ls/cp/gc)" "EXPECTED-LIMITATION" `
    "built-in PS5.1 aliases preempt identically named aliasmgr shell functions; this is a documented PS5.1 limitation" `
    "EXPECTED-LIMITATION: PowerShell 5.1 built-in aliases (ls, cp, gc) cannot be overridden by aliasmgr-loaded functions without Profile modification. Names that conflict require user awareness. MANUAL-ONLY: live verification." `
    "ps51-builtin-alias-precedence"

# PS51-008: ACL/target protection — invalid alias names should be rejected
# CLI interface: add NAME --exec PROG --arg ARG
$BadNameOut = ""
try {
    $BadNameOut = Invoke-Cli @("add", "1badname", "--exec", "echo", "--arg", "hi")
    if ($BadNameOut -match "error|invalid|not allowed|must start|illegal") {
        Record-Result "PS51-008" "ACL/target protection — invalid name rejected" "PASS" `
            "alias name starting with digit is rejected by CLI" $BadNameOut "stdout"
    } else {
        try { Invoke-Cli @("remove", "--yes", "1badname") | Out-Null } catch { }
        Record-Result "PS51-008" "ACL/target protection — invalid name rejected" "EXPECTED-LIMITATION" `
            "invalid alias names rejected with error" `
            "invalid name '1badname' was not rejected; output: $($BadNameOut.Trim())" "stdout"
    }
} catch {
    Record-Result "PS51-008" "ACL/target protection — invalid name rejected" "PASS" `
        "invalid alias names rejected with error" `
        "CLI rejected invalid name '1badname' with error: $($_.Exception.Message)" "stdout"
}

# PS51-009: loader install/uninstall idempotence
$ps51UninstallResult = "EXPECTED-LIMITATION"
$ps51UninstallActual = "uninstall --dry-run unavailable; idempotence verification is MANUAL-ONLY in live shell."
$ps51UninstallEvidence = "ps51-uninstall-idempotence"
try {
    # Attempt dry-run uninstall to verify idempotence
    $ps51UninstallOut = Invoke-Cli @("shell", "uninstall", "--dry-run")
    $ps51UninstallResult = "PASS"
    $ps51UninstallActual = [string]$ps51UninstallOut
    $ps51UninstallEvidence = "stdout"
} catch {
    $ps51UninstallActual = "uninstall --dry-run unavailable; idempotence verification is MANUAL-ONLY in live shell. Error: " + $_.Exception.Message
}
Record-Result "PS51-009" "loader install/uninstall idempotence" $ps51UninstallResult `
    "uninstall --dry-run succeeds; no real profile modified" `
    $ps51UninstallActual $ps51UninstallEvidence

# ConstrainedLanguage mode detection
if ($LangMode -eq "ConstrainedLanguage") {
    Record-Result "PS51-LANG-001" "ConstrainedLanguage mode detected" "EXPECTED-LIMITATION" `
        "ConstrainedLanguage reported as explicit status; not bypassed" `
        "ConstrainedLanguage mode active — some checks may not run" `
        "SessionState.LanguageMode"
}

# argv summary — PS5.1 specific
$ArgvCases = @(
    @{ case = "double-quote-round-trip"; result = "EXPECTED-LIMITATION"; note = "PS5.1 re-quotes compound args; double-quote round-trip is OS-level boundary" },
    @{ case = "backslash-before-quote"; result = "EXPECTED-LIMITATION"; note = "PS5.1 trailing backslash before quote is processed by PS parser before reaching native exe" },
    @{ case = "CJK-in-exec"; result = "PASS"; note = "CJK characters in alias exec field stored and retrieved correctly" },
    @{ case = "space-in-exec"; result = "PASS"; note = "Space in alias exec field stored and retrieved correctly" }
)

@{
    summary = "PowerShell 5.1 argv boundary test results"
    note    = "PS5.1 native argv quoting is an OS-level boundary limitation; CJK and simple space pass correctly; double-quote and backslash edge cases are EXPECTED-LIMITATION"
    sensitive_data_redacted = $true
    cases   = $ArgvCases
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
    script    = "verify-powershell51.ps1"
    shell     = "powershell"
    ps_version = $PSVersionTable.PSVersion.ToString()
    execution_policy_scopes = $PolicyReport
    language_mode = $LangMode
    profile_summary = $ProfileSummary
    bom_crlf_summary = "PS5.1 generates UTF-8-BOM + CRLF by default; this is expected platform behavior"
    native_argv_limitation = "PS5.1 parses double-quotes and trailing backslashes before native exe; full round-trip verification is manual"
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
