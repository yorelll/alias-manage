import type { Language, TranslationKey } from "./i18n.ts";
import { DEFAULT_LANGUAGE, hasTranslation, isLanguage, normalizeLanguage, t } from "./i18n.ts";

export interface StartupStatus {
  version: string;
  detected_shell: string | null;
  detection_source: string;
  config_directory: string;
  /** Optional backend extension: every shell detected on this machine. */
  installed_shells?: string[] | null;
}

export type Page = "aliases" | "sync" | "doctor" | "settings";

export const NAV_ITEMS = [
  { id: "aliases", labelKey: "nav.aliases" },
  { id: "sync", labelKey: "nav.sync" },
  { id: "doctor", labelKey: "nav.doctor" },
  { id: "settings", labelKey: "nav.settings" },
] as const satisfies ReadonlyArray<{ id: Page; labelKey: TranslationKey }>;

/** Sidebar label for a page in the selected language. */
export function navLabel(language: Language, id: Page): string {
  const item = NAV_ITEMS.find((entry) => entry.id === id);
  return t(language, item?.labelKey ?? "nav.aliases");
}

export type RestrictionMode = "whitelist" | "blacklist";

export interface PathRestriction {
  mode: RestrictionMode;
  paths: string[];
}

export interface AliasDto {
  id: string;
  name: string;
  description: string;
  executable: string;
  target_type: string;
  fixed_args: string[];
  pass_args: boolean;
  working_directory: string | null;
  environment: Record<string, string>;
  shells: string[];
  enabled: boolean;
  tags: string[];
  restriction?: PathRestriction | null;
  revision: number;
  path_mode?: string | null;
  path_origin?: string | null;
}

/** Placeholder id sent when creating an alias; the Rust layer assigns a real UUID. */
export const NEW_ALIAS_ID = "00000000-0000-0000-0000-000000000000";

export function normalizeTags(values: string[]): string[] { return [...new Set(values.map((value) => value.trim()).filter(Boolean))]; }
export function truncateDescription(full: string, max: number): { display: string; full: string; truncated: boolean } { return full.length <= max ? { display: full, full, truncated: false } : { display: full.slice(0, max), full, truncated: true }; }
export function statusSummaryText(status: StartupStatus | null, error: boolean, language: Language = DEFAULT_LANGUAGE): string {
  if (status) return `v${status.version} · ${status.detected_shell ?? t(language, "status.noShell")}`;
  if (error) return t(language, "status.unavailable");
  return t(language, "common.loading");
}

/** Shell list for the status drawer, preferring backend `installed_shells`. */
export function statusShellText(status: StartupStatus | null, language: Language = DEFAULT_LANGUAGE): string {
  if (!status) return "—";
  const installed = Array.isArray(status.installed_shells)
    ? [...new Set(status.installed_shells.filter((shell): shell is string => typeof shell === "string" && shell.length > 0))]
    : [];
  if (installed.length > 0) return installed.map((shell) => shellLabel(shell, language)).join(", ");
  if (status.detected_shell) return `${shellLabel(status.detected_shell, language)} (${detectionSourceLabel(status.detection_source, language)})`;
  return t(language, "status.noneDetected");
}

/** Localized labels for the controlled detection-source values. */
export function detectionSourceLabel(source: string, language: Language = DEFAULT_LANGUAGE): string {
  switch (source) {
    case "explicit": return t(language, "detection.explicit");
    case "parent": return t(language, "detection.parent");
    case "shell_env": return t(language, "detection.shellEnv");
    case "login_shell": return t(language, "detection.loginShell");
    case "installed": return t(language, "detection.installed");
    default: return source;
  }
}

/** Invalidate data-consuming pages after an import or uninstall. */
export function nextDataVersion(current: number): number {
  return current + 1;
}

export interface AliasFilterState {
  query: string;
  fuzzy: boolean;
  tags: string[];
}

export function clearAliasFilters(): AliasFilterState {
  return { query: "", fuzzy: false, tags: [] };
}

export function aliasFiltersActive(query: string, fuzzy: boolean, tags: string[]): boolean {
  return query.trim().length > 0 || fuzzy || tags.length > 0;
}

export function toggleTag(selected: string[], tag: string): string[] {
  return selected.includes(tag) ? selected.filter((value) => value !== tag) : [...selected, tag];
}

