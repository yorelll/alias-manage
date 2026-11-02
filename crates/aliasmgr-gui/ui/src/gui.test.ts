import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  SHELLS,
  TARGET_TYPES,
  aliasFiltersActive,
  clearAliasFilters,
  detectionSourceLabel,
  doctorFindingText,
  emptyAliasDraft,
  environmentToRows,
  finalizeAliasDraft,
  findingCodeLabel,
  fixedArgsPlaceholderHint,
  importContentArgs,
  importReportCounts,
  importSkipEntries,
  isImportableFilename,
  latestPreviewResponse,
  manualOnlyBoundary,
  nextDataVersion,
  normalizeDoctorFindings,
  normalizeFixedArgs,
  normalizeImportReport,
  normalizeOverriddenDefinitions,
  normalizePreviewSnippets,
  normalizeSettings,
  normalizeShells,
  normalizeShellValue,
  orderedSeverities,
  overrideMetadataText,
  overrideSummary,
  parseKeepCount,
  parseKeepCountInput,
  pathModeLabel,
  pathOriginLabel,
  previewAliasArgs,
  rowsToEnvironment,
  settingsDirty,
  settingsValidation,
  severityCounts,
  shellLabel,
  statusShellText,
  targetTypeLabel,
  toggleShell,
  toStringList,
  uninstallOutcomeText,
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

test("clear filters resets query, fuzzy, and tags together", () => {
  assert.deepEqual(clearAliasFilters(), { query: "", fuzzy: false, tags: [] });
  assert.equal(aliasFiltersActive("git", false, []), true);
  assert.equal(aliasFiltersActive("", true, []), true);
  assert.equal(aliasFiltersActive("", false, ["work"]), true);
  assert.equal(aliasFiltersActive("   ", false, []), false);
});

test("data invalidation always advances the shared refresh token", () => {
  assert.equal(nextDataVersion(0), 1);
  assert.equal(nextDataVersion(1), 2);
  assert.equal(nextDataVersion(41), 42);
});

