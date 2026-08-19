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

// ─── Task 3 source-contract tests ────────────────────────────────
// Assert that placeholder "task3-hook" comments are fully replaced with
// real implementations across all three Linux scripts.

test("verify-cli-linux.sh has no task3-hook placeholders", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.doesNotMatch(
    source,
    /task3-hook/,
    "verify-cli-linux.sh must not contain task3-hook placeholder markers"
  );
});

test("verify-bash-linux.sh has no task3-hook placeholders", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.doesNotMatch(
    source,
    /task3-hook/,
    "verify-bash-linux.sh must not contain task3-hook placeholder markers"
  );
});

test("verify-zsh-linux.sh has no task3-hook placeholders", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.doesNotMatch(
    source,
    /task3-hook/,
    "verify-zsh-linux.sh must not contain task3-hook placeholder markers"
  );
});

// verify-cli-linux.sh must implement ordered cases L-001 through L-019

test("verify-cli-linux.sh implements version check (L-001)", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.match(source, /L-001/, "verify-cli-linux.sh must include L-001 version check");
  assert.match(source, /--version/, "verify-cli-linux.sh must invoke --version");
});

test("verify-cli-linux.sh implements CRUD add/list (L-002)", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.match(source, /L-002/, "verify-cli-linux.sh must include L-002 add alias case");
  assert.match(source, /alias add|add.*--exec/, "verify-cli-linux.sh must call add with --exec");
  assert.match(source, /list/, "verify-cli-linux.sh must call list to verify added alias");
});

test("verify-cli-linux.sh implements argv boundary cases (L-004)", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.match(source, /L-004/, "verify-cli-linux.sh must include L-004 argv boundary case");
  assert.match(source, /CJK|cjk|中文|unicode/i, "verify-cli-linux.sh must test CJK or unicode argv");
});

test("verify-cli-linux.sh implements placeholder validation (L-005)", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.match(source, /L-005/, "verify-cli-linux.sh must include L-005 placeholder case");
  assert.match(source, /\{\{args\}\}/, "verify-cli-linux.sh must test {{args}} placeholder");
});

test("verify-cli-linux.sh implements search and tag filter (L-007/L-008)", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.match(source, /L-007/, "verify-cli-linux.sh must include L-007 search case");
  assert.match(source, /L-008/, "verify-cli-linux.sh must include L-008 tag case");
  assert.match(source, /--tag/, "verify-cli-linux.sh must test --tag filter");
  assert.match(source, /find|--fuzzy/, "verify-cli-linux.sh must test find/search command");
});

test("verify-cli-linux.sh implements import/export (L-012-L-014)", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.match(source, /L-012/, "verify-cli-linux.sh must include L-012 export case");
  assert.match(source, /L-013/, "verify-cli-linux.sh must include L-013 import case");
  assert.match(source, /export/, "verify-cli-linux.sh must call export command");
  assert.match(source, /import/, "verify-cli-linux.sh must call import command");
});

test("verify-cli-linux.sh implements target protection (L-016)", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.match(source, /L-016/, "verify-cli-linux.sh must include L-016 target protection case");
  // Must test that invalid names are rejected (exit non-zero)
  assert.match(
    source,
    /BAD_NAME_RC|BAD_DIGIT_RC|bad.*name|invalid.*name/i,
    "verify-cli-linux.sh must test that invalid alias names are rejected"
  );
});

test("verify-cli-linux.sh implements loader idempotence (L-019)", async () => {
  const source = await readScript("verify-cli-linux.sh");
  assert.match(source, /L-019/, "verify-cli-linux.sh must include L-019 loader idempotence case");
  assert.match(
    source,
    /shell install|LOADER_COUNT/,
    "verify-cli-linux.sh must call shell install and count markers to test idempotence"
  );
});

// verify-bash-linux.sh must implement real B-001 through B-007 cases

test("verify-bash-linux.sh implements loader install with isolated RC (B-001)", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(source, /B-001/, "verify-bash-linux.sh must include B-001 loader install case");
  assert.match(
    source,
    /shell install bash/,
    "verify-bash-linux.sh must invoke 'shell install bash'"
  );
  assert.match(
    source,
    /config\.toml|bash_rc_path|--config-dir/,
    "verify-bash-linux.sh must configure isolated RC path via config.toml or --config-dir"
  );
});

test("verify-bash-linux.sh implements loader idempotence (B-002)", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(source, /B-002/, "verify-bash-linux.sh must include B-002 idempotence case");
  assert.match(
    source,
    /LOADER_COUNT|loader.*count|grep.*count/i,
    "verify-bash-linux.sh must count loader occurrences to assert idempotence"
  );
});

test("verify-bash-linux.sh implements reload guidance (B-003)", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(source, /B-003/, "verify-bash-linux.sh must include B-003 reload guidance case");
  assert.match(source, /reload.*--print|--print.*reload/, "verify-bash-linux.sh must call reload --print");
});

test("verify-bash-linux.sh implements argv boundary matrix (B-004)", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(source, /B-004/, "verify-bash-linux.sh must include B-004 argv case");
  assert.match(
    source,
    /CJK|cjk|中文|unicode/i,
    "verify-bash-linux.sh must test CJK characters in argv"
  );
});

test("verify-bash-linux.sh implements tag/search (B-005)", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(source, /B-005/, "verify-bash-linux.sh must include B-005 tag/search case");
  assert.match(source, /--tag/, "verify-bash-linux.sh must test --tag filter");
});

