import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import type { ShellStatus } from "./lib.ts";
import {
  autoSyncNotice,
  defaultWizardSelection,
  normalizeAliasPreflight,
  normalizeAutoSync,
  normalizeDeleteAliasResult,
  normalizeSaveAliasResult,
  normalizeShellSetupOutcome,
  normalizeShellSetupPlan,
  normalizeShellStartupState,
  normalizeShellStatuses,
  normalizeSyncRun,
  preflightMessage,
  preflightProblemText,
  setupErrorMessage,
  setupOutcomeFailed,
  shellEnableArgs,
  shellLoaderLabel,
  shellOptionState,
  shellOptionStateLabel,
  shellSetupArgs,
  shellSetupOutcomeLines,
  shellSourceLabel,
  wizardReadiness,
} from "./lib.ts";
import { t, translationIssues, translationKeys } from "./i18n.ts";

function status(overrides: Partial<ShellStatus> = {}): ShellStatus {
  return {
    shell: "bash",
    detected: true,
    executable: "/bin/bash",
    source: "path",
    version: "5.2",
    wsl: false,
    enabled: false,
    loader_status: "missing",
    loader_target: "/home/me/.bashrc",
    loader_error: null,
    loader_error_code: null,
    action: "install_loader",
    ...overrides,
  };
}

function syncRun(results: Array<Record<string, unknown>> = [], revision = 3) {
  return normalizeSyncRun({
    revision,
    failed: results.some((result) => result.status === "failed"),
    results,
  });
}

test("shell status normalization keeps the four supported shells in canonical order", () => {
  const rows = normalizeShellStatuses([
    { shell: "powershell7", detected: true, enabled: true, loader_status: "installed", action: "enable" },
    { shell: "zsh", detected: false, loader_status: "unavailable" },
    { shell: "bash", detected: true, loader_status: "missing" },
    { shell: "fish", detected: true },
  ]);
  assert.deepEqual(rows.map((row) => row.shell), ["bash", "zsh", "powershell7"]);
  assert.equal(normalizeShellStatuses(null).length, 0);
  assert.equal(normalizeShellStatuses([null, 5, {}]).length, 0);
  const bash = rows[0];
  assert.equal(bash.detected, true);
  assert.equal(bash.loader_target, null);
  assert.equal(bash.source, "not_found");
  assert.equal(bash.action, "needs_path");
});

test("startup state derives invalid and new shells from the rows when flags are absent", () => {
  const state = normalizeShellStartupState({
    first_run: false,
    auto_sync: false,
    shells: [
      { shell: "bash", detected: true, enabled: true, loader_status: "missing" },
      { shell: "zsh", detected: true, enabled: false, loader_status: "missing" },
      { shell: "powershell7", detected: true, enabled: false, wsl: false, loader_status: "missing" },
      { shell: "powershell5", detected: false, enabled: false, loader_status: "missing" },
    ],
  });
  assert.equal(state.firstRun, false);
  assert.equal(state.autoSync, false);
  assert.deepEqual(state.invalidEnabled, ["bash"]);
  assert.deepEqual(state.newlyDetected, ["zsh", "powershell7"]);
  const defaults = normalizeShellStartupState(null);
  assert.equal(defaults.firstRun, false);
  assert.equal(defaults.autoSync, true);
  assert.deepEqual(defaults.shells, []);
});

test("shell option states distinguish enabled, invalid, disabled, missing and WSL", () => {
  assert.equal(shellOptionState(status({ enabled: true, loader_status: "installed" })), "enabled");
  assert.equal(shellOptionState(status({ enabled: true, loader_status: "missing" })), "loader-invalid");
  assert.equal(shellOptionState(status({ enabled: false })), "disabled");
  assert.equal(shellOptionState(status({ detected: false })), "not-installed");
  assert.equal(shellOptionState(status({ wsl: true })), "wsl");

  assert.equal(shellOptionStateLabel("enabled", "en"), "enabled");
  assert.equal(shellOptionStateLabel("not-installed", "zh"), "未安装");
  assert.equal(shellLoaderLabel("installed", "en"), "loader installed");
  assert.equal(shellLoaderLabel("unavailable", "zh"), "无 profile 目标");
  assert.equal(shellLoaderLabel("mystery", "en"), "mystery");
  assert.equal(shellSourceLabel("known_path", "en"), "known install path");
  assert.equal(shellSourceLabel("manual", "zh"), "手动路径");
  assert.equal(shellSourceLabel("mystery", "zh"), "mystery");
});

