import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  FIXED_ARGS_PLACEHOLDER_HINT,
  MANUAL_ONLY_BOUNDARY,
  SHELLS,
  TARGET_TYPES,
  emptyAliasDraft,
  environmentToRows,
  finalizeAliasDraft,
  importContentArgs,
  importReportCounts,
  isImportableFilename,
  normalizeDoctorFindings,
  normalizeFixedArgs,
  normalizeImportReport,
  normalizeOverriddenDefinitions,
  normalizePreviewSnippets,
  normalizeSettings,
  normalizeShells,
  orderedSeverities,
  overrideRecoveryText,
  overrideSummary,
  parseKeepCount,
  pathModeLabel,
  previewAliasArgs,
  rowsToEnvironment,
  settingsDirty,
  settingsValidation,
  severityCounts,
  shellLabel,
  targetTypeLabel,
  toggleShell,
  toStringList,
  uninstallRiskSummary,
  validateAliasDraft,
} from "./lib.ts";

function baseAlias() {
  return {
    ...emptyAliasDraft(),
    id: "11111111-1111-1111-1111-111111111111",
    name: "gs",
    description: "git status",
    executable: "git",
    target_type: "native_executable",
    fixed_args: ["status"],
    pass_args: false,
    working_directory: "C:\\work",
    environment: { GIT_PAGER: "cat" },
    shells: ["bash", "zsh"],
    tags: ["git"],
    revision: 7,
    path_mode: "relative_to_working_directory",
    path_origin: "cli_argument",
  };
}

// ── wizard/table helpers ──────────────────────────────────────────

test("shell constants expose the four GUI-supported shells", () => {
  assert.deepEqual([...SHELLS], ["bash", "zsh", "powershell5", "powershell7"]);
});

test("target type options exclude raw shell commands", () => {
  assert.equal(TARGET_TYPES.length, 8);
  assert.ok(TARGET_TYPES.some((option) => option.value === "native_executable"));
  assert.ok(TARGET_TYPES.some((option) => option.value === "change_directory"));
  assert.equal(TARGET_TYPES.some((option) => option.value === "raw_shell_command"), false);
});

test("shell toggle adds and removes without mutating the input", () => {
  const shells = ["bash"];
  assert.deepEqual(toggleShell(shells, "zsh"), ["bash", "zsh"]);
  assert.deepEqual(toggleShell(shells, "bash"), []);
  assert.deepEqual(shells, ["bash"]);
});

test("shell normalization trims and de-duplicates but preserves order and unknown shells", () => {
  assert.deepEqual(normalizeShells([" zsh ", "bash", "zsh", "", "fish"]), ["zsh", "bash", "fish"]);
});

test("display labels cover every shell, target type, and path mode", () => {
  assert.equal(shellLabel("powershell5"), "PowerShell 5.1");
  assert.equal(shellLabel("powershell7"), "PowerShell 7");
  assert.equal(shellLabel("power_shell7"), "PowerShell 7");
  assert.equal(shellLabel("fish"), "Fish");
  assert.equal(shellLabel("custom"), "custom");
  assert.equal(targetTypeLabel("python_script"), "Python script");
  assert.equal(targetTypeLabel("unknown_type"), "unknown_type");
  assert.equal(pathModeLabel("relative_to_working_directory"), "Relative to working directory");
  assert.equal(pathModeLabel(null), "Not recorded");
});

test("fixed argument rows accept both placeholder spellings and keep literal spaces", () => {
  assert.deepEqual(
    normalizeFixedArgs(["--dry-run", "{{args}}", "{{{{args}}}}", "   ", "two words"]),
    ["--dry-run", "{{args}}", "{{{{args}}}}", "two words"],
  );
  assert.match(FIXED_ARGS_PLACEHOLDER_HINT, /\{\{args\}\}/);
  assert.match(FIXED_ARGS_PLACEHOLDER_HINT, /\{\{\{\{args\}\}\}\}/);
});

test("environment rows round-trip, ignore empty keys, trim keys, and keep values verbatim", () => {
  const rows = environmentToRows({ GIT_PAGER: "cat", " SPACED ": " keep me " });
  assert.deepEqual(rows, [{ key: "GIT_PAGER", value: "cat" }, { key: " SPACED ", value: " keep me " }]);
  assert.deepEqual(rowsToEnvironment([...rows, { key: "  ", value: "ignored" }, { key: "GIT_PAGER", value: "less" }]), {
    GIT_PAGER: "less",
    SPACED: " keep me ",
  });
});

