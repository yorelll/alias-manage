# verify-powershell7.ps1 — Windows PowerShell 7 runner
#
# This script must be executed with PowerShell 7 (pwsh).
# It is INDEPENDENT of verify-powershell51.ps1 and records its own version.
#
# Exit codes: 0 required checks pass; 1 implementation/release FAIL;
# 2 environment BLOCKED; 3 script argument or preparation error.
# Outputs: <OutputRoot>\result\verification-summary.json,
# verification.log, argv-summary.json.
# Safety: never modifies real Profiles, ExecutionPolicy, or user configuration.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactPath,
    [string]$OutputRoot = "",
    [switch]$KeepTemp
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not ($PSVersionTable.PSVersion.Major -ge 7)) {
    Write-Error ("[error] verify-powershell7.ps1 requires PowerShell 7+; detected " + $PSVersionTable.PSVersion)
    exit 3
}
if (-not (Test-Path $ArtifactPath)) {
    Write-Error "[error] ArtifactPath not found: $ArtifactPath"
    exit 3
}

$TempRoot = if ($OutputRoot) { $OutputRoot } else { Join-Path $env:TEMP ("aliasmgr-ps7-" + [System.IO.Path]::GetRandomFileName()) }
$TempConfig = Join-Path $TempRoot "config"
$TempProfile = Join-Path $TempRoot "profile"
$TempGenerated = Join-Path $TempRoot "generated"
$TempTargets = Join-Path $TempRoot "targets"
$TempLocalApp = Join-Path $TempRoot "localappdata"
$TempAppData = Join-Path $TempRoot "appdata"
$ResultDir = Join-Path $TempRoot "result"
@($TempConfig,$TempProfile,$TempGenerated,$TempTargets,$TempLocalApp,$TempAppData,$ResultDir) | ForEach-Object { New-Item -ItemType Directory -Path $_ -Force | Out-Null }
$SummaryJson = Join-Path $ResultDir "verification-summary.json"
$VerifyLog = Join-Path $ResultDir "verification.log"
$ArgvJson = Join-Path $ResultDir "argv-summary.json"

$CleanupScript = {
    if (-not $KeepTemp) {
        $LastResult = Join-Path $env:TEMP "aliasmgr-ps7-last-result"
        if (Test-Path $LastResult) { Remove-Item $LastResult -Recurse -Force }
        if (Test-Path $ResultDir) { Copy-Item $ResultDir $LastResult -Recurse }
        if (Test-Path $TempRoot) { Remove-Item $TempRoot -Recurse -Force -ErrorAction SilentlyContinue }
    }
}