test("default wizard selection never enables a merely detected shell", () => {
  const rows = [
    status({ shell: "bash", enabled: true }),
    status({ shell: "zsh", loader_status: "installed" }),
    status({ shell: "powershell5", detected: true }),
    status({ shell: "powershell7", wsl: true, loader_status: "installed" }),
  ];
  assert.deepEqual(defaultWizardSelection(rows), ["bash", "zsh"]);
  assert.deepEqual(defaultWizardSelection([]), []);
});

test("wizard readiness blocks WSL and shells that still need a path", () => {
  const rows = [
    status({ shell: "bash", action: "install_loader" }),
    status({ shell: "zsh", detected: false, executable: null, loader_target: null, action: "needs_path" }),
    status({ shell: "powershell7", wsl: true, action: "wsl_manual" }),
  ];
  assert.deepEqual(wizardReadiness(rows, [], {}), {
    ready: false,
    blocked: [],
    needsPath: [],
  });
  const missingPath = wizardReadiness(rows, ["bash", "zsh"], {});
  assert.equal(missingPath.ready, false);
  assert.deepEqual(missingPath.needsPath, ["zsh"]);
  assert.deepEqual(
    wizardReadiness(rows, ["bash", "zsh"], { zsh: { executable: "/opt/zsh" } }).needsPath,
    ["zsh"],
    "a profile target is still required",
  );
  assert.equal(
    wizardReadiness(rows, ["bash", "zsh"], { zsh: { executable: "/opt/zsh", profile: " ~/.zshrc " } }).ready,
    true,
  );
  const wsl = wizardReadiness(rows, ["bash", "powershell7"], {});
  assert.equal(wsl.ready, false);
  assert.deepEqual(wsl.blocked, ["powershell7"]);
  assert.equal(
    wizardReadiness(rows, ["bash", "powershell7"], { powershell7: { executable: "C:\\Program Files\\PowerShell\\7\\pwsh.exe" } }).ready,
    true,
    "a manual native executable unblocks a WSL row",
  );
});

test("shell setup argument envelopes trim optional paths and omit blanks", () => {
  assert.deepEqual(shellSetupArgs("bash"), { shell: "bash" });
  assert.deepEqual(shellSetupArgs("bash", { executable: "  ", profile: " \t" }), { shell: "bash" });
  assert.deepEqual(
    shellSetupArgs("zsh", { executable: " C:\\Program Files\\Git\\bin\\bash.exe ", profile: " ~/.zshrc " }),
    { shell: "zsh", executable: "C:\\Program Files\\Git\\bin\\bash.exe", profile: "~/.zshrc" },
  );
  assert.deepEqual(shellEnableArgs([" bash ", "bash", "", "zsh"]), { shells: ["bash", "zsh"] });
});

test("setup errors map stable prefixes to localized copy in both languages", () => {
  assert.match(setupErrorMessage("setup_not_ready: bash executable was not found", "en"), /not available/);
  assert.match(setupErrorMessage("setup_config: must be absolute", "en"), /absolute paths/);
  assert.match(setupErrorMessage("setup_not_ready: WSL", "zh"), /找不到/);
  assert.match(setupErrorMessage("setup_unsupported", "zh"), /无法集成/);
  assert.match(setupErrorMessage("setup_permission", "en"), /Permission denied/);
  assert.match(setupErrorMessage("setup_missing", "en"), /does not exist/);
  assert.match(setupErrorMessage("setup_io", "en"), /I\/O error/);
  assert.equal(setupErrorMessage("something else", "en"), "something else");
});