export function emptyAliasDraft(): AliasDto {
  return {
    id: NEW_ALIAS_ID,
    name: "",
    description: "",
    executable: "",
    target_type: "native_executable",
    fixed_args: [],
    pass_args: true,
    working_directory: null,
    environment: {},
    shells: ["bash"],
    enabled: true,
    tags: [],
    restriction: null,
    revision: 1,
    path_mode: "absolute",
    path_origin: "gui_file_picker",
  };
}

/** Build the payload for `create_alias`; the Rust layer replaces the placeholder id. */
export function createAliasPayload(draft: AliasDto): AliasDto {
  return { ...finalizeAliasDraft(draft), id: NEW_ALIAS_ID };
}

/**
 * Merge a wizard edit back onto the original record.
 *
 * Identity and path provenance must come from the stored record: losing
 * `path_mode` would silently turn a relative-path alias into an absolute one,
 * and losing `revision` breaks optimistic concurrency.
 */
export function mergeEditedAlias(original: AliasDto, edited: AliasDto): AliasDto {
  return {
    ...edited,
    id: original.id,
    revision: original.revision,
    path_mode: original.path_mode ?? "absolute",
    path_origin: original.path_origin ?? "gui_file_picker",
    tags: normalizeTags(edited.tags),
    restriction: normalizeRestriction(edited.restriction),
  };
}

export function toggleAliasEnabled(alias: AliasDto): AliasDto {
  return { ...alias, enabled: !alias.enabled };
}

export function usesFilteredSearch(query: string, tagFilter: string[]): boolean {
  return query.trim().length > 0 || tagFilter.length > 0;
}

export function searchAliasesArgs(query: string, fuzzy: boolean, limit: number, tagFilter: string[]) {
  return { request: { query: query.trim(), fuzzy, limit, tag_filter: tagFilter } };
}

export let tauriInvoke: (command: string, args?: Record<string, unknown>) => Promise<unknown> = (command, args) => {
  const tauri = (globalThis as { __TAURI__?: { core?: { invoke?: (name: string, args?: Record<string, unknown>) => Promise<unknown> } } }).__TAURI__;
  if (tauri?.core?.invoke) return tauri.core.invoke(command, args);
  return import("@tauri-apps/api/core").then((module) => module.invoke(command, args));
};

export function setTauriInvoke(invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>): void {
  tauriInvoke = invoke;
}

// ── GUI phase helpers ──────────────────────────────────────────────
// Pure, dependency-free helpers shared by the wizard and panels. They are
// covered by src/gui.test.ts and keep backend-shape handling defensive.

export const SHELLS = ["bash", "zsh", "powershell5", "powershell7"] as const;

export type ShellName = (typeof SHELLS)[number];

export interface TargetTypeOption {
  value: string;
  labelKey: TranslationKey;
}

/** Target types offered by the wizard. Raw shell commands are intentionally excluded. */
export const TARGET_TYPES: TargetTypeOption[] = [
  { value: "native_executable", labelKey: "target.nativeExecutable" },
  { value: "python_script", labelKey: "target.python" },
  { value: "powershell_script", labelKey: "target.powershell" },
  { value: "batch", labelKey: "target.batch" },
  { value: "shell_script", labelKey: "target.shellScript" },
  { value: "java_jar", labelKey: "target.javaJar" },
  { value: "cmdlet", labelKey: "target.cmdlet" },
  { value: "change_directory", labelKey: "target.changeDirectory" },
];

export function fixedArgsPlaceholderHint(language: Language = DEFAULT_LANGUAGE): string {
  return t(language, "wizard.fixedArgsHint");
}

/** Explanation shown next to the wizard's target-type selector. */
export function targetTypeExplanation(language: Language = DEFAULT_LANGUAGE): string {
  return t(language, "wizard.targetTypeHint");
}

function looksLikePath(executable: string): boolean {
  return /[\\/]/.test(executable);
}

/**
 * Warn when a target contains whitespace that probably belongs in fixed
 * arguments. This is advisory only: the draft is never rewritten, so existing
 * records that intentionally use spaces keep working.
 *
 * `native_executable` with a real path (contains a separator) is accepted; a
 * bare `python test.py` is flagged because the executor treats it as one
 * program name.
 */