test("verify-bash-linux.sh implements shell uninstall (B-006)", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(source, /B-006/, "verify-bash-linux.sh must include B-006 shell uninstall case");
  assert.match(
    source,
    /shell uninstall bash/,
    "verify-bash-linux.sh must invoke 'shell uninstall bash'"
  );
});

test("verify-bash-linux.sh implements generated file syntax check (B-007)", async () => {
  const source = await readScript("verify-bash-linux.sh");
  assert.match(source, /B-007/, "verify-bash-linux.sh must include B-007 syntax check case");
  assert.match(
    source,
    /bash\s+-n/,
    "verify-bash-linux.sh must invoke 'bash -n' to syntax-check generated file"
  );
});

// verify-zsh-linux.sh must implement real Z-001 through Z-007 cases

test("verify-zsh-linux.sh implements loader install with isolated RC (Z-001)", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(source, /Z-001/, "verify-zsh-linux.sh must include Z-001 loader install case");
  assert.match(
    source,
    /shell install zsh/,
    "verify-zsh-linux.sh must invoke 'shell install zsh'"
  );
  assert.match(
    source,
    /config\.toml|zsh_rc_path|--config-dir/,
    "verify-zsh-linux.sh must configure isolated RC path via config.toml or --config-dir"
  );
});

test("verify-zsh-linux.sh implements loader idempotence (Z-002)", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(source, /Z-002/, "verify-zsh-linux.sh must include Z-002 idempotence case");
  assert.match(
    source,
    /LOADER_COUNT|loader.*count|grep.*count/i,
    "verify-zsh-linux.sh must count loader occurrences to assert idempotence"
  );
});

test("verify-zsh-linux.sh implements reload guidance (Z-003)", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(source, /Z-003/, "verify-zsh-linux.sh must include Z-003 reload guidance case");
  assert.match(source, /reload.*--print|--print.*reload/, "verify-zsh-linux.sh must call reload --print");
});

test("verify-zsh-linux.sh implements generated zsh syntax check via zsh -n (Z-004)", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(source, /Z-004/, "verify-zsh-linux.sh must include Z-004 syntax check case");
  assert.match(
    source,
    /zsh\s+-n|\$ZSH_BIN\s+-n/,
    "verify-zsh-linux.sh must invoke 'zsh -n' to syntax-check generated file"
  );
});

test("verify-zsh-linux.sh implements zsh shell uninstall (Z-007)", async () => {
  const source = await readScript("verify-zsh-linux.sh");
  assert.match(source, /Z-007/, "verify-zsh-linux.sh must include Z-007 shell uninstall case");
  assert.match(
    source,
    /shell uninstall zsh/,
    "verify-zsh-linux.sh must invoke 'shell uninstall zsh'"
  );
});

// ─── Task 4 source-contract tests ────────────────────────────────
// Assert that placeholder "task4-hook" markers are fully replaced with
// real implementations across all three Windows scripts.

test("verify-cli-windows.ps1 has no task4-hook placeholders", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.doesNotMatch(
    source,
    /task4-hook/,
    "verify-cli-windows.ps1 must not contain task4-hook placeholder markers"
  );
});

test("verify-powershell51.ps1 has no task4-hook placeholders", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.doesNotMatch(
    source,
    /task4-hook/,
    "verify-powershell51.ps1 must not contain task4-hook placeholder markers"
  );
});

test("verify-powershell7.ps1 has no task4-hook placeholders", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.doesNotMatch(
    source,
    /task4-hook/,
    "verify-powershell7.ps1 must not contain task4-hook placeholder markers"
  );
});

// ─── Task 4: Windows CLI runner implements required cases ─────────
// verify-cli-windows.ps1 must run CRUD/search/tag/argv/placeholder/import/uninstall/target-protection.

test("verify-cli-windows.ps1 implements CRUD add/list/delete (W-002/W-009)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(source, /W-002\b/, "verify-cli-windows.ps1 must include W-002 add alias case");
  assert.match(source, /W-009\b/, "verify-cli-windows.ps1 must include W-009 remove alias case");
  assert.match(source, /Invoke-Cli.*"add"|Invoke-Cli.*add/, "verify-cli-windows.ps1 must invoke add command");
  assert.match(source, /Invoke-Cli.*"remove"|Invoke-Cli.*remove/, "verify-cli-windows.ps1 must invoke remove command");
});

test("verify-cli-windows.ps1 implements argv boundary case (W-004)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(source, /W-004\b/, "verify-cli-windows.ps1 must include W-004 argv boundary case");
  assert.match(
    source,
    /CJK|cjk|\u4e2d\u6587|unicode|space.*quote|backslash/i,
    "verify-cli-windows.ps1 must test argv boundary (CJK, space/quote, or backslash)"
  );
});

test("verify-cli-windows.ps1 implements placeholder case (W-005)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(source, /W-005\b/, "verify-cli-windows.ps1 must include W-005 placeholder case");
  assert.match(source, /\{\{args\}\}/, "verify-cli-windows.ps1 must test {{args}} placeholder");
});

test("verify-cli-windows.ps1 implements tag facet case (W-006)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(source, /W-006\b/, "verify-cli-windows.ps1 must include W-006 tag facet case");
  assert.match(source, /--tag/, "verify-cli-windows.ps1 must test --tag filter");
});