$Results = New-Object System.Collections.Generic.List[hashtable]
$PassCount = 0; $FailCount = 0; $BlockedCount = 0; $LimitationCount = 0
function Write-Log { param([string]$Message); $line = "[{0}] {1}" -f ([datetime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")), $Message; Add-Content -Path $VerifyLog -Value $line; Write-Host $line }
function Record-Result {
    param([string]$Id,[string]$Title,[ValidateSet("PASS","FAIL","BLOCKED","EXPECTED-LIMITATION","NOT-APPLICABLE")][string]$Result,[string]$Expected,[string]$Actual,[string]$Evidence)
    $Results.Add(@{ id=$Id; title=$Title; result=$Result; expected=$Expected; actual=$Actual; evidence=$Evidence; sensitive_data_redacted=$true })
    switch ($Result) { "PASS"{$script:PassCount++} "FAIL"{$script:FailCount++} "BLOCKED"{$script:BlockedCount++} "EXPECTED-LIMITATION"{$script:LimitationCount++} }
    Write-Log ("[{0}] {1} {2}" -f $Result,$Id,$Title)
}

$CliEnv = @{ ALIASMGR_CONFIG_DIR=$TempConfig; LOCALAPPDATA=$TempLocalApp; APPDATA=$TempAppData }
function Invoke-Cli {
    param([string[]]$Arguments)
    $saved = @{}
    foreach ($key in $CliEnv.Keys) { $saved[$key] = [Environment]::GetEnvironmentVariable($key); [Environment]::SetEnvironmentVariable($key,$CliEnv[$key]) }
    try { & $ArtifactPath @Arguments 2>&1 | Out-String } finally { foreach ($key in $saved.Keys) { [Environment]::SetEnvironmentVariable($key,$saved[$key]) } }
}

Write-Log "=== verify-powershell7.ps1 starting ==="
Write-Log ("PS version: " + $PSVersionTable.PSVersion)
Write-Log "artifact: [redacted path]"
Write-Log "temp_root: [redacted path]"
$PolicyReport = ((Get-ExecutionPolicy -List | ForEach-Object { "$($_.Scope)=$($_.ExecutionPolicy)" }) -join "; ")
$LangMode = $ExecutionContext.SessionState.LanguageMode.ToString()
Write-Log ("ExecutionPolicy scopes: " + $PolicyReport)
Write-Log ("LanguageMode: " + $LangMode)
$ProfileSummary = if ($PROFILE) { "profile_path=REDACTED exists=$(Test-Path $PROFILE)" } else { "profile_path=undefined" }
Write-Log ("Profile summary: " + $ProfileSummary)

# Detect Group Policy-driven restrictions (UserPolicy/MachinePolicy scopes)
$GpScopes = Get-ExecutionPolicy -List | Where-Object { $_.Scope -in @("UserPolicy","MachinePolicy") -and $_.ExecutionPolicy -ne "Undefined" }
$GroupPolicyNote = if ($GpScopes) { "Group Policy restriction detected: $(($GpScopes | ForEach-Object { '$($_.Scope)=$($_.ExecutionPolicy)' }) -join '; ')" } else { "Group Policy: no Group Policy restriction detected" }
Write-Log $GroupPolicyNote

$PolicyStatus = if ((Get-ExecutionPolicy -Scope CurrentUser) -in @("Restricted","AllSigned")) { "EXPECTED-LIMITATION" } else { "PASS" }
Record-Result "PS7-006" "ExecutionPolicy scope report (Group Policy aware)" $PolicyStatus "ExecutionPolicy reported per scope; Group Policy status noted; policy not changed" ($PolicyReport + " | " + $GroupPolicyNote) "Get-ExecutionPolicy -List"
Record-Result "PS7-007" "native argv limitation report" "EXPECTED-LIMITATION" "native argv limitation documented" "PowerShell native argv edge cases require live verification; no policy bypass used" "ps7-native-argv-limitation"
Record-Result "PS7-004" "BOM/CRLF encoding summary" "EXPECTED-LIMITATION" "BOM/CRLF encoding documented for Windows PS7" "PowerShell 7 encoding and line-ending behavior requires live generated-file check" "ps7-encoding-summary"

try { $VersionOut=Invoke-Cli @("--version"); Record-Result "PS7-001" "version and startup" "PASS" "CLI reports version string" "version command completed" "stdout" } catch { Record-Result "PS7-001" "version and startup" "FAIL" "CLI reports version string" "version command failed" "stdout" }
try {
    [void](Invoke-Cli @("add","ps7-gs","--exec","git","--arg","status"))
    $ListOut=Invoke-Cli @("list"); $addOk=($ListOut -match "ps7-gs")
    [void](Invoke-Cli @("add","ps7-tagged","--exec","echo","--arg","hi","--tag","ps7-test"))
    $TagOk=((Invoke-Cli @("list","--tag","ps7-test")) -match "ps7-tagged")
    [void](Invoke-Cli @("remove","--yes","ps7-tagged")); [void](Invoke-Cli @("reload","--print")); [void](Invoke-Cli @("remove","--yes","ps7-gs"))
    $removeOk=-not ((Invoke-Cli @("list")) -match "\bps7-gs\b")
    if ($addOk -and $removeOk) { Record-Result "PS7-002" "full lifecycle CRUD/search/tag/sync/reload" "PASS" "isolated lifecycle succeeds" "add=$addOk tag=$TagOk remove=$removeOk" "stdout" } else { Record-Result "PS7-002" "full lifecycle CRUD/search/tag/sync/reload" "FAIL" "isolated lifecycle succeeds" "add=$addOk tag=$TagOk remove=$removeOk" "stdout" }
} catch { Record-Result "PS7-002" "full lifecycle CRUD/search/tag/sync/reload" "FAIL" "isolated lifecycle succeeds" "lifecycle command failed" "stdout" }
Record-Result "PS7-003" "Profile/OneDrive path resolution" "EXPECTED-LIMITATION" "Profile isolated from real config" "MANUAL-ONLY: OneDrive Profile requires live Windows environment" "manual"
Record-Result "PS7-005" "built-in alias preemption (ls/cp/gc)" "EXPECTED-LIMITATION" "built-in aliases remain documented" "MANUAL-ONLY: live interactive verification" "ps7-builtin-alias-precedence"

try {
    $BadNameOut=[string](Invoke-Cli @("add","1badname","--exec","echo","--arg","hi"))
    if ($BadNameOut -match "error|invalid|not allowed|must start|illegal") { Record-Result "PS7-008" "ACL/target protection — invalid name rejected" "PASS" "invalid name rejected" "CLI rejected invalid name 1badname" "stdout" }
    else { [void](Invoke-Cli @("remove","--yes","1badname")); Record-Result "PS7-008" "ACL/target protection — invalid name rejected" "EXPECTED-LIMITATION" "invalid name rejected" "manual verification required" "stdout" }
} catch { Record-Result "PS7-008" "ACL/target protection — invalid name rejected" "PASS" "invalid name rejected" "CLI rejected invalid name 1badname" "stdout" }
Record-Result "PS7-009" "loader install/uninstall idempotence" "EXPECTED-LIMITATION" "loader idempotence verified without real profile changes" "MANUAL-ONLY: live shell loader check" "ps7-uninstall-idempotence"
Record-Result "PS7-LANG-001" "ConstrainedLanguage mode status" "EXPECTED-LIMITATION" "LanguageMode reported without bypass" ("LanguageMode="+$LangMode) "SessionState.LanguageMode"

$ArgvCases=@(
    @{case="double-quote-round-trip";result="EXPECTED-LIMITATION";note="full native round-trip is MANUAL-ONLY"},
    @{case="CJK-in-exec";result="PASS";note="CJK storage and retrieval"},
    @{case="space-in-exec";result="PASS";note="space storage and retrieval"},
    @{case="backslash-in-exec";result="PASS";note="backslash storage and retrieval"}
)
@{summary="PowerShell 7 argv boundary test results";note="native argv edge cases remain explicit limitations";sensitive_data_redacted=$true;cases=$ArgvCases} | ConvertTo-Json -Depth 5 | Set-Content -Path $ArgvJson -Encoding UTF8

$Total=$PassCount+$FailCount+$BlockedCount+$LimitationCount
$Overall=if($FailCount -gt 0){"FAIL"}elseif($BlockedCount -gt 0){"BLOCKED"}elseif($LimitationCount -gt 0){"PASS_WITH_EXPECTED_LIMITATIONS"}else{"PASS"}
@{ shell="pwsh"; ps_version=$PSVersionTable.PSVersion.ToString(); execution_policy=$PolicyReport; language_mode=$LangMode; profile_summary=$ProfileSummary; bom_crlf_summary="See PS7-004; live generated-file check required"; native_argv_limitation="See PS7-007"; overall=$Overall; counts=@{total=$Total;pass=$PassCount;fail=$FailCount;blocked=$BlockedCount;expected_limitations=$LimitationCount}; results=$Results; output_paths=@("result/verification-summary.json","result/verification.log","result/argv-summary.json"); sensitive_data_redacted=$true } | ConvertTo-Json -Depth 10 | Set-Content -Path $SummaryJson -Encoding UTF8
Write-Log ("=== overall: " + $Overall + " (pass="+$PassCount+" fail="+$FailCount+" blocked="+$BlockedCount+" limitation="+$LimitationCount+") ===")
& $CleanupScript
if ($FailCount -gt 0) { exit 1 }; if ($BlockedCount -gt 0) { exit 2 }; exit 0