test("setup plans and outcomes normalize per-step results", () => {
  const plan = normalizeShellSetupPlan({
    shell: "bash",
    detected: true,
    executable: "/bin/bash",
    source: "known_path",
    version: null,
    wsl: false,
    enabled: false,
    action: "install_loader",
    loader_status: "missing",
    loader_target: "/home/me/.bashrc",
    loader_error: null,
    preview_target: "/home/me/.bashrc",
    loader_block: "# >>> aliasmgr >>>",
    default_profile: "/home/me/.bashrc",
  });
  assert.equal(plan.loaderStatus, "missing");
  assert.equal(plan.previewTarget, "/home/me/.bashrc");
  assert.equal(normalizeShellSetupPlan(null).action, "needs_path");

  const outcome = normalizeShellSetupOutcome({
    shell: "bash",
    enabled: true,
    executable: "/bin/bash",
    loader_target: "/home/me/.bashrc",
    loader_changed: true,
    loader_installed: true,
    loader_error: null,
    loader_error_code: null,
    sync: { revision: 4, failed: false, results: [] },
    sync_error: null,
    warnings: [],
    reload_command: "source '/root/generated/bash.sh'",
  });
  assert.equal(outcome.loaderChanged, true);
  assert.equal(outcome.reloadCommand, "source '/root/generated/bash.sh'");
  assert.equal(setupOutcomeFailed(outcome), false);
  assert.ok(shellSetupOutcomeLines(outcome, "en").some((line) => /Loader installed/.test(line)));
  assert.ok(shellSetupOutcomeLines(outcome, "en").some((line) => /Reload this terminal/.test(line)));

  const failed = normalizeShellSetupOutcome({
    shell: "bash",
    loader_error: "malformed markers",
    loader_error_code: "loader_unsafe",
    sync_error: null,
  });
  assert.equal(setupOutcomeFailed(failed), true);
  assert.ok(shellSetupOutcomeLines(failed, "zh").some((line) => /加载块/.test(line)));
});

test("preflight normalization and messages stay localized and code-driven", () => {
  const preflight = normalizeAliasPreflight({
    ok: false,
    problems: [
      { shell: "bash", code: "shell_disabled", message: "bash integration is not enabled" },
      { shell: "powershell7", code: "loader_missing", message: "missing" },
      { shell: "zsh", code: "loader_unsafe", message: "unsafe", loader_target: "/home/me/.zshrc" },
    ],
  });
  assert.equal(preflight.ok, false);
  assert.equal(preflight.problems.length, 3);
  assert.match(preflightProblemText(preflight.problems[0], "en"), /Bash is not enabled/);
  assert.match(preflightProblemText(preflight.problems[1], "zh"), /PowerShell 7 尚未安装加载块/);
  assert.match(preflightProblemText(preflight.problems[2], "en"), /integration is invalid/);
  assert.match(preflightMessage(preflight, "en"), /would not load/);
  assert.equal(normalizeAliasPreflight({ ok: true }).ok, true);
  assert.equal(normalizeAliasPreflight({ ok: false, problems: [] }).ok, false);
});

test("auto-sync notices surface success, partial failure and orchestration errors", () => {
  const skipped = autoSyncNotice(normalizeAutoSync({ attempted: false, skipped_reason: "disabled" }), "en");
  assert.equal(skipped.tone, "none");
  assert.equal(skipped.text, "");

  const ok = autoSyncNotice(normalizeAutoSync({
    attempted: true,
    run: {
      revision: 7,
      failed: false,
      results: [
        { shell: "bash", status: "ok", reload_command: "source /root/generated/bash.sh" },
        { shell: "powershell7", status: "ok", reload_command: ". 'C:\\root\\generated\\powershell7.ps1'" },
      ],
    },
  }), "en");
  assert.equal(ok.tone, "ok");
  assert.match(ok.text, /2 shell/);
  assert.match(ok.text, /revision 7/);
  assert.equal(ok.reloadCommands.length, 2);

  const partial = autoSyncNotice(normalizeAutoSync({
    attempted: true,
    run: {
      revision: 8,
      failed: true,
      results: [
        { shell: "bash", status: "ok", reload_command: "source /bash.sh" },
        { shell: "zsh", status: "failed", error_code: "sync_permission", reload_command: "source /zsh.sh" },
      ],
    },
  }), "en");
  assert.equal(partial.tone, "warning");
  assert.match(partial.text, /Zsh/);
  assert.match(partial.text, /permission/i);
  assert.doesNotMatch(partial.text, /^Saved and synced/);

  const error = autoSyncNotice(normalizeAutoSync({
    attempted: true,
    error: "database is locked",
    error_code: "sync_locked",
  }), "zh");
  assert.equal(error.tone, "warning");
  assert.match(error.text, /数据库/);
  assert.match(error.text, /database is locked/);
});