export function targetExecutableWarning(draft: AliasDto, language: Language = DEFAULT_LANGUAGE): string | null {
  const executable = draft.executable.trim();
  if (!/\s/.test(executable)) return null;
  if (draft.target_type === "batch") return null;
  if (draft.target_type === "native_executable") {
    if (looksLikePath(executable)) return null;
    const [program, ...rest] = executable.split(/\s+/);
    if (rest.length === 0) return null;
    return t(language, "wizard.targetWhitespaceNative", {
      program,
      rest: rest.join(" "),
    });
  }
  return t(language, "wizard.targetWhitespaceScript");
}

export type DoctorAction = "sync" | "install_loader";

const DOCTOR_SYNC_CODES: ReadonlySet<string> = new Set([
  "generated_missing",
  "generated_unreadable",
  "generated_modified",
  "render_failed",
  "stale",
  "sync_failed",
]);

/** Which actionable button (if any) a Doctor finding offers. */
export function doctorActionFor(finding: DoctorFinding): DoctorAction | null {
  if (finding.code === "loader_missing") {
    return finding.shell ? "install_loader" : null;
  }
  if (DOCTOR_SYNC_CODES.has(finding.code)) return "sync";
  return null;
}

export function doctorActionLabel(action: DoctorAction, language: Language = DEFAULT_LANGUAGE): string {
  return action === "sync"
    ? t(language, "doctor.actionSync")
    : t(language, "doctor.actionInstallLoader");
}

/** Argument envelope for the no-argument `sync_aliases` command. */
export function syncAliasesArgs(): Record<string, never> {
  return {};
}

/** Argument envelope for `shell_install`. */
export function shellInstallArgs(shell: string): { shell: string } {
  return { shell };
}

export interface SyncShellResult {
  shell: string;
  status: string;
  error: string | null;
  generated_path: string;
  reload_command: string;
  loader_installed: boolean;
}

export interface SyncRun {
  revision: number;
  failed: boolean;
  results: SyncShellResult[];
}

/** Defensive adapter for the `sync_aliases` response. */
export function normalizeSyncRun(value: unknown): SyncRun {
  const source = isRecord(value) ? value : {};
  const results = Array.isArray(source.results)
    ? source.results.filter(isRecord).map((entry) => ({
        shell: normalizeShellValue(entry.shell),
        status: typeof entry.status === "string" ? entry.status : "unknown",
        error: textOrNull(entry.error),
        generated_path: typeof entry.generated_path === "string" ? entry.generated_path : "",
        reload_command: typeof entry.reload_command === "string" ? entry.reload_command : "",
        loader_installed: entry.loader_installed === true,
      }))
    : [];
  return {
    revision: typeof source.revision === "number" && Number.isFinite(source.revision) ? source.revision : 0,
    failed: source.failed === true || results.some((result) => result.status === "failed"),
    results,
  };
}

function isPowerShell(shell: string): boolean {
  const normalized = normalizeShellValue(shell).toLowerCase();
  return normalized === "powershell5" || normalized === "powershell7" || normalized === "power_shell5" || normalized === "power_shell7";
}

/** Heading for the reload command; makes dot-sourcing explicit for PowerShell. */
export function reloadCommandLabel(shell: string, language: Language = DEFAULT_LANGUAGE): string {
  return isPowerShell(shell)
    ? t(language, "generated.reloadTitlePowerShell")
    : t(language, "generated.reloadTitlePosix");
}

/** Full reload explanation including the shell-specific operator meaning. */
export function reloadCommandExplanation(shell: string, language: Language = DEFAULT_LANGUAGE): string {
  const base = t(language, "generated.reloadExplanation");
  const specific = isPowerShell(shell)
    ? t(language, "generated.reloadExplanationPowerShell")
    : t(language, "generated.reloadExplanationPosix");
  return `${base} ${specific}`;
}

/** Short hover text for reload affordances. */
export function reloadHover(language: Language = DEFAULT_LANGUAGE): string {
  return t(language, "generated.reloadHover");
}

/**
 * Map a backend generated-preview error code (`code:shell`) to localized,
 * actionable copy. Raw OS text ("os error 3") is never the primary message.
 */
export function generatedPreviewErrorMessage(reason: unknown, language: Language = DEFAULT_LANGUAGE): string {
  const text = typeof reason === "string" ? reason : String(reason);
  const separator = text.indexOf(":");
  const code = separator >= 0 ? text.slice(0, separator) : text;
  const shell = separator >= 0 ? text.slice(separator + 1) : "";
  const label = shell ? shellLabel(shell, language) : "";
  switch (code) {
    case "generated_missing":
      return t(language, "generated.missingFile", { shell: label });
    case "generated_unreadable":
      return t(language, "generated.unreadableFile", { shell: label });
    case "generated_permission":
      return t(language, "generated.permissionFile", { shell: label });
    case "generated_unsupported_shell":
      return t(language, "generated.unsupportedShell");
    case "generated_read_error":
      return t(language, "generated.readError", { shell: label });
    default:
      return text;
  }
}

