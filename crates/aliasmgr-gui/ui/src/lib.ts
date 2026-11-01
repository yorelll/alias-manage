export interface StartupStatus {
  version: string;
  detected_shell: string | null;
  detection_source: string;
  config_directory: string;
}

export type Page = "aliases" | "sync" | "doctor" | "settings";

export const NAV_ITEMS = [
  { id: "aliases", label: "Aliases" },
  { id: "sync", label: "Sync" },
  { id: "doctor", label: "Doctor" },
  { id: "settings", label: "Settings" },
] as const;

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
  revision: number;
  path_mode?: string | null;
  path_origin?: string | null;
}

/** Placeholder id sent when creating an alias; the Rust layer assigns a real UUID. */
export const NEW_ALIAS_ID = "00000000-0000-0000-0000-000000000000";

export function normalizeTags(values: string[]): string[] { return [...new Set(values.map((value) => value.trim()).filter(Boolean))]; }
export function truncateDescription(full: string, max: number): { display: string; full: string; truncated: boolean } { return full.length <= max ? { display: full, full, truncated: false } : { display: full.slice(0, max), full, truncated: true }; }
export function statusSummaryText(status: StartupStatus | null, error: boolean): string {
  if (status) return `v${status.version} · ${status.detected_shell ?? "no shell detected"}`;
  if (error) return "status unavailable";
  return "loading…";
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
  label: string;
}

/** Target types offered by the wizard. Raw shell commands are intentionally excluded. */
export const TARGET_TYPES: TargetTypeOption[] = [
  { value: "native_executable", label: "Native executable" },
  { value: "python_script", label: "Python script" },
  { value: "powershell_script", label: "PowerShell script" },
  { value: "batch", label: "Batch" },
  { value: "shell_script", label: "Shell script" },
  { value: "java_jar", label: "Java JAR" },
  { value: "cmdlet", label: "Cmdlet" },
  { value: "change_directory", label: "Change directory" },
];

export const FIXED_ARGS_PLACEHOLDER_HINT =
  "Use {{args}} to insert user arguments at that position. Use {{{{args}}}} to pass the literal text {{args}}.";

export const MANUAL_ONLY_BOUNDARY =
  "Manual-only boundary: this panel only reports findings. Alias Manager never edits your shell profile, bypasses ExecutionPolicy, or restarts your shell. Copy a suggestion and apply it yourself.";