test("finalizeAliasDraft normalizes editable fields while preserving identity and path provenance", () => {
  const finalized = finalizeAliasDraft({
    ...baseAlias(),
    tags: [" git ", "git", "work"],
    shells: [" bash ", "bash", "zsh"],
    fixed_args: ["status", "  "],
  });
  assert.deepEqual(finalized.tags, ["git", "work"]);
  assert.deepEqual(finalized.shells, ["bash", "zsh"]);
  assert.deepEqual(finalized.fixed_args, ["status"]);
  assert.equal(finalized.id, "11111111-1111-1111-1111-111111111111");
  assert.equal(finalized.revision, 7);
  assert.equal(finalized.path_mode, "relative_to_working_directory");
  assert.equal(finalized.path_origin, "cli_argument");
});

test("draft validation rejects bad names, missing targets, empty shells, and raw shell commands", () => {
  assert.deepEqual(validateAliasDraft(baseAlias()), []);
  assert.match(validateAliasDraft({ ...baseAlias(), name: "" }).join(" "), /Name must start/);
  assert.match(validateAliasDraft({ ...baseAlias(), name: "bad name" }).join(" "), /Name must start/);
  assert.match(validateAliasDraft({ ...baseAlias(), executable: "  " }).join(" "), /required/);
  assert.match(validateAliasDraft({ ...baseAlias(), shells: [] }).join(" "), /at least one shell/);
  assert.match(validateAliasDraft({ ...baseAlias(), target_type: "raw_shell_command" }).join(" "), /not supported/);
});

test("preview args wrap a finalized alias without writing anything", () => {
  const args = previewAliasArgs({ ...baseAlias(), fixed_args: ["status", ""] });
  assert.deepEqual(Object.keys(args), ["alias"]);
  assert.deepEqual(args.alias.fixed_args, ["status"]);
  assert.equal(args.alias.id, "11111111-1111-1111-1111-111111111111");
});

test("preview response normalization is defensive about malformed payloads", () => {
  assert.deepEqual(normalizePreviewSnippets(null), []);
  assert.deepEqual(
    normalizePreviewSnippets([{ shell: "bash", content: "alias gs='git status'", error: null }, { shell: "zsh", error: "boom" }, "nope"]),
    [
      { shell: "bash", content: "alias gs='git status'", error: null },
      { shell: "zsh", content: "", error: "boom" },
    ],
  );
});

// ── import helpers ────────────────────────────────────────────────

test("import accepts only json and toml filenames and sends content plus filename", () => {
  assert.equal(isImportableFilename("aliases.json"), true);
  assert.equal(isImportableFilename("ALIASES.TOML"), true);
  assert.equal(isImportableFilename("aliases.yaml"), false);
  assert.deepEqual(importContentArgs("[]", "aliases.json"), { content: "[]", filename: "aliases.json" });
});

test("import report normalization and counts handle missing fields and reasons", () => {
  assert.deepEqual(normalizeImportReport(null), { imported: [], skipped: [], warnings: [], unsupported: [] });
  const report = normalizeImportReport({
    imported: ["gui-import-test"],
    skipped: ["INVALID NAME WITH SPACES: invalid name"],
    warnings: ["shell alias rewritten"],
    unsupported: ["raw_shell_command"],
    extra: true,
  });
  assert.deepEqual(report, {
    imported: ["gui-import-test"],
    skipped: ["INVALID NAME WITH SPACES: invalid name"],
    warnings: ["shell alias rewritten"],
    unsupported: ["raw_shell_command"],
  });
  assert.deepEqual(importReportCounts(report), { imported: 1, skipped: 1, warnings: 1, unsupported: 1 });
  assert.deepEqual(toStringList(["a", 1, "", "b"]), ["a", "b"]);
});

// ── doctor helpers ────────────────────────────────────────────────

test("doctor findings normalize severity, shell, action, and detail defensively", () => {
  const findings = normalizeDoctorFindings([
    { shell: "powershell7", severity: "WARNING", code: "generated_missing", message: "missing", action: "sync", detail: "detail" },
    { code: "global" },
    "nope",
  ]);
  assert.equal(findings.length, 2);
  assert.deepEqual(findings[0], {
    shell: "powershell7", severity: "warning", code: "generated_missing", message: "missing", action: "sync", detail: "detail",
  });
  assert.deepEqual(findings[1], { shell: null, severity: "info", code: "global", message: "", action: null, detail: null });
  assert.deepEqual(severityCounts(findings), { warning: 1, info: 1 });
  assert.deepEqual(orderedSeverities({ info: 1, warning: 2, error: 1 }), ["error", "warning", "info"]);
});