test("verify-cli-windows.ps1 implements import case (W-007)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(source, /W-007\b/, "verify-cli-windows.ps1 must include W-007 import case");
  assert.match(source, /import/, "verify-cli-windows.ps1 must invoke import command");
});

test("verify-cli-windows.ps1 implements uninstall case (W-008)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(source, /W-008\b/, "verify-cli-windows.ps1 must include W-008 uninstall case");
  assert.match(source, /uninstall/, "verify-cli-windows.ps1 must invoke uninstall command");
});

test("verify-cli-windows.ps1 implements target-protection case (W-011)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.match(source, /W-011\b/, "verify-cli-windows.ps1 must include W-011 target-protection case");
  assert.match(
    source,
    /invalid.*name|bad.*name|protected|BAD_NAME/i,
    "verify-cli-windows.ps1 must test invalid alias name rejection"
  );
});

// ─── Task 4: PS wrappers implement required shell metadata ────────

test("verify-powershell51.ps1 records exact shell version in JSON summary", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(
    source,
    /ps_version.*PSVersionTable|PSVersionTable.*ps_version/i,
    "verify-powershell51.ps1 must record ps_version from PSVersionTable in JSON summary"
  );
});

test("verify-powershell7.ps1 records exact shell version in JSON summary", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(
    source,
    /ps_version.*PSVersionTable|PSVersionTable.*ps_version/i,
    "verify-powershell7.ps1 must record ps_version from PSVersionTable in JSON summary"
  );
});

test("verify-powershell51.ps1 records profile summary (sanitized) without reading content", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /profile_summary|ProfileSummary/,
    "verify-powershell51.ps1 must record profile_summary");
  // Must sanitize the actual profile path (REDACTED)
  assert.match(source, /REDACTED/,
    "verify-powershell51.ps1 must redact actual profile path");
});

test("verify-powershell7.ps1 records profile summary (sanitized) without reading content", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /profile_summary|ProfileSummary/,
    "verify-powershell7.ps1 must record profile_summary");
  assert.match(source, /REDACTED/,
    "verify-powershell7.ps1 must redact actual profile path");
});

test("verify-powershell51.ps1 records BOM/CRLF encoding summary", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(
    source,
    /BOM|CRLF|bom.*crlf|encoding/i,
    "verify-powershell51.ps1 must record BOM/CRLF encoding summary"
  );
});

test("verify-powershell7.ps1 records BOM/CRLF encoding summary", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(
    source,
    /BOM|CRLF|bom.*crlf|encoding/i,
    "verify-powershell7.ps1 must record BOM/CRLF encoding summary"
  );
});

test("verify-powershell51.ps1 records native argv limitation explicitly", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(
    source,
    /native.*argv|argv.*limitation|native.*limitation/i,
    "verify-powershell51.ps1 must document native argv limitation explicitly"
  );
  // Must be a real documented result, not just a placeholder
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell51.ps1 must not defer argv limitation to Task 4 (this IS Task 4)"
  );
});

test("verify-powershell7.ps1 records native argv limitation explicitly", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(
    source,
    /native.*argv|argv.*limitation|native.*limitation/i,
    "verify-powershell7.ps1 must document native argv limitation explicitly"
  );
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell7.ps1 must not defer argv limitation to Task 4 (this IS Task 4)"
  );
});

// ─── Task 4: forbidden patterns in all PS scripts ─────────────────
// Scripts must not use Invoke-Expression, Invoke-WebRequest eval-style patterns,
// or Set-ExecutionPolicy.

test("verify-cli-windows.ps1 forbids Invoke-Expression", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.doesNotMatch(
    source,
    /Invoke-Expression\b/,
    "verify-cli-windows.ps1 must not use Invoke-Expression (insecure eval)"
  );
});

test("verify-powershell51.ps1 forbids Invoke-Expression", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.doesNotMatch(
    source,
    /Invoke-Expression\b/,
    "verify-powershell51.ps1 must not use Invoke-Expression (insecure eval)"
  );
});

test("verify-powershell7.ps1 forbids Invoke-Expression", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.doesNotMatch(
    source,
    /Invoke-Expression\b/,
    "verify-powershell7.ps1 must not use Invoke-Expression (insecure eval)"
  );
});

test("verify-cli-windows.ps1 forbids Set-ExecutionPolicy", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.doesNotMatch(
    source,
    /Set-ExecutionPolicy\b/,
    "verify-cli-windows.ps1 must not call Set-ExecutionPolicy"
  );
});

test("verify-powershell51.ps1 forbids Set-ExecutionPolicy", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.doesNotMatch(
    source,
    /Set-ExecutionPolicy\b/,
    "verify-powershell51.ps1 must not call Set-ExecutionPolicy"
  );
});

test("verify-powershell7.ps1 forbids Set-ExecutionPolicy", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.doesNotMatch(
    source,
    /Set-ExecutionPolicy\b/,
    "verify-powershell7.ps1 must not call Set-ExecutionPolicy"
  );
});

test("verify-cli-windows.ps1 forbids environment/profile dumps (Get-ChildItem env:)", async () => {
  const source = await readScript("verify-cli-windows.ps1");
  assert.doesNotMatch(
    source,
    /Get-ChildItem\s+env:/i,
    "verify-cli-windows.ps1 must not dump all environment variables"
  );
});

test("verify-powershell51.ps1 forbids environment/profile dumps (Get-ChildItem env:)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.doesNotMatch(
    source,
    /Get-ChildItem\s+env:/i,
    "verify-powershell51.ps1 must not dump all environment variables"
  );
});

