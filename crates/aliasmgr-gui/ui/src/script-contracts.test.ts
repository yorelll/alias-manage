// script-contracts.test.ts — Source-contract tests for terminal verification scripts
//
// These tests run in Node (node:test) and assert that each verification script
// declares the required output paths, uses the exact result enum values,
// creates a temporary config root, and contains no environment-dump commands.
//
// No local Shell, Cargo, Rust, or PowerShell execution is performed.
// All assertions are source-text checks on the script files only.

import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

// Resolve script paths relative to this file (ui/src/) → ../../../../scripts/
const __dirname = dirname(fileURLToPath(import.meta.url));
const SCRIPTS_DIR = resolve(__dirname, "../../../../scripts");

async function readScript(name: string): Promise<string> {
  const path = resolve(SCRIPTS_DIR, name);
  try {
    return await readFile(path, "utf8");
  } catch (err) {
    throw new Error(`Failed to read script ${name}: ${err}`);
  }
}

// ─── shared contract: output paths ───────────────────────────────

const REQUIRED_OUTPUT_PATHS = [
  "result/verification-summary.json",
  "result/verification.log",
  "result/argv-summary.json",
];

// ─── shared contract: result enum values ─────────────────────────

const REQUIRED_ENUM_VALUES = [
  "PASS",
  "FAIL",
  "BLOCKED",
  "EXPECTED-LIMITATION",
  "NOT-APPLICABLE",
];

// ─── shared contract: forbidden environment-dump patterns ────────

// These commands dump full environment state and must not appear in scripts.
// Note: `env VAR=value cmd` is allowed (it sets specific vars for a command).
// Only bare `env` on its own line (no arguments after it) is forbidden.
const FORBIDDEN_PATTERNS = [
  /^\s*env\s*$/m,                // bare `env` alone on a line (dumps all vars)
  /\bprintenv\b(?!\s+\w)/,      // printenv without a specific var name
  /\bset\s+-x\b/,                // bash set -x (execution trace)
  /\$env:PATH\b.*Write-Host/i,   // PS: printing full PATH
  /Get-ChildItem\s+env:/i,       // PS: listing all env vars
];

// ─── helpers ─────────────────────────────────────────────────────

function assertOutputPaths(source: string, scriptName: string): void {
  for (const path of REQUIRED_OUTPUT_PATHS) {
    assert.match(
      source,
      new RegExp(path.replace("/", "[/\\\\]")),
      `${scriptName} must declare output path: ${path}`
    );
  }
}

function assertEnumValues(source: string, scriptName: string): void {
  for (const value of REQUIRED_ENUM_VALUES) {
    assert.match(
      source,
      new RegExp(`["']${value}["']`),
      `${scriptName} must use exact enum value: ${value}`
    );
  }
}

function assertTempConfigRoot(source: string, scriptName: string): void {
  // Every script must create a temporary config directory
  assert.match(
    source,
    /ALIASMGR_CONFIG_DIR|TempConfig/,
    `${scriptName} must declare an isolated config root (ALIASMGR_CONFIG_DIR or TempConfig)`
  );
  // Must reference a temp directory pattern
  assert.match(
    source,
    /mktemp|TEMP_ROOT|TempRoot|GetRandomFileName/,
    `${scriptName} must create a temporary root directory`
  );
}

function assertNoEnvironmentDump(source: string, scriptName: string): void {
  for (const pattern of FORBIDDEN_PATTERNS) {
    assert.doesNotMatch(
      source,
      pattern,
      `${scriptName} must not contain environment-dump command matching: ${pattern}`
    );
  }
}

function assertSensitiveDataRedacted(source: string, scriptName: string): void {
  assert.match(
    source,
    /sensitive_data_redacted/,
    `${scriptName} must declare sensitive_data_redacted in output`
  );
}

function assertCleanupRegistered(source: string, scriptName: string): void {
  // Scripts must register a cleanup handler
  assert.match(
    source,
    /trap\s+cleanup\s+EXIT|CleanupScript|\$CleanupScript/,
    `${scriptName} must register a cleanup handler`
  );
}