test("overridden definitions render defensively and expose recovery text without executing it", () => {
  const entries = normalizeOverriddenDefinitions([
    { name: "old", shell: "bash", definition_kind: "function", original_definition: "alias old='tool'", recoverable: true, captured_at: "2026-01-01" },
    { name: "other", shell: "zsh", definition: "alias other='x'", recoverable: "true" },
    { name: "none" },
    null,
  ]);
  assert.equal(entries.length, 3);
  assert.equal(overrideRecoveryText(entries[0]), "alias old='tool'");
  assert.equal(overrideRecoveryText(entries[1]), "alias other='x'");
  assert.equal(overrideRecoveryText(entries[2]), null);
  assert.equal(entries[1].recoverable, true);
  assert.equal(entries[2].recoverable, false);
  assert.equal(overrideSummary(entries[0]), "old (Bash)");
  assert.equal(overrideSummary(entries[2]), "none (unknown shell)");
});

test("manual-only boundary is explicit about profile, policy, and shell restart limits", () => {
  assert.match(MANUAL_ONLY_BOUNDARY, /Manual-only boundary/);
  assert.match(MANUAL_ONLY_BOUNDARY, /ExecutionPolicy/);
  assert.match(MANUAL_ONLY_BOUNDARY, /never edits your shell profile/);
});

// ── settings helpers ──────────────────────────────────────────────

test("settings normalization supplies safe defaults", () => {
  assert.deepEqual(normalizeSettings(null), {
    config_directory: "",
    default_shell: "bash",
    backup_keep: 10,
    log_keep: 10,
    allow_relative_paths: false,
  });
  assert.deepEqual(normalizeSettings({ config_directory: "/tmp", default_shell: "zsh", backup_keep: 3.9, log_keep: -1, allow_relative_paths: true }), {
    config_directory: "/tmp",
    default_shell: "zsh",
    backup_keep: 3,
    log_keep: 10,
    allow_relative_paths: true,
  });
});

test("settings validation keeps drafts on invalid directories, counts, and shells", () => {
  const valid = normalizeSettings({ config_directory: "/tmp", default_shell: "zsh", backup_keep: 3, log_keep: 4 });
  assert.deepEqual(settingsValidation(valid), []);
  assert.match(settingsValidation({ ...valid, config_directory: " " }).join(" "), /Config directory/);
  assert.match(settingsValidation({ ...valid, backup_keep: -1 }).join(" "), /Backup keep/);
  assert.match(settingsValidation({ ...valid, log_keep: 1.5 }).join(" "), /Log keep/);
  assert.match(settingsValidation({ ...valid, default_shell: "fish" }).join(" "), /default shell/);
});

test("parseKeepCount floors valid values and falls back for blanks and negatives", () => {
  assert.equal(parseKeepCount(" 5 "), 5);
  assert.equal(parseKeepCount("3.8"), 3);
  assert.equal(parseKeepCount("", 7), 7);
  assert.equal(parseKeepCount("-2", 9), 9);
  assert.equal(parseKeepCount("abc", 4), 4);
});

test("settingsDirty detects changes field by field", () => {
  const loaded = normalizeSettings({ config_directory: "/tmp", default_shell: "bash", backup_keep: 3, log_keep: 4 });
  assert.equal(settingsDirty(loaded, { ...loaded }), false);
  assert.equal(settingsDirty(loaded, { ...loaded, default_shell: "zsh" }), true);
  assert.equal(settingsDirty(loaded, { ...loaded, allow_relative_paths: true }), true);
});

// ── uninstall helpers ─────────────────────────────────────────────

test("uninstall risk summaries distinguish retain and purge while protecting targets", () => {
  const retain = uninstallRiskSummary(false);
  const purge = uninstallRiskSummary(true);
  assert.ok(retain.kept.some((item) => /Alias database/.test(item)));
  assert.ok(retain.removed.some((item) => /loader block/.test(item)));
  assert.ok(purge.removed.some((item) => /Alias database/.test(item)));
  assert.ok(purge.removed.some((item) => /Generated shell definition files/.test(item)));
  assert.ok(retain.protected.join(" ").includes("never deleted"));
  assert.ok(purge.protected.join(" ").includes("never deleted"));
  assert.notDeepEqual(retain, purge);
});

// ── component source contracts ────────────────────────────────────

async function source(name: string): Promise<string> {
  return readFile(new URL(`./components/${name}.tsx`, import.meta.url), "utf8");
}