/** Copy-paste command that points a new process at another config directory. */
export function configDirExampleCommand(platform: PathPlatform = detectPathPlatform()): string {
  return platform === "windows"
    ? 'setx ALIASMGR_CONFIG_DIR "D:\\alias-manager"'
    : 'export ALIASMGR_CONFIG_DIR="/home/user/.alias-manager"';
}

export function configDirPlatformLabel(platform: PathPlatform, language: Language = DEFAULT_LANGUAGE): string {
  return platform === "windows"
    ? t(language, "settings.configDirPlatformWindows")
    : t(language, "settings.configDirPlatformPosix");
}

/** Full config-directory explanation, also used as the field's hover text. */
export function configDirExplanation(language: Language = DEFAULT_LANGUAGE): string {
  return `${t(language, "settings.configDirHintBefore")} ALIASMGR_CONFIG_DIR=<path> ${t(language, "settings.configDirHintAfter")}`;
}

export function manualOnlyBoundary(language: Language = DEFAULT_LANGUAGE): string {
  return t(language, "doctor.manualBoundary");
}

const DEFAULT_BACKUP_KEEP = 10;
const DEFAULT_LOG_KEEP = 10;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function textOrNull(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

/** Strip JSON quoting from persisted shell values such as `"bash"`. */
export function normalizeShellValue(value: unknown): string {
  if (typeof value !== "string") return "";
  let trimmed = value.trim();
  if (trimmed.length >= 2 && trimmed.startsWith('"') && trimmed.endsWith('"')) {
    try {
      const parsed: unknown = JSON.parse(trimmed);
      if (typeof parsed === "string") return parsed.trim();
    } catch {
      // Fall through to a plain quote strip for malformed JSON strings.
    }
    trimmed = trimmed.slice(1, -1).trim();
  }
  return trimmed;
}

export function shellLabel(shell: string, language: Language = DEFAULT_LANGUAGE): string {
  const normalized = normalizeShellValue(shell);
  switch (normalized.toLowerCase()) {
    case "bash": return t(language, "shell.bash");
    case "zsh": return t(language, "shell.zsh");
    case "powershell5":
    case "power_shell5": return t(language, "shell.powershell5");
    case "powershell7":
    case "power_shell7": return t(language, "shell.powershell7");
    case "fish": return t(language, "shell.fish");
    case "posix_sh": return t(language, "shell.posixSh");
    default: return normalized || t(language, "shell.unknown");
  }
}

export function targetTypeLabel(value: string, language: Language = DEFAULT_LANGUAGE): string {
  if (value === "raw_shell_command") return t(language, "target.rawShellCommand");
  const option = TARGET_TYPES.find((entry) => entry.value === value);
  return option ? t(language, option.labelKey) : (value || t(language, "target.unknown"));
}

export function pathModeLabel(mode?: string | null, language: Language = DEFAULT_LANGUAGE): string {
  switch (mode) {
    case "absolute": return t(language, "path.absolute");
    case "relative_to_working_directory": return t(language, "path.relativeWorkingDirectory");
    case "relative_to_path": return t(language, "path.relativePath");
    case null:
    case undefined:
    case "": return t(language, "path.notRecorded");
    default: return mode;
  }
}

/** Localized labels for controlled path-origin values; unknown values stay raw. */
export function pathOriginLabel(origin?: string | null, language: Language = DEFAULT_LANGUAGE): string {
  switch (origin) {
    case "gui_file_picker": return t(language, "pathOrigin.guiFilePicker");
    case "cli_argument": return t(language, "pathOrigin.cliArgument");
    case "import": return t(language, "pathOrigin.import");
    case null:
    case undefined:
    case "": return t(language, "path.notRecorded");
    default: return origin;
  }
}

/** Localized labels for restriction modes; unknown values stay raw. */
export function restrictionModeLabel(mode?: string | null, language: Language = DEFAULT_LANGUAGE): string {
  switch (mode) {
    case "whitelist": return t(language, "restriction.mode.whitelist");
    case "blacklist": return t(language, "restriction.mode.blacklist");
    default: return mode || t(language, "common.none");
  }
}

/** Localized severity badge text; unknown severities fall back to the backend value. */
export function severityLabel(severity: string, language: Language = DEFAULT_LANGUAGE): string {
  const key = `severity.${severity}`;
  return hasTranslation(key) ? t(language, key) : severity;
}

/** Localized definition-kind text for override metadata; unknown kinds stay raw. */
export function definitionKindLabel(kind: string, language: Language = DEFAULT_LANGUAGE): string {
  switch (kind) {
    case "function": return t(language, "override.kindFunction");
    case "alias": return t(language, "override.kindAlias");
    case "set_alias": return t(language, "override.kindSetAlias");
    default: return kind;
  }
}

export function toggleShell(shells: string[], shell: string): string[] {
  return shells.includes(shell) ? shells.filter((value) => value !== shell) : [...shells, shell];
}

export function normalizeShells(shells: string[]): string[] {
  return [...new Set(shells.map((value) => value.trim()).filter(Boolean))];
}

/** Trim only blank rows; keep each remaining row as one exact argument. */
export function normalizeFixedArgs(rows: string[]): string[] {
  return rows.filter((row) => row.trim().length > 0);
}

export interface EnvRow {
  key: string;
  value: string;
}

export function environmentToRows(environment: Record<string, string>): EnvRow[] {
  return Object.entries(environment ?? {}).map(([key, value]) => ({ key, value: String(value) }));
}

export function rowsToEnvironment(rows: EnvRow[]): Record<string, string> {
  const result: Record<string, string> = {};
  for (const row of rows) {
    const key = row.key.trim();
    if (!key) continue;
    result[key] = row.value;
  }
  return result;
}

export function finalizeAliasDraft(draft: AliasDto): AliasDto {
  return {
    ...draft,
    tags: normalizeTags(draft.tags),
    shells: normalizeShells(draft.shells),
    fixed_args: normalizeFixedArgs(draft.fixed_args),
    restriction: normalizeRestriction(draft.restriction),
  };
}

/** Trim restriction paths and drop blanks; `null` means no restriction. */
export function normalizeRestriction(
  restriction: PathRestriction | null | undefined,
): PathRestriction | null {
  if (!restriction) return null;
  const paths = [...new Set(restriction.paths.map((path) => path.trim()).filter(Boolean))];
  return { mode: restriction.mode, paths };
}

/**
 * Fold a path typed into the wizard input but not yet confirmed with Enter or
 * the Add button into the draft, so saving or previewing never silently drops
 * it.
 */
export function foldRestrictionPath(draft: AliasDto, pending: string): AliasDto {
  const path = pending.trim();
  if (!path || !draft.restriction || draft.restriction.paths.includes(path)) {
    return draft;
  }
  return {
    ...draft,
    restriction: { ...draft.restriction, paths: [...draft.restriction.paths, path] },
  };
}

export type PathPlatform = "windows" | "posix";

/** Best-effort host platform detection for path-form validation. */
export function detectPathPlatform(): PathPlatform {
  const nav = (globalThis as {
    navigator?: { platform?: string; userAgent?: string };
  }).navigator;
  const value = `${nav?.platform ?? ""} ${nav?.userAgent ?? ""}`;
  return /win/i.test(value) ? "windows" : "posix";
}

/**
 * Absolute path check used by the wizard; existence is not required. The
 * platform argument mirrors Rust's `Path::is_absolute`, so a Windows host
 * rejects POSIX paths and vice versa.
 */
export function isAbsolutePath(
  value: string,
  platform: PathPlatform = detectPathPlatform(),
): boolean {
  if (platform === "windows") {
    return /^[A-Za-z]:[\\/]/.test(value) || value.startsWith("\\\\");
  }
  return value.startsWith("/");
}

export function validateAliasDraft(draft: AliasDto, language: Language = DEFAULT_LANGUAGE): string[] {
  const errors: string[] = [];
  if (!/^[A-Za-z_][A-Za-z0-9_-]{0,63}$/.test(draft.name)) {
    errors.push(t(language, "validation.name"));
  }
  if (!draft.executable.trim()) errors.push(t(language, "validation.executable"));
  if (normalizeShells(draft.shells).length === 0) errors.push(t(language, "validation.shells"));
  if (!draft.target_type) errors.push(t(language, "validation.targetType"));
  if (draft.target_type === "raw_shell_command") errors.push(t(language, "validation.rawShell"));
  const restriction = normalizeRestriction(draft.restriction);
  if (restriction) {
    if (restriction.paths.length === 0) {
      errors.push(t(language, "validation.restrictionPaths"));
    } else if (restriction.paths.some((path) => !isAbsolutePath(path))) {
      errors.push(t(language, "validation.restrictionAbsolute"));
    }
  }
  return errors;
}

export function previewAliasArgs(alias: AliasDto): { alias: AliasDto } {
  return { alias: finalizeAliasDraft(alias) };
}

export interface PreviewSnippet {
  shell: string;
  content: string;
  error: string | null;
}

export function normalizePreviewSnippets(value: unknown): PreviewSnippet[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isRecord).map((entry) => ({
    shell: typeof entry.shell === "string" ? entry.shell : "",
    content: typeof entry.content === "string" ? entry.content : "",
    error: textOrNull(entry.error),
  }));
}