function assertExitCodeContract(source: string, scriptName: string): void {
  // Scripts must define and use exit codes 0, 1, 2, 3.
  // Bash: EXIT_CODE=1 / EXIT_CODE=2 / exit $EXIT_CODE or exit 3
  // PS:   $ExitCode = if (...) { 1 } elseif (...) { 2 } else { 0 }; exit $ExitCode
  assert.match(source, /exit\s+\$EXIT_CODE|exit\s+\$ExitCode|\bexit\s+0\b/,
    `${scriptName} must use an exit-code variable or exit 0`);
  assert.match(source, /EXIT_CODE=1|\{\s*1\s*\}|\bexit\s+1\b/,
    `${scriptName} must use exit code 1 for FAIL`);
  assert.match(source, /EXIT_CODE=2|\{\s*2\s*\}|\bexit\s+2\b/,
    `${scriptName} must use exit code 2 for BLOCKED`);
  assert.match(source, /\bexit\s+3\b/,
    `${scriptName} must use exit code 3 for argument/preparation error`);
}

function assertPassWithLimitations(source: string, scriptName: string): void {
  assert.match(
    source,
    /PASS_WITH_EXPECTED_LIMITATIONS/,
    `${scriptName} must distinguish PASS_WITH_EXPECTED_LIMITATIONS from all-pass`
  );
}

function assertNoRealProfileModification(source: string, scriptName: string): void {
  // Must not source or write to the real ~/.bashrc or ~/.zshrc (outside comments).
  // Comments explaining the safety guarantee are allowed; actual code references are not.
  // We match only executable-looking references: variable assignments or command arguments
  // that point at the literal home-relative path, excluding comment lines.
  const nonCommentLines = source
    .split("\n")
    .filter((line) => !line.trimStart().startsWith("#"))
    .join("\n");
  assert.doesNotMatch(
    nonCommentLines,
    /["']?\$HOME\/\.bashrc\b|[^{]~\/\.bashrc\b/,
    `${scriptName} must not reference real ~/.bashrc in executable code`
  );
  assert.doesNotMatch(
    nonCommentLines,
    /["']?\$HOME\/\.zshrc\b|[^{]~\/\.zshrc\b/,
    `${scriptName} must not reference real ~/.zshrc in executable code`
  );
}

// ─── verify-cli-linux.sh ─────────────────────────────────────────

test("verify-cli-linux.sh declares all required output paths", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertOutputPaths(source, "verify-cli-linux.sh");
});

test("verify-cli-linux.sh uses exact result enum values", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertEnumValues(source, "verify-cli-linux.sh");
});

test("verify-cli-linux.sh creates a temporary config root", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertTempConfigRoot(source, "verify-cli-linux.sh");
});

test("verify-cli-linux.sh contains no environment-dump commands", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertNoEnvironmentDump(source, "verify-cli-linux.sh");
});

test("verify-cli-linux.sh marks sensitive data as redacted", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertSensitiveDataRedacted(source, "verify-cli-linux.sh");
});

test("verify-cli-linux.sh registers cleanup on exit", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertCleanupRegistered(source, "verify-cli-linux.sh");
});

test("verify-cli-linux.sh uses exit codes 0/1/2/3", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertExitCodeContract(source, "verify-cli-linux.sh");
});

test("verify-cli-linux.sh distinguishes PASS_WITH_EXPECTED_LIMITATIONS", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertPassWithLimitations(source, "verify-cli-linux.sh");
});

test("verify-cli-linux.sh does not modify real profile paths", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assertNoRealProfileModification(source, "verify-cli-linux.sh");
});

// ─── verify-bash-linux.sh ────────────────────────────────────────