const DEFAULT_BACKUP_KEEP = 10;
const DEFAULT_LOG_KEEP = 10;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function textOrNull(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

export function shellLabel(shell: string): string {
  switch (shell) {
    case "bash": return "Bash";
    case "zsh": return "Zsh";
    case "powershell5":
    case "power_shell5": return "PowerShell 5.1";
    case "powershell7":
    case "power_shell7": return "PowerShell 7";
    case "fish": return "Fish";
    case "posix_sh": return "POSIX sh";
    default: return shell || "Unknown shell";
  }
}

export function targetTypeLabel(value: string): string {
  return TARGET_TYPES.find((option) => option.value === value)?.label ?? (value || "Unknown target type");
}

export function pathModeLabel(mode?: string | null): string {
  switch (mode) {
    case "absolute": return "Absolute";
    case "relative_to_working_directory": return "Relative to working directory";
    case "relative_to_path": return "Relative to PATH";
    case null:
    case undefined:
    case "": return "Not recorded";
    default: return mode;
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
  };
}

export function validateAliasDraft(draft: AliasDto): string[] {
  const errors: string[] = [];
  if (!/^[A-Za-z_][A-Za-z0-9_-]{0,63}$/.test(draft.name)) {
    errors.push("Name must start with a letter or underscore and use only letters, digits, underscores, and hyphens (max 64 characters).");
  }
  if (!draft.executable.trim()) errors.push("Target executable or command is required.");
  if (normalizeShells(draft.shells).length === 0) errors.push("Select at least one shell.");
  if (!draft.target_type) errors.push("Select a target type.");
  if (draft.target_type === "raw_shell_command") errors.push("Raw shell commands are not supported in this form.");
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

export interface OverriddenDefinition {
  name: string;
  shell: string;
  definition_kind: string;
  original_definition: string | null;
  recovery_text: string | null;
  definition: string | null;
  recoverable: boolean;
  captured_at: string | null;
}

export function normalizeOverriddenDefinitions(value: unknown): OverriddenDefinition[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isRecord).map((entry) => ({
    name: typeof entry.name === "string" && entry.name ? entry.name : "(unnamed)",
    shell: typeof entry.shell === "string" ? entry.shell : "",
    definition_kind: typeof entry.definition_kind === "string" && entry.definition_kind ? entry.definition_kind : "function",
    original_definition: textOrNull(entry.original_definition),
    recovery_text: textOrNull(entry.recovery_text),
    definition: textOrNull(entry.definition),
    recoverable: entry.recoverable === true || entry.recoverable === 1 || entry.recoverable === "true",
    captured_at: textOrNull(entry.captured_at),
  }));
}

/** Recovery text is data for the clipboard only. It is never executed by the UI. */
export function overrideRecoveryText(entry: OverriddenDefinition): string | null {
  return entry.recovery_text ?? entry.original_definition ?? entry.definition;
}

export function overrideSummary(entry: OverriddenDefinition): string {
  return `${entry.name} (${entry.shell ? shellLabel(entry.shell) : "unknown shell"})`;
}

export interface Settings {
  config_directory: string;
  default_shell: string;
  backup_keep: number;
  log_keep: number;
  allow_relative_paths: boolean;
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
  };
}

export function parseKeepCount(value: string, fallback = 0): number {
  const trimmed = value.trim();
  if (!trimmed) return fallback;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.floor(parsed) : fallback;
}

export function settingsValidation(settings: Settings): string[] {
  const errors: string[] = [];
  if (!settings.config_directory.trim()) errors.push("Config directory is required.");
  if (!Number.isInteger(settings.backup_keep) || settings.backup_keep < 0) errors.push("Backup keep must be a non-negative whole number.");
  if (!Number.isInteger(settings.log_keep) || settings.log_keep < 0) errors.push("Log keep must be a non-negative whole number.");
  if (!(SHELLS as readonly string[]).includes(settings.default_shell)) errors.push("Choose a supported default shell.");
  return errors;
}

export function settingsDirty(loaded: Settings, draft: Settings): boolean {
  return loaded.config_directory !== draft.config_directory
    || loaded.default_shell !== draft.default_shell
    || loaded.backup_keep !== draft.backup_keep
    || loaded.log_keep !== draft.log_keep
    || loaded.allow_relative_paths !== draft.allow_relative_paths;
}

export interface UninstallRiskSummary {
  kept: string[];
  removed: string[];
  protected: string[];
}

export function uninstallRiskSummary(purge: boolean): UninstallRiskSummary {
  if (purge) {
    return {
      kept: ["Referenced target files", "Unrelated shell profile content"],
      removed: [
        "Alias database and every alias record",
        "Generated shell definition files",
        "Alias Manager loader block from shell profiles",
        "Application shims and backup/journal files owned by Alias Manager",
      ],
      protected: ["Referenced target files are never deleted."],
    };
  }
  return {
    kept: [
      "Alias database and every alias record",
      "Generated shell definition files",
      "Referenced target files and unrelated profile content",
    ],
    removed: [
      "Alias Manager loader block from shell profiles",
      "Application shims and PATH entries added by Alias Manager",
    ],
    protected: ["Referenced target files are never deleted."],
  };
}

export async function copyToClipboard(text: string): Promise<void> {
  if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) {
    throw new Error("Clipboard is not available in this environment.");
  }
  await navigator.clipboard.writeText(text);
}