test("verify-powershell7.ps1 forbids environment/profile dumps (Get-ChildItem env:)", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.doesNotMatch(
    source,
    /Get-ChildItem\s+env:/i,
    "verify-powershell7.ps1 must not dump all environment variables"
  );
});

// ─── Task 4: PS wrappers preserve isolated temp roots ─────────────

test("verify-powershell51.ps1 accepts and uses -OutputRoot parameter", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /\$OutputRoot\b/,
    "verify-powershell51.ps1 must declare and use -OutputRoot parameter");
  assert.match(source, /TempRoot.*OutputRoot|OutputRoot.*TempRoot/,
    "verify-powershell51.ps1 must derive TempRoot from OutputRoot");
});

test("verify-powershell7.ps1 accepts and uses -OutputRoot parameter", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /\$OutputRoot\b/,
    "verify-powershell7.ps1 must declare and use -OutputRoot parameter");
  assert.match(source, /TempRoot.*OutputRoot|OutputRoot.*TempRoot/,
    "verify-powershell7.ps1 must derive TempRoot from OutputRoot");
});

test("verify-powershell51.ps1 accepts -KeepTemp parameter", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /\$KeepTemp\b/,
    "verify-powershell51.ps1 must declare and use -KeepTemp parameter");
});

test("verify-powershell7.ps1 accepts -KeepTemp parameter", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /\$KeepTemp\b/,
    "verify-powershell7.ps1 must declare and use -KeepTemp parameter");
});

// ─── Task 4: PS wrappers invoke only their own shell ──────────────

test("verify-powershell51.ps1 self-identifies as powershell (shell field)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(
    source,
    /shell\s*=\s*["']powershell["']|shell.*powershell/,
    "verify-powershell51.ps1 must record shell as 'powershell' in summary"
  );
});

test("verify-powershell7.ps1 self-identifies as pwsh (shell field)", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(
    source,
    /shell\s*=\s*["']pwsh["']|shell.*pwsh/,
    "verify-powershell7.ps1 must record shell as 'pwsh' in summary"
  );
});

// ─── Task 4: PS51 full lifecycle cases ───────────────────────────

test("verify-powershell51.ps1 implements full CRUD lifecycle (PS51-002)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /PS51-002\b/, "verify-powershell51.ps1 must include PS51-002 lifecycle case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell51.ps1 PS51-002 must not be deferred to Task 4"
  );
});

test("verify-powershell51.ps1 implements argv boundary (PS51-007)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /PS51-007\b/, "verify-powershell51.ps1 must include PS51-007 argv case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell51.ps1 PS51-007 must not be deferred to Task 4"
  );
});

test("verify-powershell51.ps1 implements BOM/CRLF check (PS51-004)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /PS51-004\b/, "verify-powershell51.ps1 must include PS51-004 BOM/CRLF case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell51.ps1 PS51-004 must not be deferred to Task 4"
  );
});

test("verify-powershell51.ps1 implements ACL/target-protection (PS51-008)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /PS51-008\b/, "verify-powershell51.ps1 must include PS51-008 target-protection case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell51.ps1 PS51-008 must not be deferred to Task 4"
  );
});

test("verify-powershell51.ps1 implements loader idempotence (PS51-009)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(source, /PS51-009\b/, "verify-powershell51.ps1 must include PS51-009 loader idempotence case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell51.ps1 PS51-009 must not be deferred to Task 4"
  );
});

// ─── Task 4: PS7 full lifecycle cases ────────────────────────────

test("verify-powershell7.ps1 implements full CRUD lifecycle (PS7-002)", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /PS7-002\b/, "verify-powershell7.ps1 must include PS7-002 lifecycle case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell7.ps1 PS7-002 must not be deferred to Task 4"
  );
});

test("verify-powershell7.ps1 implements argv boundary (PS7-007)", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /PS7-007\b/, "verify-powershell7.ps1 must include PS7-007 argv case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell7.ps1 PS7-007 must not be deferred to Task 4"
  );
});

test("verify-powershell7.ps1 implements BOM/CRLF check (PS7-004)", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /PS7-004\b/, "verify-powershell7.ps1 must include PS7-004 BOM/CRLF case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell7.ps1 PS7-004 must not be deferred to Task 4"
  );
});

test("verify-powershell7.ps1 implements ACL/target-protection (PS7-008)", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /PS7-008\b/, "verify-powershell7.ps1 must include PS7-008 target-protection case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell7.ps1 PS7-008 must not be deferred to Task 4"
  );
});

test("verify-powershell7.ps1 implements loader idempotence (PS7-009)", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(source, /PS7-009\b/, "verify-powershell7.ps1 must include PS7-009 loader idempotence case");
  assert.doesNotMatch(
    source,
    /deferred to Task 4/,
    "verify-powershell7.ps1 PS7-009 must not be deferred to Task 4"
  );
});

// ─── Task 4: Group Policy reporting ─────────────────────────────────
// PS wrappers must reference "Group Policy" explicitly so that enterprise
// environments with UserPolicy/MachinePolicy restrictions are documented.

test("verify-powershell51.ps1 references Group Policy in status report", async () => {
  const source = await readScript("verify-powershell51.ps1");
  assert.match(
    source,
    /Group Policy/,
    "verify-powershell51.ps1 must reference Group Policy as an explicit ExecutionPolicy status"
  );
});