test("verify-bash-linux.sh declares all required output paths", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertOutputPaths(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh uses exact result enum values", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertEnumValues(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh creates a temporary config root", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertTempConfigRoot(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh contains no environment-dump commands", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertNoEnvironmentDump(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh marks sensitive data as redacted", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertSensitiveDataRedacted(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh registers cleanup on exit", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertCleanupRegistered(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh uses exit codes 0/1/2/3", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertExitCodeContract(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh distinguishes PASS_WITH_EXPECTED_LIMITATIONS", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertPassWithLimitations(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh does not modify real profile paths", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assertNoRealProfileModification(source, "verify-bash-linux.sh");
});

test("verify-bash-linux.sh isolates RC to a temporary path", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(
    source,
    /TEMP_BASHRC|TEMP_PROFILE/,
    "verify-bash-linux.sh must use an isolated RC file path"
  );
  assert.match(
    source,
    /TEMP_PROFILE.*\.bashrc|TEMP_BASHRC/,
    "verify-bash-linux.sh must write .bashrc inside the temp profile dir"
  );
});

test("verify-bash-linux.sh marks manual-only checks explicitly", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(
    source,
    /MANUAL-ONLY/,
    "verify-bash-linux.sh must mark manual-only checks"
  );
});

// ─── verify-zsh-linux.sh ─────────────────────────────────────────

test("verify-zsh-linux.sh declares all required output paths", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertOutputPaths(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh uses exact result enum values", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertEnumValues(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh creates a temporary config root", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertTempConfigRoot(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh contains no environment-dump commands", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertNoEnvironmentDump(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh marks sensitive data as redacted", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertSensitiveDataRedacted(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh registers cleanup on exit", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertCleanupRegistered(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh uses exit codes 0/1/2/3", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertExitCodeContract(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh distinguishes PASS_WITH_EXPECTED_LIMITATIONS", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertPassWithLimitations(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh does not modify real profile paths", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assertNoRealProfileModification(source, "verify-zsh-linux.sh");
});

test("verify-zsh-linux.sh isolates RC to a temporary path", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(
    source,
    /TEMP_ZSHRC|TEMP_PROFILE/,
    "verify-zsh-linux.sh must use an isolated RC file path"
  );
  assert.match(
    source,
    /TEMP_PROFILE.*\.zshrc|TEMP_ZSHRC/,
    "verify-zsh-linux.sh must write .zshrc inside the temp profile dir"
  );
});

test("verify-zsh-linux.sh isolates ZDOTDIR to temp", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(
    source,
    /ZDOTDIR.*TEMP/,
    "verify-zsh-linux.sh must isolate ZDOTDIR"
  );
});

test("verify-zsh-linux.sh marks manual-only checks explicitly", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(
    source,
    /MANUAL-ONLY/,
    "verify-zsh-linux.sh must mark manual-only checks"
  );
});

test("verify-zsh-linux.sh marks oh-my-zsh ordering as manual", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(
    source,
    /oh-my-zsh/i,
    "verify-zsh-linux.sh must reference oh-my-zsh ordering as a manual check"
  );
});

// ─── verify-cli-windows.ps1 ──────────────────────────────────────

test("verify-cli-windows.ps1 declares all required output paths", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assertOutputPaths(source, "verify-cli-windows.ps1");
});

test("verify-cli-windows.ps1 uses exact result enum values", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assertEnumValues(source, "verify-cli-windows.ps1");
});

test("verify-cli-windows.ps1 creates a temporary config root", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assertTempConfigRoot(source, "verify-cli-windows.ps1");
});

test("verify-cli-windows.ps1 marks sensitive data as redacted", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assertSensitiveDataRedacted(source, "verify-cli-windows.ps1");
});

test("verify-cli-windows.ps1 registers cleanup", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assertCleanupRegistered(source, "verify-cli-windows.ps1");
});

test("verify-cli-windows.ps1 uses exit codes 0/1/2/3", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assertExitCodeContract(source, "verify-cli-windows.ps1");
});

test("verify-cli-windows.ps1 distinguishes PASS_WITH_EXPECTED_LIMITATIONS", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assertPassWithLimitations(source, "verify-cli-windows.ps1");
});

test("verify-cli-windows.ps1 reports ExecutionPolicy without changing it", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(
    source,
    /Get-ExecutionPolicy/,
    "verify-cli-windows.ps1 must report ExecutionPolicy"
  );
  assert.doesNotMatch(
    source,
    /Set-ExecutionPolicy/,
    "verify-cli-windows.ps1 must not change ExecutionPolicy"
  );
});

test("verify-cli-windows.ps1 reports LanguageMode", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(
    source,
    /LanguageMode/,
    "verify-cli-windows.ps1 must report LanguageMode"
  );
});

test("verify-cli-windows.ps1 isolates LOCALAPPDATA and APPDATA", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(
    source,
    /TempLocalApp|LOCALAPPDATA/,
    "verify-cli-windows.ps1 must isolate LOCALAPPDATA"
  );
  assert.match(
    source,
    /TempAppData|APPDATA/,
    "verify-cli-windows.ps1 must isolate APPDATA"
  );
});

// ─── verify-powershell51.ps1 ─────────────────────────────────────

test("verify-powershell51.ps1 declares all required output paths", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assertOutputPaths(source, "verify-powershell51.ps1");
});

test("verify-powershell51.ps1 uses exact result enum values", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assertEnumValues(source, "verify-powershell51.ps1");
});