/** Render only the response that belongs to the newest preview request. */
export function latestPreviewResponse(currentToken: number, responseToken: number, snippets: PreviewSnippet[]): PreviewSnippet[] | null {
  return currentToken === responseToken ? snippets : null;
}

export interface ImportReport {
  imported: string[];
  skipped: string[];
  warnings: string[];
  unsupported: string[];
}

export function toStringList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((item): item is string => typeof item === "string" && item.length > 0);
}

export function normalizeImportReport(value: unknown): ImportReport {
  const source = isRecord(value) ? value : {};
  return {
    imported: toStringList(source.imported),
    skipped: toStringList(source.skipped),
    warnings: toStringList(source.warnings),
    unsupported: toStringList(source.unsupported),
  };
}

export function importReportCounts(report: ImportReport): { imported: number; skipped: number; warnings: number; unsupported: number } {
  return {
    imported: report.imported.length,
    skipped: report.skipped.length,
    warnings: report.warnings.length,
    unsupported: report.unsupported.length,
  };
}

export interface ImportSkipEntry {
  name: string;
  reason: string | null;
}

/**
 * Pair skipped names with their reason. Backends sometimes fold the reason
 * into the skipped item ("name: reason") and sometimes emit it separately in
 * `warnings`; support both without inventing a reason when none exists.
 */