test("AliasWizard exposes complete create/edit fields, preview, and draft-preserving errors", async () => {
  const wizard = await source("AliasWizard");
  for (const fragment of [
    'value={draft.name}', "target_type", "TARGET_TYPES", "SHELLS", "pass_args",
    "Fixed argument", "Add argument", "Environment key", "Add variable",
    "working_directory", "TagInput", "Show advanced", "preview_alias", "previewAliasArgs",
    "normalizePreviewSnippets", "Cancel", "Save", "Unable to save", "role=\"alert\"",
  ]) {
    assert.ok(wizard.includes(fragment), `wizard is missing ${fragment}`);
  }
  assert.equal(wizard.includes("raw_shell_command"), false);
  assert.equal(/\.focus\(/.test(wizard), false);
  assert.equal(/dangerouslySetInnerHTML|eval\(|new Function/.test(wizard), false);
});

test("AliasTable renders every required column, tooltip, expansion, and action", async () => {
  const table = await source("AliasTable");
  for (const header of ["Name", "Target", "Target type", "Shells", "Description", "Tags", "Status", "Actions"]) {
    assert.ok(table.includes(`>${header}<`), `table is missing ${header}`);
  }
  for (const fragment of ["title={alias.description}", "table-scroll", "alias.shells.map(shellLabel)", "alias.enabled", "Confirm delete", "pathModeLabel", "working_directory", "environment", "fixed_args"]) {
    assert.ok(table.includes(fragment), `table is missing ${fragment}`);
  }
});

test("DoctorPanel counts severities, expands findings, and copies recovery text without executing it", async () => {
  const doctor = await source("DoctorPanel");
  for (const fragment of [
    "doctor_status", "overridden_definitions", "severityCounts", "orderedSeverities",
    "navigator.clipboard.writeText", "Copy recovery text", "Copy suggestion",
    "MANUAL_ONLY_BOUNDARY", "finding.action", "finding.detail", "recoverable",
  ]) {
    assert.ok(doctor.includes(fragment), `doctor is missing ${fragment}`);
  }
  assert.equal(/eval\(|new Function|child_process/.test(doctor), false);
});

test("ImportPanel uses a real file picker and content commands without persistence before confirm", async () => {
  const importer = await source("ImportPanel");
  for (const fragment of [
    'type="file"', 'accept=".json,.toml"', "selected.text()", "import_preview_content",
    "import_confirm_content", "importContentArgs", "Confirm import", "Preview only", "Cancel",
    "Imported", "Skipped", "Warnings", "Unsupported",
  ]) {
    assert.ok(importer.includes(fragment), `import panel is missing ${fragment}`);
  }
  assert.ok(importer.indexOf("import_preview_content") < importer.indexOf("import_confirm_content"));
});

test("SettingsPanel keeps a draft, resets on cancel, and calls config_save with settings", async () => {
  const settings = await source("SettingsPanel");
  for (const fragment of [
    "config_get", "config_save", "{ settings: draft }", "Cancel", "Reload persisted values",
    "setDraft({ ...loaded })", "setDraft({ ...stored })", "allow_relative_paths", "backup_keep", "log_keep",
  ]) {
    assert.ok(settings.includes(fragment), `settings panel is missing ${fragment}`);
  }
});

test("UninstallPanel requires review then confirmation for both modes", async () => {
  const uninstall = await source("UninstallPanel");
  for (const fragment of [
    "uninstall_preview", "uninstall_confirm", 'stage !== "reviewed"', "Confirm purge",
    "Confirm uninstall", "Review purge", "Review uninstall", "uninstallRiskSummary", "Kept", "Removed",
  ]) {
    assert.ok(uninstall.includes(fragment), `uninstall panel is missing ${fragment}`);
  }
});

test("GeneratedFilePanel offers shell selection, preview, reload command, and copy buttons", async () => {
  const generated = await source("GeneratedFilePanel");
  for (const fragment of ["generated_preview", "reload_command", "SHELLS", "copyToClipboard", "Copy", "Generated file preview", "Reload command"]) {
    assert.ok(generated.includes(fragment), `generated panel is missing ${fragment}`);
  }
});

test("Sync page hosts ImportPanel and GeneratedFilePanel; navigation order is unchanged", async () => {
  const app = await readFile(new URL("./App.tsx", import.meta.url), "utf8");
  assert.match(app, /page !== "sync"[^]*<ImportPanel \/><GeneratedFilePanel \/>/);
  assert.match(app, /NAV_ITEMS\.map/);
  assert.match(app, /hidden=\{page !== "aliases"\}/);
});

test("accessibility styles provide focus-visible, horizontal table scroll, and responsive layout", async () => {
  const css = await readFile(new URL("./styles.css", import.meta.url), "utf8");
  assert.ok(css.includes(":focus-visible"));
  assert.ok(css.includes("overflow-x: auto"));
  assert.ok(css.includes("min-width: 940px"));
  assert.ok(css.includes("@media (max-width: 760px)"));
  assert.ok(css.includes("visually-hidden"));
});