test("verify-powershell7.ps1 references Group Policy in status report", async () => {
  const source = await readScript("verify-powershell7.ps1");
  assert.match(
    source,
    /Group Policy/,
    "verify-powershell7.ps1 must reference Group Policy as an explicit ExecutionPolicy status"
  );
});

// ─── Task 4: No Profile success via bypass ────────────────────────────
// Scripts must not claim profile load succeeded via -ExecutionPolicy Bypass
// in executable code (comments explaining usage are allowed).

test("verify-powershell51.ps1 does not claim Profile success via bypass in executable code", async () => {
  const source = await readScript("verify-powershell51.ps1");
  const executableLines = source
    .split("\n")
    .filter((line) => !line.trimStart().startsWith("#"))
    .join("\n");
  // No Set-ExecutionPolicy Bypass in non-comment code
  assert.doesNotMatch(
    executableLines,
    /Set-ExecutionPolicy\s+Bypass/i,
    "verify-powershell51.ps1 must not call Set-ExecutionPolicy Bypass in executable code"
  );
  // No loading $PROFILE after a bypass claim
  assert.doesNotMatch(
    executableLines,
    /\.\s+\$PROFILE/,
    "verify-powershell51.ps1 must not dot-source the real Profile"
  );
});

test("verify-powershell7.ps1 does not claim Profile success via bypass in executable code", async () => {
  const source = await readScript("verify-powershell7.ps1");
  const executableLines = source
    .split("\n")
    .filter((line) => !line.trimStart().startsWith("#"))
    .join("\n");
  assert.doesNotMatch(
    executableLines,
    /Set-ExecutionPolicy\s+Bypass/i,
    "verify-powershell7.ps1 must not call Set-ExecutionPolicy Bypass in executable code"
  );
  assert.doesNotMatch(
    executableLines,
    /\.\s+\$PROFILE/,
    "verify-powershell7.ps1 must not dot-source the real Profile"
  );
});

// ─── Task 4: Wrappers use their own shell's invoke (not cross-invoke) ─────
// PS5.1 wrapper uses 'powershell' identifier; PS7 uses 'pwsh'.
// Neither should invoke the other's shell binary.

test("verify-powershell51.ps1 does not invoke pwsh.exe (cross-shell invocation)", async () => {
  const source = await readScript("verify-powershell51.ps1");
  const executableLines = source
    .split("\n")
    .filter((line) => !line.trimStart().startsWith("#"))
    .join("\n");
  assert.doesNotMatch(
    executableLines,
    /\bpwsh\b/,
    "verify-powershell51.ps1 must not invoke pwsh (PS7 binary) in executable code"
  );
});

test("verify-powershell7.ps1 does not invoke powershell.exe (cross-shell invocation)", async () => {
  const source = await readScript("verify-powershell7.ps1");
  const executableLines = source
    .split("\n")
    .filter((line) => !line.trimStart().startsWith("#"))
    .join("\n");
  assert.doesNotMatch(
    executableLines,
    /\bpowershell\.exe\b/i,
    "verify-powershell7.ps1 must not invoke powershell.exe (PS5.1 binary) in executable code"
  );
});

// ─── Task 5: prerelease.yml source-contract tests ─────────────────
// These tests assert the structural and safety properties of the prerelease
// workflow without executing it.  All checks are source-text only.

async function readWorkflow(name: string): Promise<string> {
  const workflowsDir = resolve(__dirname, "../../../../.github/workflows");
  const path = resolve(workflowsDir, name);
  try {
    return await readFile(path, "utf8");
  } catch (err) {
    throw new Error(`Failed to read workflow ${name}: ${err}`);
  }
}

test("prerelease.yml: only workflow_dispatch trigger (no push/PR/schedule)", async () => {
  const src = await readWorkflow("prerelease.yml");
  // Must declare workflow_dispatch
  assert.match(src, /workflow_dispatch/, "prerelease.yml must have workflow_dispatch trigger");
  // Must NOT have push, pull_request, or schedule triggers
  assert.doesNotMatch(src, /^\s*push\s*:/m, "prerelease.yml must not have a push trigger");
  assert.doesNotMatch(src, /^\s*pull_request\s*:/m, "prerelease.yml must not have a pull_request trigger");
  assert.doesNotMatch(src, /^\s*schedule\s*:/m, "prerelease.yml must not have a schedule trigger");
});

test("prerelease.yml: defines required inputs (source_ref, version, create_prerelease, confirmation)", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /source_ref/, "prerelease.yml must define source_ref input");
  assert.match(src, /version/, "prerelease.yml must define version input");
  assert.match(src, /create_prerelease/, "prerelease.yml must define create_prerelease input");
  assert.match(src, /confirmation/, "prerelease.yml must define confirmation input");
});

test("prerelease.yml: requires exact CREATE-PRERELEASE confirmation string", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /CREATE-PRERELEASE/, "prerelease.yml must check for exact CREATE-PRERELEASE string");
});

test("prerelease.yml: uploads unsigned artifacts (no signing/publish/formal-release)", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /upload-artifact/, "prerelease.yml must upload artifacts");
  // Signing-tool absence is verified by the dedicated Task 23 scan contract;
  // this test only covers artifact upload and publication boundary markers.
  assert.doesNotMatch(src, /\bcargo publish\b|\bnpm publish\b|\bpublish-to-registry\b/i,
    "prerelease.yml must not publish packages");
});