test("verify-powershell51.ps1 creates a temporary config root", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assertTempConfigRoot(source, "verify-powershell51.ps1");
});

test("verify-powershell51.ps1 marks sensitive data as redacted", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assertSensitiveDataRedacted(source, "verify-powershell51.ps1");
});

test("verify-powershell51.ps1 registers cleanup", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assertCleanupRegistered(source, "verify-powershell51.ps1");
});

test("verify-powershell51.ps1 uses exit codes 0/1/2/3", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assertExitCodeContract(source, "verify-powershell51.ps1");
});

test("verify-powershell51.ps1 distinguishes PASS_WITH_EXPECTED_LIMITATIONS", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assertPassWithLimitations(source, "verify-powershell51.ps1");
});

test("verify-powershell51.ps1 rejects non-PS5 execution", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(
    source,
    /PSVersion.*Major.*eq.*5|Major.*-eq.*5/,
    "verify-powershell51.ps1 must gate on PS major version 5"
  );
});

test("verify-powershell51.ps1 reports ExecutionPolicy without changing it", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /Get-ExecutionPolicy/,
    "verify-powershell51.ps1 must report ExecutionPolicy");
  assert.doesNotMatch(source, /Set-ExecutionPolicy/,
    "verify-powershell51.ps1 must not change ExecutionPolicy");
});

test("verify-powershell51.ps1 reports LanguageMode", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /LanguageMode/,
    "verify-powershell51.ps1 must report LanguageMode");
});

test("verify-powershell51.ps1 reports profile summary without reading content", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /ProfileSummary|profile_summary/,
    "verify-powershell51.ps1 must include a profile summary");
  // Must not source/read the real profile
  assert.doesNotMatch(source, /\.\s+\$PROFILE\b|\.\s+["'].*Profile/,
    "verify-powershell51.ps1 must not source the real Profile");
});

test("verify-powershell51.ps1 marks Restricted/AllSigned/ConstrainedLanguage as explicit statuses", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /Restricted/,
    "verify-powershell51.ps1 must reference Restricted as a status");
  assert.match(source, /AllSigned/,
    "verify-powershell51.ps1 must reference AllSigned as a status");
  assert.match(source, /ConstrainedLanguage/,
    "verify-powershell51.ps1 must reference ConstrainedLanguage as a status");
});

test("verify-powershell51.ps1 is independent of verify-powershell7.ps1", async () => {
  const source51 = await readScript("verify-powershell51.ps1");
  const source7  = await readScript("verify-powershell7.ps1");
  // They should record their own versions and have separate gates
  assert.match(source51, /verify-powershell51\.ps1/,
    "verify-powershell51.ps1 must self-identify");
  assert.match(source7,  /verify-powershell7\.ps1/,
    "verify-powershell7.ps1 must self-identify");
  // Each must gate on its own major version
  assert.match(source51, /-eq.*5|Major.*5/,
    "PS51 script must gate on major version 5");
  assert.match(source7,  /-ge.*7|Major.*7/,
    "PS7 script must gate on major version 7");
});

// ─── verify-powershell7.ps1 ──────────────────────────────────────

test("verify-powershell7.ps1 declares all required output paths", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assertOutputPaths(source, "verify-powershell7.ps1");
});

test("verify-powershell7.ps1 uses exact result enum values", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assertEnumValues(source, "verify-powershell7.ps1");
});

test("verify-powershell7.ps1 creates a temporary config root", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assertTempConfigRoot(source, "verify-powershell7.ps1");
});

test("verify-powershell7.ps1 marks sensitive data as redacted", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assertSensitiveDataRedacted(source, "verify-powershell7.ps1");
});

test("verify-powershell7.ps1 registers cleanup", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assertCleanupRegistered(source, "verify-powershell7.ps1");
});

test("verify-powershell7.ps1 uses exit codes 0/1/2/3", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assertExitCodeContract(source, "verify-powershell7.ps1");
});

test("verify-powershell7.ps1 distinguishes PASS_WITH_EXPECTED_LIMITATIONS", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assertPassWithLimitations(source, "verify-powershell7.ps1");
});

test("verify-powershell7.ps1 rejects non-PS7 execution", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(
    source,
    /PSVersion.*Major.*ge.*7|Major.*-ge.*7/,
    "verify-powershell7.ps1 must gate on PS major version 7+"
  );
});

