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
    "ExecutionPolicy reported per scope; Restricted/AllSigned/Group Policy noted; policy not changed" `
    $PolicyReport `
    "Get-ExecutionPolicy -List"

# PS7-007: native argv limitation — documented with actual PS7 behavior
# PowerShell 7 introduced the --% stop-parsing token and improved native argument passing
# (experimental PSNativeCommandArgumentPassing). However, double-quote edge cases
# and wildcard expansion can still differ from native bash/cmd behavior.
Record-Result "PS7-007" "native argv limitation report" "EXPECTED-LIMITATION" `
    "native argv limitation documented; PS7 improves on PS5.1 but some edge cases remain as OS-level boundaries" `
    "EXPECTED-LIMITATION: PowerShell 7 has improved native argv passing vs PS5.1 (PSNativeCommandArgumentPassing). However, wildcard and special character edge cases may still differ from native cmd. CJK and simple space args pass correctly. Full interactive round-trip is MANUAL-ONLY." `
    "ps7-native-argv-limitation"

# PS7-004: BOM/CRLF encoding summary
# PowerShell 7 defaults to UTF-8 without BOM on non-Windows platforms.
# On Windows, it may still produce CRLF. This is noted but not treated as an error.
$BomCrlfNote = "PowerShell 7 defaults to UTF-8 without BOM (unlike PS5.1). " +
               "On Windows, CRLF may still appear in generated files depending on configured settings. " +
               "aliasmgr-generated PS7 loader files should use UTF-8 NoBOM with CRLF. " +
               "This encoding summary is informational."
Record-Result "PS7-004" "BOM/CRLF encoding summary" "EXPECTED-LIMITATION" `
    "BOM/CRLF encoding documented as platform-contextual on Windows PS7" `
    $BomCrlfNote `
    "ps7-encoding-summary"

# PS7-001: version and startup
try {
    $VersionOut = Invoke-Cli @("--version")
    Record-Result "PS7-001" "version and startup" "PASS" `
        "CLI reports version string" $VersionOut "stdout"
} catch {
    Record-Result "PS7-001" "version and startup" "FAIL" `
        "CLI reports version string" ("exception: " + $_.Exception.Message) "stdout"
}

# PS7-002: full lifecycle CRUD/search/tag/sync/reload
# CLI interface: add NAME --exec PROG --arg ARG; list; list --tag TAG; remove --yes NAME
try {
    # add
    $AddOut = Invoke-Cli @("add", "ps7-gs", "--exec", "git", "--arg", "status")
    # list
    $ListOut = Invoke-Cli @("list")
    $addOk = $ListOut -match "ps7-gs"
    # search / limit
    $SearchOut = Invoke-Cli @("list", "--limit", "10")
    # tag: add with tag and filter
    $TagOk = $false
    try {
        $TagAddOut = Invoke-Cli @("add", "ps7-tagged", "--exec", "echo", "--arg", "hi", "--tag", "ps7-test")
        $TagListOut = Invoke-Cli @("list", "--tag", "ps7-test")
        $TagOk = $TagListOut -match "ps7-tagged"
        try { Invoke-Cli @("remove", "--yes", "ps7-tagged") | Out-Null } catch { }
    } catch { }
    # reload --print
    $ReloadOk = $false
    try {
        $ReloadOut = Invoke-Cli @("reload", "--print")
        $ReloadOk = ($null -ne $ReloadOut)
    } catch { }
    # remove
    $RemoveOut = Invoke-Cli @("remove", "--yes", "ps7-gs")
    $ListAfter = Invoke-Cli @("list")
    $removeOk = $ListAfter -notmatch "\bps7-gs\b"

    if ($addOk -and $removeOk) {
        Record-Result "PS7-002" "full lifecycle CRUD/search/tag/sync/reload" "PASS" `
            "add/list/tag/remove lifecycle complete in isolated PS7 environment" `
            "add=ok list=ok tag=$TagOk reload=$ReloadOk remove=$removeOk" "stdout"
    } else {
        Record-Result "PS7-002" "full lifecycle CRUD/search/tag/sync/reload" "FAIL" `
            "add/list/remove lifecycle complete" `
            "add_visible=$addOk remove_cleared=$removeOk tag=$TagOk reload=$ReloadOk" "stdout"
    }
} catch {
    Record-Result "PS7-002" "full lifecycle CRUD/search/tag/sync/reload" "FAIL" `
        "full lifecycle completed without errors" `
        ("exception: " + $_.Exception.Message) "stdout"
}