function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Match a backend warning that names the record it belongs to. Comparing the
 * whole prefix + name (instead of substring matching) prevents a short name
 * such as `gs` from consuming a warning that belongs to `gst`.
 */
function importWarningReasonForName(warning: string, name: string, language: Language): string | null {
  const conflict = "name conflict skipped: ";
  if (warning === `${conflict}${name}`) return t(language, "import.skipNameConflict");
  const invalid = "invalid alias rejected: ";
  if (warning === `${invalid}${name}`) return t(language, "import.skipInvalidAlias");
  if (warning.startsWith(`${invalid}${name} (`) && warning.endsWith(")")) {
    // The parenthesized detail is backend text; keep it as the fallback.
    return warning.slice(`${invalid}${name} (`.length, -1).trim() || t(language, "import.skipInvalidAlias");
  }
  const relative = "relative path requires a selected base directory: ";
  if (warning === `${relative}${name}`) return t(language, "import.skipRelativeBase");
  return null;
}

export function importSkipEntries(report: ImportReport, language: Language = DEFAULT_LANGUAGE): ImportSkipEntry[] {
  const warnings = report.warnings.map((warning) => warning.trim()).filter(Boolean);
  const used = new Set<number>();
  return report.skipped.map((value) => {
    const trimmed = value.trim();
    let name = trimmed;
    let inlineReason: string | null = null;
    const separator = trimmed.indexOf(": ");
    if (separator > 0) {
      name = trimmed.slice(0, separator).trim();
      inlineReason = trimmed.slice(separator + 2).trim() || null;
    }
    if (inlineReason) return { name, reason: inlineReason };
    let index = warnings.findIndex((warning, position) => (
      !used.has(position) && importWarningReasonForName(warning, name, language) !== null
    ));
    if (index < 0) {
      const matcher = new RegExp(`(^|[^A-Za-z0-9_-])${escapeRegExp(name)}([^A-Za-z0-9_-]|$)`, "i");
      index = warnings.findIndex((warning, position) => !used.has(position) && matcher.test(warning));
    }
    if (index >= 0) {
      used.add(index);
      return { name, reason: importWarningReasonForName(warnings[index], name, language) ?? warnings[index] };
    }
    return { name, reason: null };
  });
}