test("verify-powershell7.ps1 reports ExecutionPolicy without changing it", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /Get-ExecutionPolicy/,
    "verify-powershell7.ps1 must report ExecutionPolicy");
  assert.doesNotMatch(source, /Set-ExecutionPolicy/,
    "verify-powershell7.ps1 must not change ExecutionPolicy");
});

test("verify-powershell7.ps1 reports LanguageMode", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /LanguageMode/,
    "verify-powershell7.ps1 must report LanguageMode");
});

test("verify-powershell7.ps1 reports profile summary without reading content", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /ProfileSummary|profile_summary/,
    "verify-powershell7.ps1 must include a profile summary");
  assert.doesNotMatch(source, /\.\s+\$PROFILE\b|\.\s+["'].*Profile/,
    "verify-powershell7.ps1 must not source the real Profile");
});

test("verify-powershell7.ps1 marks Restricted/AllSigned/ConstrainedLanguage as explicit statuses", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /Restricted/,
    "verify-powershell7.ps1 must reference Restricted as a status");
  assert.match(source, /AllSigned/,
    "verify-powershell7.ps1 must reference AllSigned as a status");
  assert.match(source, /ConstrainedLanguage/,
    "verify-powershell7.ps1 must reference ConstrainedLanguage as a status");
});

// ─── PS5.1 compatibility: no Join-String ─────────────────────────
// Join-String was introduced in PS7; PS5.1 requires the -join operator instead.

test("verify-cli-windows.ps1 does not use Join-String (PS5.1 incompatible)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.doesNotMatch(
    source,
    /\|\s*Join-String\b/,
    "verify-cli-windows.ps1 must not pipe to Join-String (requires PS7+); use -join instead"
  );
});

test("verify-powershell51.ps1 does not use Join-String (PS5.1 incompatible)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.doesNotMatch(
    source,
    /\|\s*Join-String\b/,
    "verify-powershell51.ps1 must not pipe to Join-String (requires PS7+); use -join instead"
  );
});

// ─── stale last-result cleanup before cp -r ──────────────────────
// Without removing the stale destination first, a repeated cp -r nests the
// directory inside the old one instead of replacing it.

test("verify-cli-linux.sh removes stale last-result before cp", async () => {
  const source = await readScript("verify-cli-linux.sh");
  // rm -rf must appear before the cp -r that writes aliasmgr-last-result
  const rmIndex = source.indexOf("rm -rf /tmp/aliasmgr-last-result");
  const cpIndex = source.indexOf("cp -r \"$RESULT_DIR\" /tmp/aliasmgr-last-result");
  assert.ok(rmIndex !== -1, "verify-cli-linux.sh must remove stale /tmp/aliasmgr-last-result before cp");
  assert.ok(cpIndex !== -1, "verify-cli-linux.sh must cp result to /tmp/aliasmgr-last-result");
  assert.ok(rmIndex < cpIndex, "verify-cli-linux.sh: rm -rf of last-result must precede the cp");
});

test("verify-bash-linux.sh removes stale last-result before cp", async () => {
  const source = await readScript("verify-bash-linux.sh");
  const rmIndex = source.indexOf("rm -rf /tmp/aliasmgr-bash-last-result");
  const cpIndex = source.indexOf("cp -r \"$RESULT_DIR\" /tmp/aliasmgr-bash-last-result");
  assert.ok(rmIndex !== -1, "verify-bash-linux.sh must remove stale /tmp/aliasmgr-bash-last-result before cp");
  assert.ok(cpIndex !== -1, "verify-bash-linux.sh must cp result to /tmp/aliasmgr-bash-last-result");
  assert.ok(rmIndex < cpIndex, "verify-bash-linux.sh: rm -rf of last-result must precede the cp");
});

test("verify-zsh-linux.sh removes stale last-result before cp", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  const rmIndex = source.indexOf("rm -rf /tmp/aliasmgr-zsh-last-result");
  const cpIndex = source.indexOf("cp -r \"$RESULT_DIR\" /tmp/aliasmgr-zsh-last-result");
  assert.ok(rmIndex !== -1, "verify-zsh-linux.sh must remove stale /tmp/aliasmgr-zsh-last-result before cp");
  assert.ok(cpIndex !== -1, "verify-zsh-linux.sh must cp result to /tmp/aliasmgr-zsh-last-result");
  assert.ok(rmIndex < cpIndex, "verify-zsh-linux.sh: rm -rf of last-result must precede the cp");
});