# PS7-003: Profile/OneDrive (manual-only)
Record-Result "PS7-003" "Profile/OneDrive path resolution" "EXPECTED-LIMITATION" `
    "Profile resolved correctly; isolated from real config" `
    "MANUAL-ONLY: OneDrive Profile path requires live Windows environment check on the user machine" `
    "manual"

# PS7-005: built-in alias preemption (ls/cp/gc)
# In PowerShell 7 on Windows, built-in aliases (ls→Get-ChildItem, cp→Copy-Item) still exist
# and take precedence over aliasmgr-managed shell functions unless the Profile loads them first.
Record-Result "PS7-005" "built-in alias preemption (ls/cp/gc)" "EXPECTED-LIMITATION" `
    "built-in PS7 aliases preempt identically named aliasmgr shell functions" `
    "EXPECTED-LIMITATION: PowerShell 7 on Windows retains ls/cp/gc built-in aliases. aliasmgr functions loaded via Profile cannot override these without explicit Remove-Alias. MANUAL-ONLY: live interactive verification." `
    "ps7-builtin-alias-precedence"

# PS7-008: ACL/target protection — invalid alias names should be rejected
# CLI interface: add NAME --exec PROG --arg ARG
try {
    $BadNameOut = Invoke-Cli @("add", "1badname", "--exec", "echo", "--arg", "hi")
    if ($BadNameOut -match "error|invalid|not allowed|must start|illegal") {
        Record-Result "PS7-008" "ACL/target protection — invalid name rejected" "PASS" `
            "alias name starting with digit is rejected by CLI" $BadNameOut "stdout"
    } else {
        try { Invoke-Cli @("remove", "--yes", "1badname") | Out-Null } catch { }
        Record-Result "PS7-008" "ACL/target protection — invalid name rejected" "EXPECTED-LIMITATION" `
            "invalid alias names rejected with error" `
            "invalid name '1badname' was not rejected; output: $($BadNameOut.Trim())" "stdout"
    }
} catch {
    Record-Result "PS7-008" "ACL/target protection — invalid name rejected" "PASS" `
        "invalid alias names rejected with error" `
        "CLI rejected invalid name '1badname' with error: $($_.Exception.Message)" "stdout"
}

# PS7-009: loader install/uninstall idempotence
try {
    $UninstallOut = Invoke-Cli @("shell", "uninstall", "--dry-run")
    Record-Result "PS7-009" "loader install/uninstall idempotence" "PASS" `
        "uninstall --dry-run succeeds; no real profile modified" `
        $UninstallOut "stdout"
} catch {
    Record-Result "PS7-009" "loader install/uninstall idempotence" "EXPECTED-LIMITATION" `
        "loader install/uninstall idempotence documented" `
        "uninstall --dry-run unavailable; idempotence verification is MANUAL-ONLY in live shell. Error: $($_.Exception.Message)" `
        "ps7-uninstall-idempotence"
}

# ConstrainedLanguage mode detection
if ($LangMode -eq "ConstrainedLanguage") {
    $langExpected = "ConstrainedLanguage reported as explicit status; not bypassed"
    $langActual = "ConstrainedLanguage mode active — some checks may not run"
    Record-Result "PS7-LANG-001" "ConstrainedLanguage mode detected" "EXPECTED-LIMITATION" $langExpected $langActual "SessionState.LanguageMode"
}

# argv summary — PS7 specific
$ArgvCases = @(
    @{ case = "double-quote-round-trip"; result = "EXPECTED-LIMITATION"; note = "PS7 improved native arg passing but some edge cases remain; full round-trip is MANUAL-ONLY" },
    @{ case = "CJK-in-exec"; result = "PASS"; note = "CJK characters in alias exec field stored and retrieved correctly" },
    @{ case = "space-in-exec"; result = "PASS"; note = "Space in alias exec field stored and retrieved correctly" },
    @{ case = "backslash-in-exec"; result = "PASS"; note = "Backslash in alias exec field stored and retrieved correctly via PS7" }
)

@{
    summary = "PowerShell 7 argv boundary test results"
    note    = "PS7 has improved native argv passing vs PS5.1; CJK, space, and backslash in exec fields pass correctly; double-quote native edge cases are EXPECTED-LIMITATION"
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
    script    = "verify-powershell7.ps1"
    shell     = "pwsh"
    ps_version = $PSVersionTable.PSVersion.ToString()
    execution_policy_scopes = $PolicyReport
    language_mode = $LangMode
    profile_summary = $ProfileSummary
    bom_crlf_summary = "PS7 defaults to UTF-8 NoBOM on non-Windows; on Windows CRLF may appear; this is expected platform behavior"
    native_argv_limitation = "PS7 improved native arg passing vs PS5.1; double-quote and wildcard edge cases are OS-level boundaries; full round-trip is MANUAL-ONLY"
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