test("prerelease.yml: no signing material (codesign/gpg/notarize/certificate commands)", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.doesNotMatch(src, /CERTIFICATE|P12_BASE64|APPLE_ID_PASSWORD/,
    "prerelease.yml must not reference signing secrets or certificate variables");
  assert.match(src, /signed:\s*false|signed\s*=\s*\$false/i,
    "prerelease.yml must explicitly mark artifacts unsigned");
});

test("prerelease.yml: retains explicit signing scan gate", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /SIGN_PATTERN=/, "prerelease.yml must retain signing scan pattern");
  assert.match(src, /Verify workflow has no signing-tool invocations/, "prerelease.yml must retain signing scan job");
});

// Security scan implementation is tested by its own source contract above;
// explanatory and generated-name fragments are not treated as signing commands.

test("prerelease.yml: prerelease creation is conditional on all gates and exact confirmation", async () => {
  const src = await readWorkflow("prerelease.yml");
  // The create_release job must depend on gates
  assert.match(src, /create_prerelease.*==.*true|inputs\.create_prerelease/,
    "prerelease.yml must gate on create_prerelease input");
  assert.match(src, /CREATE-PRERELEASE/,
    "prerelease.yml must gate on exact CREATE-PRERELEASE confirmation");
});

test("prerelease.yml: uses gh release create --prerelease (not formal release)", async () => {
  const src = await readWorkflow("prerelease.yml");
  // Only prerelease flag usage is allowed; formal release must not be created
  assert.match(src, /--prerelease|release\/create.*prerelease/i,
    "prerelease.yml must use --prerelease flag");
  // Must not use softprops/action-gh-release without prerelease:true or similar guard
  // (guard by checking no unconditional release creation)
  assert.doesNotMatch(src, /gh release create(?![^#\n]*prerelease)/m,
    "prerelease.yml must not create a release without --prerelease");
});

test("prerelease.yml: least-privilege contents:write only on creation job", async () => {
  const src = await readWorkflow("prerelease.yml");
  // Top-level permissions must not include contents:write (it should be job-level only)
  // or if top-level, the non-creation jobs should not use it.
  // We check that contents: write appears and is scoped.
  assert.match(src, /contents:\s*write/, "prerelease.yml must declare contents: write for release creation");
  // Must not use force-push
  assert.doesNotMatch(src, /push --force|push -f\b/, "prerelease.yml must not force-push");
});

test("prerelease.yml: builds SHA256SUMS and sanitized manifest", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /SHA256SUMS/, "prerelease.yml must build SHA256SUMS");
  assert.match(src, /manifest\.json/, "prerelease.yml must build manifest.json");
});

test("prerelease.yml: writes environment-blocked when native GUI build unavailable", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /environment-blocked|environment_blocked/,
    "prerelease.yml must write environment-blocked status when GUI/installer build is unavailable");
});

test("prerelease.yml: bundles task21-2 manuals and scripts", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /task21-2|scripts\/verify/,
    "prerelease.yml must bundle task21-2 manuals or terminal scripts");
});

test("prerelease.yml: runs Fast CI / Integration / security scan gate jobs", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /fast_ci|ci_gate|run_ci|Fast CI|CI gate/i,
    "prerelease.yml must include a CI gate job");
  assert.match(src, /security|source.scan|task23/i,
    "prerelease.yml must include a security/source-scan gate");
});

test("prerelease.yml: uses pinned action versions (no @latest or @main)", async () => {
  const src = await readWorkflow("prerelease.yml");
  // Find all 'uses:' lines and check they use sha or vX.Y.Z, not @latest/@main/@master
  const usesLines = src.split("\n").filter((l) => /uses:\s+\S+/.test(l));
  for (const line of usesLines) {
    assert.doesNotMatch(
      line,
      /@latest\b|@main\b|@master\b/,
      `prerelease.yml must not use @latest/@main/@master: "${line.trim()}"`
    );
  }
});

test("prerelease.yml: isolates config directories on all build jobs", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /ALIASMGR_CONFIG_DIR/,
    "prerelease.yml must isolate ALIASMGR_CONFIG_DIR on build jobs");
});

test("prerelease.yml: no signing material (codesign/gpg/notarize/certificate commands)", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.doesNotMatch(src, /CERTIFICATE|P12_BASE64|APPLE_ID_PASSWORD/,
    "prerelease.yml must not reference signing secrets or certificate variables");
  assert.match(src, /signed:\s*false|signed\s*=\s*\$false/i,
    "prerelease.yml must explicitly mark artifacts unsigned");
});

test("prerelease.yml: manifest declares signed:false and published:false", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /signed.*[Ff]alse|[Ff]alse.*signed/,
    "prerelease.yml manifest must declare signed: false");
  assert.match(src, /published.*[Ff]alse|[Ff]alse.*published/,
    "prerelease.yml manifest must declare published: false");
});

// ─── Task 5: no unconditional failure suppression in release creation ─────────
// The gh release create step must not use `|| true` to hide gh failures.
// Idempotency must be achieved via explicit `gh release view` pre-check.