test("save and delete results keep the draft when preflight blocks", () => {
  const blocked = normalizeSaveAliasResult({
    saved: false,
    alias: null,
    preflight: { ok: false, problems: [{ shell: "bash", code: "shell_disabled", message: "no" }] },
    auto_sync: { attempted: false, skipped_reason: "preflight" },
  });
  assert.equal(blocked.saved, false);
  assert.equal(blocked.alias, null);
  assert.equal(blocked.preflight.ok, false);
  assert.equal(blocked.autoSync.skippedReason, "preflight");

  const saved = normalizeSaveAliasResult({
    saved: true,
    alias: { name: "gs" },
    preflight: { ok: true, problems: [] },
    auto_sync: { attempted: true, run: { revision: 1, failed: false, results: [] } },
  });
  assert.equal(saved.saved, true);
  assert.equal(saved.alias?.name, "gs");
  assert.equal(saved.autoSync.attempted, true);

  const deleted = normalizeDeleteAliasResult({ deleted: true, auto_sync: { attempted: true } });
  assert.equal(deleted.deleted, true);
  assert.equal(deleted.autoSync.attempted, true);
  assert.equal(normalizeDeleteAliasResult(null).deleted, false);
});

test("sync run counts and failure flags stay consistent with normalized rows", () => {
  const run = syncRun([
    { shell: "bash", status: "ok", reload_command: "source /bash.sh" },
    { shell: "zsh", status: "failed", error_code: "sync_failed" },
  ], 9);
  assert.equal(run.failed, true);
  assert.equal(run.results.length, 2);
  assert.equal(run.revision, 9);
  assert.equal(run.results.filter((result) => result.status === "failed").length, 1);
});

test("every shell setup translation exists in both dictionaries and is translated", () => {
  assert.deepEqual(translationIssues(), []);
  const keys = translationKeys().filter((key) => (
    key.startsWith("shellSetup.")
    || key.startsWith("preflight.")
    || key.startsWith("autoSync.")
    || key === "settings.autoSync"
    || key === "common.close"
  ));
  assert.ok(keys.length >= 80, `expected the full shell setup copy, got ${keys.length}`);
  for (const key of keys) {
    assert.ok(t("en", key).trim().length > 0, `en:${key} is empty`);
    assert.ok(t("zh", key).trim().length > 0, `zh:${key} is empty`);
    assert.notEqual(t("en", key), t("zh", key), `${key} is not translated`);
  }
  assert.match(t("zh", "autoSync.partial"), /同步失败/);
  assert.match(t("en", "preflight.shellDisabled", { shell: "Bash" }), /Bash is not enabled/);
});

// ── component and command source contracts ────────────────────────

async function component(name: string): Promise<string> {
  return readFile(new URL(`./components/${name}.tsx`, import.meta.url), "utf8");
}