export function isImportableFilename(filename: string): boolean {
  return /\.(json|toml)$/i.test(filename.trim());
}

export function importContentArgs(content: string, filename: string): { content: string; filename: string } {
  return { content, filename };
}

export interface DoctorFinding {
  shell: string | null;
  severity: string;
  code: string;
  message: string;
  action: string | null;
  detail: string | null;
}

export function normalizeDoctorFindings(value: unknown): DoctorFinding[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isRecord).map((entry) => ({
    shell: textOrNull(entry.shell),
    severity: typeof entry.severity === "string" && entry.severity ? entry.severity.toLowerCase() : "info",
    code: typeof entry.code === "string" ? entry.code : "",
    message: typeof entry.message === "string" ? entry.message : "",
    action: textOrNull(entry.action),
    detail: textOrNull(entry.detail),
  }));
}

export function severityCounts(findings: DoctorFinding[]): Record<string, number> {
  return findings.reduce<Record<string, number>>((counts, finding) => {
    counts[finding.severity] = (counts[finding.severity] ?? 0) + 1;
    return counts;
  }, {});
}

function severityRank(severity: string): number {
  switch (severity) {
    case "error": return 0;
    case "warning": return 1;
    case "info": return 2;
    default: return 3;
  }
}

export function orderedSeverities(counts: Record<string, number>): string[] {
  return Object.keys(counts).sort((left, right) => severityRank(left) - severityRank(right) || left.localeCompare(right));
}

export interface DoctorFindingText {
  title: string;
  message: string;
  action: string | null;
}

/**
 * Prefer local copy for known diagnostic codes in both languages; unknown
 * codes fall back to the backend text (which may already be localized).
 */
export function findingCodeLabel(code: string, language: Language = DEFAULT_LANGUAGE): string {
  const titleKey = `doctor.code.${code}.title`;
  if (hasTranslation(titleKey)) return t(language, titleKey);
  return code || t(language, "doctor.findingFallback");
}

export function doctorFindingText(finding: DoctorFinding, language: Language = DEFAULT_LANGUAGE): DoctorFindingText {
  const titleKey = `doctor.code.${finding.code}.title`;
  if (hasTranslation(titleKey)) {
    const messageKey = `doctor.code.${finding.code}.message`;
    const actionKey = `doctor.code.${finding.code}.action`;
    return {
      title: t(language, titleKey),
      message: hasTranslation(messageKey) ? t(language, messageKey) : finding.message,
      action: hasTranslation(actionKey) ? t(language, actionKey) : finding.action,
    };
  }
  return {
    title: findingCodeLabel(finding.code, language),
    message: finding.message || t(language, "doctor.noMessage"),
    action: finding.action,
  };
}

export interface OverriddenDefinition {
  name: string;
  shell: string;
  definition_kind: string;
  recoverable: boolean;
  captured_at: string | null;
}

/**
 * Normalize override metadata into a privacy-safe shape. Captured definition
 * text (`original_definition` / `recovery_text` / `definition`) is discarded
 * and never reaches component state.
 */
export function normalizeOverriddenDefinitions(value: unknown): OverriddenDefinition[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isRecord).map((entry) => ({
    name: typeof entry.name === "string" ? entry.name : "",
    shell: normalizeShellValue(entry.shell),
    definition_kind: typeof entry.definition_kind === "string" && entry.definition_kind ? entry.definition_kind : "function",
    recoverable: entry.recoverable === true || entry.recoverable === 1 || entry.recoverable === "true",
    captured_at: textOrNull(entry.captured_at),
  }));
}

/**
 * Desensitized clipboard metadata for an overridden definition.
 * Never include the captured definition/script body.
 */
export function overrideMetadataText(entry: OverriddenDefinition, language: Language = DEFAULT_LANGUAGE): string {
  return [
    t(language, "override.header"),
    `${t(language, "override.name")}: ${entry.name || t(language, "override.unnamed")}`,
    `${t(language, "override.shell")}: ${entry.shell || t(language, "override.unknown")}`,
    `${t(language, "override.kind")}: ${definitionKindLabel(entry.definition_kind, language)}`,
    `${t(language, "override.recoverable")}: ${entry.recoverable ? t(language, "override.true") : t(language, "override.false")}`,
    `${t(language, "override.capturedAt")}: ${entry.captured_at ?? t(language, "override.unknown")}`,
  ].join("\n");
}