test("status shell text prefers installed_shells and falls back to the detected shell", () => {
  assert.equal(statusShellText(null), "—");
  assert.equal(statusShellText({
    version: "0.1.0", detected_shell: "bash", detection_source: "shell_env", config_directory: "/tmp",
    installed_shells: ["powershell5", "powershell7", "powershell7"],
  }), "PowerShell 5.1, PowerShell 7");
  assert.equal(statusShellText({
    version: "0.1.0", detected_shell: "bash", detection_source: "shell_env", config_directory: "/tmp",
    installed_shells: null,
  }), "Bash (Shell 环境变量)");
  assert.equal(statusShellText({
    version: "0.1.0", detected_shell: "bash", detection_source: "shell_env", config_directory: "/tmp",
    installed_shells: null,
  }, "en"), "Bash (shell environment)");
  assert.equal(statusShellText({
    version: "0.1.0", detected_shell: null, detection_source: "installed", config_directory: "/tmp",
  }), "未检测到");
  assert.equal(statusShellText({
    version: "0.1.0", detected_shell: null, detection_source: "installed", config_directory: "/tmp",
  }, "en"), "none detected");
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

test("display labels cover every shell, target type, and path mode in both languages", () => {
  assert.equal(shellLabel("powershell5", "en"), "PowerShell 5.1");
  assert.equal(shellLabel("powershell7", "en"), "PowerShell 7");
  assert.equal(shellLabel("power_shell7", "en"), "PowerShell 7");
  assert.equal(shellLabel('"bash"', "en"), "Bash");
  assert.equal(shellLabel('"power_shell5"', "en"), "PowerShell 5.1");
  assert.equal(shellLabel("fish", "en"), "Fish");
  assert.equal(shellLabel("custom", "en"), "custom");
  assert.equal(shellLabel("custom"), "custom");
  assert.equal(shellLabel("", "zh"), "未知 Shell");
  assert.equal(normalizeShellValue('"bash"'), "bash");
  assert.equal(normalizeShellValue('"power_shell5"'), "power_shell5");
  assert.equal(normalizeShellValue(42), "");
  assert.equal(targetTypeLabel("python_script", "en"), "Python script");
  assert.equal(targetTypeLabel("python_script"), "Python 脚本");
  assert.equal(targetTypeLabel("unknown_type", "en"), "unknown_type");
  assert.equal(pathModeLabel("relative_to_working_directory", "en"), "Relative to working directory");
  assert.equal(pathModeLabel("relative_to_working_directory"), "相对于工作目录");
  assert.equal(pathModeLabel(null, "en"), "Not recorded");
  assert.equal(pathModeLabel(null), "未记录");
});

test("controlled enum displays are localized and unknown values stay raw", () => {
  assert.equal(pathOriginLabel("gui_file_picker", "en"), "GUI file picker");
  assert.equal(pathOriginLabel("gui_file_picker"), "GUI 文件选择器");
  assert.equal(pathOriginLabel("cli_argument"), "CLI 参数");
  assert.equal(pathOriginLabel("import", "en"), "Import");
  assert.equal(pathOriginLabel("mystery_origin"), "mystery_origin");
  assert.equal(pathOriginLabel(null, "en"), "Not recorded");
  assert.equal(detectionSourceLabel("explicit", "en"), "explicit");
  assert.equal(detectionSourceLabel("parent"), "父 Shell");
  assert.equal(detectionSourceLabel("shell_env", "en"), "shell environment");
  assert.equal(detectionSourceLabel("login_shell"), "登录 Shell");
  assert.equal(detectionSourceLabel("installed", "en"), "installed");
  assert.equal(detectionSourceLabel("custom_source"), "custom_source");
  assert.equal(targetTypeLabel("raw_shell_command", "en"), "Raw shell command");
  assert.equal(targetTypeLabel("raw_shell_command"), "原始 Shell 命令");
  assert.equal(targetTypeLabel("totally_unknown"), "totally_unknown");
  assert.equal(findingCodeLabel("stale", "en"), "Generated file out of date");
  assert.equal(findingCodeLabel("stale"), "生成文件已过期");
  assert.equal(findingCodeLabel("custom_code"), "custom_code");
  assert.equal(findingCodeLabel(""), "问题");
});

test("fixed argument rows accept both placeholder spellings and keep literal spaces", () => {
  assert.deepEqual(
    normalizeFixedArgs(["--dry-run", "{{args}}", "{{{{args}}}}", "   ", "two words"]),
    ["--dry-run", "{{args}}", "{{{{args}}}}", "two words"],
  );
  assert.match(fixedArgsPlaceholderHint("en"), /\{\{args\}\}/);
  assert.match(fixedArgsPlaceholderHint("en"), /\{\{\{\{args\}\}\}\}/);
  assert.match(fixedArgsPlaceholderHint("zh"), /\{\{args\}\}/);
  assert.match(fixedArgsPlaceholderHint("zh"), /\{\{\{\{args\}\}\}\}/);
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
  assert.match(validateAliasDraft({ ...baseAlias(), name: "" }, "en").join(" "), /Name must start/);
  assert.match(validateAliasDraft({ ...baseAlias(), name: "bad name" }, "en").join(" "), /Name must start/);
  assert.match(validateAliasDraft({ ...baseAlias(), executable: "  " }, "en").join(" "), /required/);
  assert.match(validateAliasDraft({ ...baseAlias(), shells: [] }, "en").join(" "), /at least one shell/);
  assert.match(validateAliasDraft({ ...baseAlias(), target_type: "raw_shell_command" }, "en").join(" "), /not supported/);
  assert.match(validateAliasDraft({ ...baseAlias(), name: "" }).join(" "), /名称必须/);
  assert.match(validateAliasDraft({ ...baseAlias(), shells: [] }).join(" "), /至少选择一个 Shell/);
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

test("only the latest preview response is rendered", () => {
  const stale = [{ shell: "bash", content: "old", error: null }];
  const latest = [{ shell: "bash", content: "new", error: null }];
  assert.equal(latestPreviewResponse(3, 2, stale), null);
  assert.deepEqual(latestPreviewResponse(3, 3, latest), latest);
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

test("skipped import names are paired with inline or warning reasons", () => {
  const report = normalizeImportReport({
    skipped: ["INVALID NAME WITH SPACES: invalid name", "duplicate"],
    warnings: ["duplicate: name already exists", "unrelated warning"],
  });
  assert.deepEqual(importSkipEntries(report, "en"), [
    { name: "INVALID NAME WITH SPACES", reason: "invalid name" },
    { name: "duplicate", reason: "duplicate: name already exists" },
  ]);
  assert.deepEqual(importSkipEntries(normalizeImportReport({ skipped: ["mystery"] }), "en"), [
    { name: "mystery", reason: null },
  ]);
});

test("import skip reasons parse backend prefixes without short-name collisions", () => {
  const collision = normalizeImportReport({
    skipped: ["gs", "gst"],
    warnings: ["name conflict skipped: gst"],
  });
  assert.deepEqual(importSkipEntries(collision, "en"), [
    { name: "gs", reason: null },
    { name: "gst", reason: "name conflict" },
  ]);
  const prefixed = normalizeImportReport({
    skipped: ["bad name", "chain"],
    warnings: [
      "invalid alias rejected: bad name (name is reserved)",
      "relative path requires a selected base directory: chain",
    ],
  });
  assert.deepEqual(importSkipEntries(prefixed, "en"), [
    { name: "bad name", reason: "name is reserved" },
    { name: "chain", reason: "relative path requires a selected base directory" },
  ]);
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

test("known doctor codes use localized copy and unknown codes fall back to backend text", () => {
  const finding = normalizeDoctorFindings([
    { code: "generated_missing", severity: "warning", message: "生成文件缺失", action: "执行同步" },
  ])[0];
  const knownEn = doctorFindingText(finding, "en");
  assert.equal(knownEn.title, "Generated file missing");
  assert.doesNotMatch(knownEn.message, /[一-鿿]/);
  assert.doesNotMatch(knownEn.action ?? "", /[一-鿿]/);
  const knownZh = doctorFindingText(finding, "zh");
  assert.equal(knownZh.title, "生成文件缺失");
  assert.match(knownZh.message, /[一-鿿]/);
  assert.match(knownZh.action ?? "", /[一-鿿]/);
  assert.equal(doctorFindingText(finding).title, "生成文件缺失", "default language is Chinese");

  const unknown = doctorFindingText(normalizeDoctorFindings([
    { code: "custom_code", severity: "info", message: "backend text", action: "backend action" },
  ])[0], "en");
  assert.deepEqual(unknown, { title: "custom_code", message: "backend text", action: "backend action" });
  const unknownZh = doctorFindingText(normalizeDoctorFindings([
    { code: "custom_code", severity: "info", message: "backend text", action: "backend action" },
  ])[0]);
  assert.equal(unknownZh.message, "backend text", "backend text stays the fallback");
  for (const code of ["database_missing", "generated_missing", "generated_unreadable", "generated_modified", "sync_failed", "loader_missing", "stale", "unreliable_filesystem", "render_failed"]) {
    const mappedEn = doctorFindingText(normalizeDoctorFindings([{ code, severity: "warning", message: "中文", action: "中文" }])[0], "en");
    const mappedZh = doctorFindingText(normalizeDoctorFindings([{ code, severity: "warning", message: "中文", action: "中文" }])[0], "zh");
    assert.doesNotMatch(`${mappedEn.title} ${mappedEn.message} ${mappedEn.action ?? ""}`, /[一-鿿]/, `unmapped English copy for ${code}`);
    assert.match(`${mappedZh.title} ${mappedZh.message} ${mappedZh.action ?? ""}`, /[一-鿿]/, `unmapped Chinese copy for ${code}`);
  }
});

test("overridden definitions keep metadata only and never expose captured definition text", () => {
  const entries = normalizeOverriddenDefinitions([
    { name: "old", shell: '"bash"', definition_kind: "function", original_definition: "alias old='tool'", recovery_text: "alias old='tool'", definition: "function old { tool }", recoverable: true, captured_at: "2026-01-01" },
    { name: "other", shell: '"power_shell7"', definition: "alias other='x'", recoverable: "true" },
    { name: "none", shell: 42 },
    null,
  ]);
  assert.equal(entries.length, 3);
  assert.equal(entries[0].shell, "bash");
  assert.equal(entries[1].shell, "power_shell7");
  assert.equal(entries[2].shell, "");
  assert.equal("original_definition" in entries[0], false);
  assert.equal("recovery_text" in entries[0], false);
  assert.equal("definition" in entries[0], false);
  assert.equal(entries[1].recoverable, true);
  assert.equal(entries[2].recoverable, false);
  assert.equal(overrideSummary(entries[0], "en"), "old (Bash)");
  assert.equal(overrideSummary(entries[2], "en"), "none (unknown shell)");
  assert.equal(overrideSummary(entries[2], "zh"), "none (未知 Shell)");
  const metadata = overrideMetadataText(entries[0], "en");
  assert.match(metadata, /name: old/);
  assert.match(metadata, /shell: bash/);
  assert.match(metadata, /kind: function/);
  assert.match(metadata, /recoverable: true/);
  assert.match(metadata, /captured_at: 2026-01-01/);
  assert.doesNotMatch(metadata, /alias old='tool'/);
  assert.doesNotMatch(metadata, /function old \{ tool \}/);
  const metadataZh = overrideMetadataText(entries[0], "zh");
  assert.match(metadataZh, /名称: old/);
  assert.match(metadataZh, /可恢复: true/);
  assert.match(metadataZh, /捕获时间: 2026-01-01/);
  const unnamed = normalizeOverriddenDefinitions([{ shell: "bash" }])[0];
  assert.equal(unnamed.name, "");
  assert.equal(overrideSummary(unnamed, "en"), "(unnamed) (Bash)");
  assert.equal(overrideSummary(unnamed), "（未命名） (Bash)");
  assert.match(overrideMetadataText(unnamed, "en"), /name: \(unnamed\)/);
});

test("manual-only boundary is explicit about profile, policy, and shell restart limits in both languages", () => {
  const boundaryEn = manualOnlyBoundary("en");
  assert.match(boundaryEn, /Manual-only boundary/);
  assert.match(boundaryEn, /never bypasses ExecutionPolicy or restarts your shell/);
  assert.match(boundaryEn, /edits only the marked Alias Manager block/);
  assert.match(boundaryEn, /leaves the rest of the profile untouched/);
  const boundaryZh = manualOnlyBoundary("zh");
  assert.match(boundaryZh, /仅手动边界/);
  assert.match(boundaryZh, /ExecutionPolicy/);
  assert.match(boundaryZh, /标记为 Alias Manager 的区块/);
});

// ── settings helpers ──────────────────────────────────────────────

test("settings normalization supplies safe defaults", () => {
  assert.deepEqual(normalizeSettings(null), {
    config_directory: "",
    default_shell: "bash",
    backup_keep: 10,
    log_keep: 10,
    allow_relative_paths: false,
    language: "zh",
  });
  assert.deepEqual(normalizeSettings({ config_directory: "/tmp", default_shell: "zsh", backup_keep: 3.9, log_keep: -1, allow_relative_paths: true }), {
    config_directory: "/tmp",
    default_shell: "zsh",
    backup_keep: 3,
    log_keep: 10,
    allow_relative_paths: true,
    language: "zh",
  });
  assert.equal(normalizeSettings({ language: "en" }).language, "en");
  assert.equal(normalizeSettings({ language: "fr" }).language, "zh", "unknown languages fall back to Chinese");
});

test("settings validation keeps drafts on invalid directories, counts, and shells", () => {
  const valid = normalizeSettings({ config_directory: "/tmp", default_shell: "zsh", backup_keep: 3, log_keep: 4 });
  assert.deepEqual(settingsValidation(valid), []);
  assert.match(settingsValidation({ ...valid, config_directory: " " }, "en").join(" "), /Config directory/);
  assert.match(settingsValidation({ ...valid, backup_keep: -1 }, "en").join(" "), /Backup keep/);
  assert.match(settingsValidation({ ...valid, log_keep: 1.5 }, "en").join(" "), /Log keep/);
  assert.match(settingsValidation({ ...valid, default_shell: "fish" }, "en").join(" "), /default shell/);
  assert.match(settingsValidation({ ...valid, config_directory: " " }).join(" "), /配置目录/);
  assert.match(settingsValidation({ ...valid, backup_keep: -1 }).join(" "), /备份保留数/);
});

test("parseKeepCount floors valid values and falls back for blanks and negatives", () => {
  assert.equal(parseKeepCount(" 5 "), 5);
  assert.equal(parseKeepCount("3.8"), 3);
  assert.equal(parseKeepCount("", 7), 7);
  assert.equal(parseKeepCount("-2", 9), 9);
  assert.equal(parseKeepCount("abc", 4), 4);
});

test("keep-count inputs preserve the previous value when blank or invalid", () => {
  assert.equal(parseKeepCountInput("", 4), 4);
  assert.equal(parseKeepCountInput("  ", 4), 4);
  assert.equal(parseKeepCountInput("-1", 4), 4);
  assert.equal(parseKeepCountInput("0", 4), 0);
  assert.equal(parseKeepCountInput("6.9", 4), 6);
});

test("settingsDirty detects changes field by field", () => {
  const loaded = normalizeSettings({ config_directory: "/tmp", default_shell: "bash", backup_keep: 3, log_keep: 4 });
  assert.equal(settingsDirty(loaded, { ...loaded }), false);
  assert.equal(settingsDirty(loaded, { ...loaded, default_shell: "zsh" }), true);
  assert.equal(settingsDirty(loaded, { ...loaded, allow_relative_paths: true }), true);
  assert.equal(settingsDirty(loaded, { ...loaded, language: "en" }), true);
});

test("settings language draft applies immediately, cancel restores, and save persists", async () => {
  const settings = await source("SettingsPanel");
  // Changing the selector updates the draft and the live UI language.
  assert.match(settings, /edit\(\{ language: next \}\)/);
  assert.match(settings, /setLanguage\(next\)/);
  // Cancel restores the loaded language together with the loaded draft.
  assert.match(settings, /setLanguage\(loaded\.language\)/);
  // A successful save adopts the stored language; a failure keeps the draft/error.
  assert.match(settings, /setLanguage\(stored\.language\)/);
  assert.match(settings, /catch[^]*setError\(String\(reason\)\)/);
  assert.match(settings, /LANGUAGES\.map/);
});

// ── uninstall helpers ─────────────────────────────────────────────

test("uninstall risk summaries distinguish retain and purge while protecting targets", () => {
  const retain = uninstallRiskSummary(false, "en");
  const purge = uninstallRiskSummary(true, "en");
  assert.ok(retain.kept.some((item) => /Alias database/.test(item)));
  assert.ok(retain.removed.some((item) => /loader block/.test(item)));
  assert.ok(purge.removed.some((item) => /Alias database/.test(item)));
  assert.ok(purge.removed.some((item) => /Generated shell definition files/.test(item)));
  assert.ok(retain.protected.join(" ").includes("never deleted"));
  assert.ok(purge.protected.join(" ").includes("never deleted"));
  assert.notDeepEqual(retain, purge);
  const retainZh = uninstallRiskSummary(false, "zh");
  const purgeZh = uninstallRiskSummary(true, "zh");
  assert.ok(retainZh.kept.some((item) => /别名数据库/.test(item)));
  assert.ok(retainZh.removed.some((item) => /加载器区块/.test(item)));
  assert.ok(purgeZh.protected.join(" ").includes("绝不会被删除"));
  assert.ok(retainZh.kept.every((item) => /[一-鿿]/.test(item)));
});

test("uninstall outcome copy is localized for both modes and languages", () => {
  assert.match(uninstallOutcomeText(false, "en"), /loader block and shims/);
  assert.match(uninstallOutcomeText(false, "en"), /keeping the alias database/);
  assert.match(uninstallOutcomeText(true, "en"), /removed its database/);
  assert.match(uninstallOutcomeText(true, "en"), /Referenced target files were not touched/);
  assert.doesNotMatch(uninstallOutcomeText(false, "en"), /[一-鿿]/);
  assert.doesNotMatch(uninstallOutcomeText(true, "en"), /[一-鿿]/);
  assert.match(uninstallOutcomeText(false), /加载器区块/);
  assert.match(uninstallOutcomeText(true), /数据库/);
  assert.match(uninstallOutcomeText(true), /未受影响/);
  assert.doesNotMatch(uninstallOutcomeText(true, "zh"), /Referenced target files/);
});

// ── component source contracts ────────────────────────────────────

async function source(name: string): Promise<string> {
  return readFile(new URL(`./components/${name}.tsx`, import.meta.url), "utf8");
}

test("AliasWizard exposes complete create/edit fields, preview, and draft-preserving errors", async () => {
  const wizard = await source("AliasWizard");
  for (const fragment of [
    'value={draft.name}', "target_type", "TARGET_TYPES", "SHELLS", "pass_args",
    "wizard.fixedArgumentAria", "wizard.addArgument", "wizard.envKeyAria", "wizard.addVariable",
    "working_directory", "TagInput", "wizard.showAdvanced", "preview_alias", "previewAliasArgs",
    "normalizePreviewSnippets", "latestPreviewResponse", "previewToken", "invalidatePreview",
    "nameRef", "isConnected", "common.cancel", "common.save", "wizard.unableToSave", "role=\"alert\"",
    "disabled={saving}",
  ]) {
    assert.ok(wizard.includes(fragment), `wizard is missing ${fragment}`);
  }
  assert.equal(wizard.includes("raw_shell_command"), false);
  assert.match(wizard, /nameRef\.current\?\.focus\(\)/);
  assert.match(wizard, /opener\?\.isConnected/);
  assert.equal(/dangerouslySetInnerHTML|eval\(|new Function/.test(wizard), false);
});

test("AliasTable renders every required column, tooltip, expansion, and action", async () => {
  const table = await source("AliasTable");
  for (const header of ["table.name", "table.target", "table.targetType", "table.shells", "table.description", "table.tags", "table.status", "table.actions"]) {
    assert.ok(table.includes(`t("${header}")`), `table is missing ${header}`);
  }
  for (const fragment of ["title={alias.description}", "table-scroll", "alias.shells.map", "alias.enabled", "table.confirmDelete", "pathModeLabel", "working_directory", "environment", "fixed_args"]) {
    assert.ok(table.includes(fragment), `table is missing ${fragment}`);
  }
});

test("DoctorPanel scans overrides and copies desensitized metadata only", async () => {
  const doctor = await source("DoctorPanel");
  for (const fragment of [
    "doctor_status", "scan_overrides", "severityCounts", "orderedSeverities",
    "navigator.clipboard.writeText", "doctor.copyMetadata", "doctor.copySuggestion",
    "manualOnlyBoundary", "doctorFindingText", "findingCodeLabel", "overrideMetadataText",
    "text.message", "text.action", "finding.detail", "recoverable", "copyError", "title={finding.code}",
  ]) {
    assert.ok(doctor.includes(fragment), `doctor is missing ${fragment}`);
  }
  assert.equal(doctor.includes("overridden_definitions"), false);
  assert.equal(/original_definition|recovery_text|entry\.definition\b/.test(doctor), false);
  assert.match(doctor, /setCopyError\(t\("doctor\.copyFailed"/);
  assert.equal(/setError\(t\("doctor\.copyFailed"/.test(doctor), false, "copy errors must not be treated as load errors");
  assert.equal(/eval\(|new Function|child_process/.test(doctor), false);
});

test("ImportPanel uses a real file picker and content commands without persistence before confirm", async () => {
  const importer = await source("ImportPanel");
  for (const fragment of [
    'type="file"', 'id="import-file-input"', 'accept=".json,.toml"', "selected.text()", "import_preview_content",
    "import_confirm_content", "importContentArgs", "importSkipEntries", "onImported",
    "import.confirm", "import.previewOnly", "common.cancel", "import.imported", "import.skipped", "import.warnings", "import.unsupported",
    'disabled={busy === "confirm"}', 'busy === "confirm" ? t("import.importingShort")',
  ]) {
    assert.ok(importer.includes(fragment), `import panel is missing ${fragment}`);
  }
  assert.ok(importer.indexOf("import_preview_content") < importer.indexOf("import_confirm_content"));
});

test("SettingsPanel keeps a draft, resets on cancel, and calls config_save with settings", async () => {
  const settings = await source("SettingsPanel");
  for (const fragment of [
    "config_get", "config_save", "{ settings: draft }", "common.cancel", "settings.reload",
    "setDraft({ ...loaded })", "setDraft({ ...stored })", "allow_relative_paths", "backup_keep", "log_keep",
    "readOnly", "parseKeepCountInput", "settings.configDirHintBefore", "disabled={busy}", "settings.language",
  ]) {
    assert.ok(settings.includes(fragment), `settings panel is missing ${fragment}`);
  }
});

test("UninstallPanel requires review then confirmation for both modes", async () => {
  const uninstall = await source("UninstallPanel");
  for (const fragment of [
    "uninstall_preview", "uninstall_confirm", 'stage !== "reviewed"', "uninstall.confirmPurge",
    "uninstall.confirmUninstall", "uninstall.reviewPurge", "uninstall.reviewUninstall", "uninstallRiskSummary", "uninstallOutcomeText",
    "onUninstalled", "uninstall.kept", "uninstall.removed",
  ]) {
    assert.ok(uninstall.includes(fragment), `uninstall panel is missing ${fragment}`);
  }
  assert.equal(/resultItems|previewItems/.test(uninstall), false);
});

test("GeneratedFilePanel offers shell selection, preview, reload command, and copy buttons", async () => {
  const generated = await source("GeneratedFilePanel");
  for (const fragment of ["generated_preview", "reload_command", "SHELLS", "copyToClipboard", "common.copy", "generated.previewTitle", "generated.reloadTitle"]) {
    assert.ok(generated.includes(fragment), `generated panel is missing ${fragment}`);
  }
});

test("Sync page hosts ImportPanel and GeneratedFilePanel; navigation order is unchanged", async () => {
  const app = await readFile(new URL("./App.tsx", import.meta.url), "utf8");
  assert.match(app, /page !== "sync"[^]*<ImportPanel onImported=\{invalidateAliasData\} \/><GeneratedFilePanel \/>/);
  assert.match(app, /NAV_ITEMS\.map/);
  assert.match(app, /hidden=\{page !== "aliases"\}/);
  assert.match(app, /refreshToken=\{dataVersion\}/);
  assert.match(app, /onUninstalled=\{invalidateAliasData\}/);
  assert.match(app, /statusShellText\(status, language\)/);
  assert.match(app, /t\("aliases.clearFilters"\)/);
  assert.match(app, /onOpenImport/);
  assert.match(app, /nextDataVersion/);
  assert.match(app, /import-file-input/);
  assert.match(app, /fileInput\.focus\(\)/);
});

test("TagInput has a real label, id, and accessible remove controls", async () => {
  const tagInput = await source("TagInput");
  for (const fragment of ['htmlFor={inputId}', 'id={inputId}', "aria-label", "useId", 't("common.remove")', "datalist"]) {
    assert.ok(tagInput.includes(fragment), `tag input is missing ${fragment}`);
  }
});

test("localized transient messages are cleared when the language changes", async () => {
  const cases: Array<[string, string]> = [
    ["AliasWizard", "useEffect(() => { setError(null); setPreviewError(null); }, [language]);"],
    ["DoctorPanel", "useEffect(() => { setCopyNotice(null); setCopyError(null); }, [language]);"],
    ["GeneratedFilePanel", "useEffect(() => { setCopied(null); setError(null); }, [language]);"],
    ["ImportPanel", "useEffect(() => { setError(null); }, [language]);"],
  ];
  for (const [name, effect] of cases) {
    const component = await source(name);
    assert.ok(component.includes(effect), `${name} does not clear transient copy on language change`);
  }
});

test("accessibility styles provide focus-visible, horizontal table scroll, and responsive layout", async () => {
  const css = await readFile(new URL("./styles.css", import.meta.url), "utf8");
  assert.ok(css.includes(":focus-visible"));
  assert.ok(css.includes("overflow-x: auto"));
  assert.ok(css.includes("min-width: 940px"));
  assert.ok(css.includes("@media (max-width: 760px)"));
  assert.ok(css.includes("visually-hidden"));
});