test("first-run wizard exposes selection, preview, apply and skip", async () => {
  const wizard = await component("ShellSetupWizard");
  for (const fragment of [
    "defaultWizardSelection",
    "wizardReadiness",
    "shell_setup_plan",
    "shell_setup_apply",
    "shell_setup_skip",
    "shellSetup.preview",
    "shellSetup.apply",
    "shellSetup.skip",
    "shellSetup.wslHint",
    "shellSetup.notDetectedHint",
    "normalizeShellSetupPlan",
    "normalizeShellSetupOutcome",
    "shellSetupArgs",
    'role="alert"',
  ]) {
    assert.ok(wizard.includes(fragment), `wizard is missing ${fragment}`);
  }
  assert.equal(/eval\(|new Function|child_process/.test(wizard), false);
});

test("startup banner offers re-enable, session ignore and persistent disable", async () => {
  const banner = await component("ShellStartupBanner");
  for (const fragment of [
    "shell_setup_apply",
    "shell_set_enabled",
    "shellSetup.reenable",
    "shellSetup.ignore",
    "shellSetup.disableShell",
    "shellSetup.notConfiguredTitle",
    "shellSetup.newDetectedTitle",
    "shellSetup.openSetup",
    "onIgnore",
    "onOpenSetup",
  ]) {
    assert.ok(banner.includes(fragment), `banner is missing ${fragment}`);
  }
  assert.equal(/eval\(|new Function|child_process/.test(banner), false);
});

test("settings shell panel reuses status, enable and setup commands with path overrides", async () => {
  const panel = await component("ShellSettingsPanel");
  for (const fragment of [
    "shell_status",
    "shell_set_enabled",
    "shell_setup_plan",
    "shell_setup_apply",
    "shellSetup.rescan",
    "shellSetup.executable",
    "shellSetup.profile",
    "shellSetup.loaderTarget",
    "shellSetup.disable",
    "shellSetup.enable",
    "focusToken",
    "normalizeShellStatuses",
  ]) {
    assert.ok(panel.includes(fragment), `settings panel is missing ${fragment}`);
  }
  assert.equal(/eval\(|new Function|child_process/.test(panel), false);
});

test("wizard labels shell options and blocks saves through the shared preflight", async () => {
  const wizard = await component("AliasWizard");
  for (const fragment of [
    "shell_status",
    "normalizeShellStatuses",
    "shellOptionStateLabel",
    "shellOptionState",
    "shellLoaderLabel",
    "preflightMessage",
    "preflight.openSetup",
    "onOpenShellSetup",
    "preflight.draftKept",
  ]) {
    assert.ok(wizard.includes(fragment), `alias wizard is missing ${fragment}`);
  }
  assert.match(wizard, /onSave\(finalized\)/);
});

test("App wires startup state, the first-run wizard, banner and auto-sync notices", async () => {
  const app = await readFile(new URL("./App.tsx", import.meta.url), "utf8");
  for (const fragment of [
    "shell_startup_state",
    "normalizeShellStartupState",
    "ShellSetupWizard",
    "ShellStartupBanner",
    "autoSyncNotice",
    "normalizeSaveAliasResult",
    "normalizeDeleteAliasResult",
    "onOpenShellSetup",
    "focusShellsToken",
    "ignoreShellWarning",
    'data-testid="auto-sync-notice"',
  ]) {
    assert.ok(app.includes(fragment), `App is missing ${fragment}`);
  }
});

test("Tauri registers the shell setup, startup and preflight commands", async () => {
  const main = await readFile(new URL("../../src-tauri/src/main.rs", import.meta.url), "utf8");
  for (const command of [
    "shell_status",
    "shell_startup_state",
    "shell_set_enabled",
    "shell_setup_plan",
    "shell_setup_apply",
    "shell_setup_skip",
    "preflight_alias_shells",
  ]) {
    assert.match(main, new RegExp(`commands::${command}`), `${command} is not registered`);
  }
});

test("shell setup surfaces never read raw profile text or execute commands", async () => {
  const sources = [
    await component("ShellSetupWizard"),
    await component("ShellStartupBanner"),
    await component("ShellSettingsPanel"),
    await component("AliasWizard"),
    await readFile(new URL("./App.tsx", import.meta.url), "utf8"),
    await readFile(new URL("./components/ImportPanel.tsx", import.meta.url), "utf8"),
  ].join("\n");
  for (const forbidden of [
    "original_definition",
    "recovery_text",
    "dangerouslySetInnerHTML",
    "child_process",
    "eval(",
    "new Function",
    "Invoke-Expression",
    "password",
    "api_key",
  ]) {
    assert.equal(sources.includes(forbidden), false, `unexpected ${forbidden} in shell setup surfaces`);
  }
});