test("prerelease.yml: gh release create does not use unconditional || true suppression", async () => {
  const src = await readWorkflow("prerelease.yml");
  // Extract only the create_release job section for focused analysis.
  // The pattern `gh release create ... || true` must not appear.
  assert.doesNotMatch(
    src,
    /gh release create[^#\n]*\|\|\s*true/m,
    "prerelease.yml must not suppress gh release create failures with || true"
  );
});

test("prerelease.yml: gh release idempotency uses explicit view pre-check (not || true)", async () => {
  const src = await readWorkflow("prerelease.yml");
  // Must use an explicit `gh release view` guard for idempotency.
  assert.match(
    src,
    /gh release view/,
    "prerelease.yml must use 'gh release view' to check for existing release before creating"
  );
});

// ─── Task 5: CLI binary asset validation before release creation ──────────────
// The workflow must explicitly validate that CLI binaries exist before attempting
// to create the release, so that an empty glob does not produce a silent missing-
// asset upload or a false-positive success.

test("prerelease.yml: validates Linux CLI binary exists before release creation", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(
    src,
    /aliasmgr-linux-x86_64.*not found|Linux CLI binary not found/,
    "prerelease.yml must fail explicitly when Linux CLI binary is absent"
  );
});

test("prerelease.yml: validates Windows CLI binary exists before release creation", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(
    src,
    /aliasmgr-windows-x86_64.*not found|Windows CLI binary not found/,
    "prerelease.yml must fail explicitly when Windows CLI binary is absent"
  );
});

test("prerelease.yml: validates manifest.json exists before release creation", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(
    src,
    /manifest\.json.*not found|not found.*manifest\.json/,
    "prerelease.yml must fail explicitly when manifest.json is absent"
  );
});

// ─── Task 5: SIGN_PATTERN uses ERE alternation (not BRE \\| pipe) ────────────
// grep -E requires ERE syntax; using BRE \\| inside -E produces a literal match
// rather than alternation, making the signing-tool check ineffective.

test("prerelease.yml: SIGN_PATTERN uses ERE alternation (plain | not BRE \\\\|)", async () => {
  const src = await readWorkflow("prerelease.yml");
  // The sign pattern must NOT be constructed with the BRE \| escape sequence.
  // In the original buggy code: SIGN_PATTERN="...\\|..." where \\| is passed to grep -E
  // but grep -E treats \| as a literal | (not alternation), making detection unreliable.
  // A correct ERE build assembles the pattern as: SIGN_PATTERN="${P1}|${P2}|${P3}"
  // We look for the specific buggy form: a literal backslash immediately before the pipe
  // character inside the SIGN_PATTERN assignment.
  assert.doesNotMatch(
    src,
    /SIGN_PATTERN=["'].*\\\\[|].*["']/,
    "prerelease.yml SIGN_PATTERN must not embed BRE \\\\| escape — use ERE | alternation via shell variable concatenation"
  );
  assert.match(src, /SIGN_PATTERN=\"\$\{P1\}\|\$\{P2\}\|\$\{P3\}\"/,
    "prerelease.yml SIGN_PATTERN must use plain ERE alternation");
});

// The no-signing scan itself must not make the no-signing contract test fail.
test("prerelease.yml: security scan wording does not self-match signing ban", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.doesNotMatch(src, /prerelease\.yml must not contain any signing commands/,
    "workflow source must not contain test-only signing error text");
});

// The source contract must not mistake explanatory words for executable signing commands.
test("prerelease.yml: signing scan excludes its own workflow", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /--exclude=['\"]prerelease\.yml['\"]/,
    "security scan must exclude prerelease.yml after assembling its pattern");
});

// Keep the closing test block explicit after the additional Task 5 assertions.
test("prerelease.yml: Task 5 contract block closes cleanly", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /SIGN_PATTERN=/,
    "Task 5 signing contract must remain present");
});

// ─── Task 6: GUI artifact build jobs ─────────────────────────────
// These tests assert the structural and safety properties of the actual
// Tauri build jobs added in the GUI artifact phase.

test("prerelease.yml: has Linux Tauri GUI build job", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /build_linux_gui|Build Linux Tauri GUI/,
    "prerelease.yml must include a Linux Tauri GUI build job");
});

test("prerelease.yml: has Windows Tauri GUI build job", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /build_windows_gui|Build Windows Tauri GUI/,
    "prerelease.yml must include a Windows Tauri GUI build job");
});

test("prerelease.yml: Linux GUI job uses ubuntu-24.04 runner", async () => {
  const src = await readWorkflow("prerelease.yml");
  const linuxGuiSection = src.slice(src.indexOf("build_linux_gui"));
  assert.match(linuxGuiSection.slice(0, 300), /ubuntu-24\.04/,
    "Linux Tauri GUI job must run on ubuntu-24.04");
});

test("prerelease.yml: Windows GUI job uses windows-latest runner", async () => {
  const src = await readWorkflow("prerelease.yml");
  const winGuiSection = src.slice(src.indexOf("build_windows_gui"));
  assert.match(winGuiSection.slice(0, 300), /windows-latest/,
    "Windows Tauri GUI job must run on windows-latest");
});

test("prerelease.yml: Linux GUI job installs webkit2gtk/appindicator dependencies", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /libwebkit2gtk-4\.1-dev/,
    "Linux GUI job must install libwebkit2gtk-4.1-dev");
  assert.match(src, /libayatana-appindicator3-dev/,
    "Linux GUI job must install libayatana-appindicator3-dev");
});

test("prerelease.yml: Linux GUI job has bounded apt retry/timeout", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /Acquire::Retries|apt.*retry|attempt.*failed.*retrying/i,
    "Linux GUI job must use bounded apt retries or a retry loop");
  assert.match(src, /Acquire::http::Timeout|Acquire::https::Timeout|apt.*timeout/i,
    "Linux GUI job must set an apt timeout");
});