export function overrideSummary(entry: OverriddenDefinition, language: Language = DEFAULT_LANGUAGE): string {
  return `${entry.name || t(language, "override.unnamed")} (${entry.shell ? shellLabel(entry.shell, language) : t(language, "override.unknownShell")})`;
}

export interface Settings {
  config_directory: string;
  default_shell: string;
  backup_keep: number;
  log_keep: number;
  allow_relative_paths: boolean;
  language: Language;
}

function countOr(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? Math.floor(value) : fallback;
}

export function normalizeSettings(value: unknown): Settings {
  const source = isRecord(value) ? value : {};
  return {
    config_directory: typeof source.config_directory === "string" ? source.config_directory : "",
    default_shell: typeof source.default_shell === "string" && source.default_shell ? source.default_shell : "bash",
    backup_keep: countOr(source.backup_keep, DEFAULT_BACKUP_KEEP),
    log_keep: countOr(source.log_keep, DEFAULT_LOG_KEEP),
    allow_relative_paths: source.allow_relative_paths === true,
    language: normalizeLanguage(source.language),
  };
}

export function parseKeepCount(value: string, fallback = 0): number {
  const trimmed = value.trim();
  if (!trimmed) return fallback;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.floor(parsed) : fallback;
}

/** Preserve the previous value when a keep-count field is blank or invalid. */
export function parseKeepCountInput(value: string, previous: number): number {
  const trimmed = value.trim();
  if (!trimmed) return previous;
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed) || parsed < 0) return previous;
  return Math.floor(parsed);
}

export function settingsValidation(settings: Settings, language: Language = DEFAULT_LANGUAGE): string[] {
  const errors: string[] = [];
  if (!settings.config_directory.trim()) errors.push(t(language, "settings.configDirectoryRequired"));
  if (!Number.isInteger(settings.backup_keep) || settings.backup_keep < 0) errors.push(t(language, "settings.backupKeepInvalid"));
  if (!Number.isInteger(settings.log_keep) || settings.log_keep < 0) errors.push(t(language, "settings.logKeepInvalid"));
  if (!(SHELLS as readonly string[]).includes(settings.default_shell)) errors.push(t(language, "settings.defaultShellInvalid"));
  if (!isLanguage(settings.language)) errors.push(t(language, "settings.invalidLanguage"));
  return errors;
}

export function settingsDirty(loaded: Settings, draft: Settings): boolean {
  return loaded.config_directory !== draft.config_directory
    || loaded.default_shell !== draft.default_shell
    || loaded.backup_keep !== draft.backup_keep
    || loaded.log_keep !== draft.log_keep
    || loaded.allow_relative_paths !== draft.allow_relative_paths
    || loaded.language !== draft.language;
}

export interface UninstallRiskSummary {
  kept: string[];
  removed: string[];
  protected: string[];
}

export function uninstallRiskSummary(purge: boolean, language: Language = DEFAULT_LANGUAGE): UninstallRiskSummary {
  const protectedNotice = t(language, "uninstall.risk.protected");
  const database = t(language, "uninstall.risk.database");
  const generated = t(language, "uninstall.risk.generated");
  const loader = t(language, "uninstall.risk.loader");
  if (purge) {
    return {
      kept: [t(language, "uninstall.risk.referencedTargets"), t(language, "uninstall.risk.unrelatedProfile")],
      removed: [
        database,
        generated,
        loader,
        t(language, "uninstall.risk.shims"),
      ],
      protected: [protectedNotice],
    };
  }
  return {
    kept: [
      database,
      generated,
      t(language, "uninstall.risk.targetsAndProfile"),
    ],
    removed: [
      loader,
      t(language, "uninstall.risk.shimsPath"),
    ],
    protected: [protectedNotice],
  };
}

/** Local outcome copy; backend result strings are not rendered. */
export function uninstallOutcomeText(purge: boolean, language: Language = DEFAULT_LANGUAGE): string {
  return t(language, purge ? "uninstall.outcomePurge" : "uninstall.outcomeRetain");
}

export async function copyToClipboard(text: string, language: Language = DEFAULT_LANGUAGE): Promise<void> {
  if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) {
    throw new Error(t(language, "error.clipboardUnavailable"));
  }
  await navigator.clipboard.writeText(text);
}