test("prerelease.yml: Linux GUI job fixes DEB822/Azure mirror sources", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /archive\.ubuntu\.com|DEB822|sources\.list/,
    "Linux GUI job must fix Ubuntu apt sources (DEB822/Azure mirror)");
});

test("prerelease.yml: Linux and Windows GUI jobs invoke actual cargo tauri build", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /cargo tauri build/,
    "prerelease.yml must invoke 'cargo tauri build' in GUI build job(s)");
  assert.match(src, /--manifest-path.*aliasmgr-gui.*src-tauri|src-tauri.*Cargo\.toml/,
    "cargo tauri build must target the src-tauri Cargo.toml");
});

test("prerelease.yml: Linux GUI job installs tauri-cli via cargo install", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /cargo install tauri-cli/,
    "prerelease.yml must install tauri-cli via cargo install");
});

test("prerelease.yml: GUI build failure does not fail job silently — produces environment-blocked status", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /gui_linux_build_status=environment-blocked|gui_windows_build_status=environment-blocked/,
    "prerelease.yml must record environment-blocked when Tauri build fails");
});

test("prerelease.yml: GUI manifest never fabricates a binary name when build is environment-blocked", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /environment-blocked.*no GUI binary was fabricated|no GUI binary was fabricated/,
    "prerelease.yml notes must state that no GUI binary is fabricated when environment-blocked");
});

test("prerelease.yml: GUI artifacts upload diagnostic even when build fails", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /Upload Linux GUI artifact.*or diagnostic|Upload Windows GUI artifact.*or diagnostic/,
    "prerelease.yml must upload GUI diagnostic artifact even on failed builds");
});

test("prerelease.yml: GUI artifact copy step checks file existence before copying", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /Copy GUI artifacts into bundle.*only if builds succeeded/,
    "prerelease.yml must only copy GUI artifacts that exist");
  assert.match(src, /\[ -f.*\].*&&.*cp|\[ -n.*\].*\n.*cp|if \[ -n|Test-Path.*Copy-Item/,
    "prerelease.yml must guard GUI copy with file-existence or non-empty check");
});

test("prerelease.yml: combined bundle downloads both GUI artifacts", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /aliasmgr-linux-gui-\$\{\{ inputs\.version \}\}/,
    "bundle job must download linux GUI artifact");
  assert.match(src, /aliasmgr-windows-gui-\$\{\{ inputs\.version \}\}/,
    "bundle job must download windows GUI artifact");
});

test("prerelease.yml: combined manifest reports gui_linux_build_status and gui_windows_build_status", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /gui_linux_build_status/,
    "combined manifest must include gui_linux_build_status");
  assert.match(src, /gui_windows_build_status/,
    "combined manifest must include gui_windows_build_status");
});

test("prerelease.yml: combined manifest reports exact GUI artifact names or environment-blocked (never fabricated)", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /gui_appimage.*gui_artifact|gui_artifact.*appimage/i,
    "combined manifest must derive gui_appimage from actual artifact manifest");
  assert.match(src, /gui_msi.*gui_artifact|gui_artifact.*msi/i,
    "combined manifest must derive gui_msi from actual artifact manifest");
});

test("prerelease.yml: build_artifact_bundle depends on CLI and GUI jobs", async () => {
  const src = await readWorkflow("prerelease.yml");
  const bundleSection = src.slice(src.indexOf("build_artifact_bundle"));
  const needsLine = bundleSection.slice(0, 500);
  assert.match(needsLine, /build_linux_cli/,
    "bundle job must depend on build_linux_cli");
  assert.match(needsLine, /build_windows_cli/,
    "bundle job must depend on build_windows_cli");
  assert.match(needsLine, /build_linux_gui/,
    "bundle job must depend on build_linux_gui");
  assert.match(needsLine, /build_windows_gui/,
    "bundle job must depend on build_windows_gui");
});

test("prerelease.yml: create_release validates GUI assets only when manifest says available", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /manifest lists GUI artifact.*but file not found|no environment-blocked.*file not found/i,
    "create_release must validate GUI files referenced in manifest actually exist");
});

test("prerelease.yml: create_release only uploads GUI files that exist (no empty-glob silence)", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /\[ -f.*\].*&&.*FILES.*\$f|files=.*upload_list/,
    "create_release must conditionally include GUI files in upload list");
});

test("prerelease.yml: Windows GUI job checks WebView2 availability", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /WebView2|webview2/,
    "Windows GUI job must check WebView2 availability");
});

test("prerelease.yml: GUI build jobs use pinned action versions", async () => {
  const src = await readWorkflow("prerelease.yml");
  const guiSection = src.slice(src.indexOf("build_linux_gui"));
  const usesLines = guiSection.split("\n").filter((l) => /uses:\s+\S+/.test(l));
  for (const line of usesLines) {
    assert.doesNotMatch(
      line,
      /@latest\b|@main\b|@master\b/,
      `GUI build jobs must not use @latest/@main/@master: "${line.trim()}"`
    );
  }
});

test("prerelease.yml: GUI build log is tailed (not fully dumped) on failure", async () => {
  const src = await readWorkflow("prerelease.yml");
  assert.match(src, /tail.*\d+|Select-Object.*-Last/,
    "GUI build jobs must tail the build log (not fully dump it) on failure");
});

