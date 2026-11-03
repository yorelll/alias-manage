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

/** True when the value contains a Windows or POSIX path separator. */
function looksLikePath(executable: string): boolean {
  return /[\\/]/.test(executable);
}

const EXECUTABLE_SUFFIX = /\.(exe|cmd|bat|ps1|py|sh|jar)$/i;

const KNOWN_INTERPRETERS: ReadonlySet<string> = new Set([
  "python",
  "python3",
  "py",
  "pwsh",
  "powershell",
  "bash",
  "sh",
  "zsh",
  "java",
  "node",
  "cmd",
]);

/**
 * Warn when a target contains whitespace that probably belongs in fixed
 * arguments. This is advisory only: the draft is never rewritten.
 *
 * A bare first token (no path separator) with any trailing tokens is a command
 * line: `python /srv/run.py`, `bash deploy.sh`, and `tool.exe C:\in.txt` all
 * warn even when the later tokens look like paths. A real path with spaces
 * (`C:\Program Files\Tool\tool.exe`) is accepted unless it also carries a
 * flag-like tail. Script-type paths with spaces are accepted; a single path is
 * not a command line. `change_directory` targets are paths, not command lines.
 */
export function targetExecutableWarning(draft: AliasDto, language: Language = DEFAULT_LANGUAGE): string | null {
  const executable = draft.executable.trim();
  if (!/\s/.test(executable)) return null;
  if (draft.target_type === "batch" || draft.target_type === "change_directory") return null;
  const parts = executable.split(/\s+/);
  const rest = parts.slice(1);
  const hasFlagTail = rest.some((token) => token.startsWith("-"));
  const restLooksLikePath = rest.some((token) => looksLikePath(token));
  const bareProgram = !looksLikePath(parts[0]);
  const firstBase = parts[0].split(/[\\/]/).pop()?.toLowerCase().replace(/\.exe$/, "") ?? "";
  const interpreterTail =
    rest.length > 0 && !restLooksLikePath && KNOWN_INTERPRETERS.has(firstBase);
  // The LAST extension-bearing token is the executable candidate. Earlier
  // extension-like directory segments (`Foo.exe Folder`, `python venv`) must
  // not be mistaken for an argument tail.
  let lastExecutableIndex = -1;
  for (let index = 0; index < parts.length; index += 1) {
    if (EXECUTABLE_SUFFIX.test(parts[index])) lastExecutableIndex = index;
  }
  const executableWithFollowing =
    lastExecutableIndex >= 0 && lastExecutableIndex < parts.length - 1;
  // A path whose only internal spaces are directory names and whose final
  // token is the executable is one path, not a command line.
  const singlePathWithSpaces =
    !bareProgram && !hasFlagTail && !interpreterTail && !executableWithFollowing;
  if (singlePathWithSpaces) return null;
  const suspicious =
    bareProgram || hasFlagTail || interpreterTail || executableWithFollowing || !restLooksLikePath;
  if (!suspicious) return null;
  if (draft.target_type === "native_executable") {
    return t(language, "wizard.targetWhitespaceNative", {
      program: parts[0],
      rest: rest.join(" "),
    });
  }
  return t(language, "wizard.targetWhitespaceScript");
}

/**
 * Literal preview using the same normalized draft and expansion rules as the
 * core executor: the executable is always one element, `{{args}}` expands user
 * arguments in place, `{{{{args}}}}` is a literal `{{args}}`, and user
 * arguments are appended when `pass_args` is true and no placeholder exists.
 *
 * `change_directory` never runs the executable; it changes directory, so the
 * preview is `cd` plus the effective directory. An empty target has no argv.
 */
export function aliasArgvPreview(
  draft: AliasDto,
  userArgsLabel = "<user args>",
): string[] {
  const normalized = finalizeAliasDraft(draft);
  if (!normalized.executable.trim()) return [];
  if (normalized.target_type === "change_directory") {
    return [
      "cd",
      (normalized.working_directory ?? normalized.executable).trim(),
    ];
  }
  const argv = [normalized.executable];
  for (const argument of normalized.fixed_args) {
    if (argument === "{{args}}") argv.push(userArgsLabel);
    else if (argument === "{{{{args}}}}") argv.push("{{args}}");
    else argv.push(argument);
  }
  if (normalized.pass_args && !normalized.fixed_args.includes("{{args}}")) {
    argv.push(userArgsLabel);
  }
  return argv;
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
  error_code: string | null;
  generated_path: string;
  reload_command: string;
  loader_installed: boolean;
  loader_error_code: string | null;
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
        error_code: textOrNull(entry.error_code),
        generated_path: typeof entry.generated_path === "string" ? entry.generated_path : "",
        reload_command: typeof entry.reload_command === "string" ? entry.reload_command : "",
        loader_installed: entry.loader_installed === true,
        loader_error_code: textOrNull(entry.loader_error_code),
      }))
    : [];
  return {
    revision: typeof source.revision === "number" && Number.isFinite(source.revision) ? source.revision : 0,
    failed: source.failed === true || results.some((result) => result.status === "failed"),
    results,
  };
}

// ── Shell integration (R2.0 first-run, settings, preflight, auto-sync) ──

export interface ShellStatus {
  shell: string;
  detected: boolean;
  executable: string | null;
  /** manual | path | known_path | wsl | not_found */
  source: string;
  version: string | null;
  wsl: boolean;
  enabled: boolean;
  /** installed | missing | unavailable | error */
  loader_status: string;
  loader_target: string | null;
  loader_error: string | null;
  loader_error_code: string | null;
  /** enable | install_loader | needs_path | wsl_manual */
  action: string;
}

export function normalizeShellStatus(value: unknown): ShellStatus | null {
  if (!isRecord(value) || typeof value.shell !== "string" || !value.shell) return null;
  return {
    shell: normalizeShellValue(value.shell),
    detected: value.detected === true,
    executable: textOrNull(value.executable),
    source: typeof value.source === "string" ? value.source : "not_found",
    version: textOrNull(value.version),
    wsl: value.wsl === true,
    enabled: value.enabled === true,
    loader_status: typeof value.loader_status === "string" ? value.loader_status : "missing",
    loader_target: textOrNull(value.loader_target),
    loader_error: textOrNull(value.loader_error),
    loader_error_code: textOrNull(value.loader_error_code),
    action: typeof value.action === "string" ? value.action : "needs_path",
  };
}

/** Normalize a shell list into the canonical four-shell order. */
export function normalizeShellStatuses(value: unknown): ShellStatus[] {
  if (!Array.isArray(value)) return [];
  const statuses = value
    .map(normalizeShellStatus)
    .filter((status): status is ShellStatus => status !== null);
  return [...SHELLS]
    .map((shell) => statuses.find((status) => status.shell === shell))
    .filter((status): status is ShellStatus => status !== undefined);
}

export interface ShellStartupState {
  firstRun: boolean;
  autoSync: boolean;
  shells: ShellStatus[];
  invalidEnabled: string[];
  newlyDetected: string[];
}

/** Derive the startup state defensively; missing flags fall back to the shell rows. */
export function normalizeShellStartupState(value: unknown): ShellStartupState {
  const source = isRecord(value) ? value : {};
  const shells = normalizeShellStatuses(source.shells);
  const listed = (input: unknown): string[] => toStringList(input).map(normalizeShellValue).filter(Boolean);
  const invalidEnabled = Array.isArray(source.invalid_enabled)
    ? listed(source.invalid_enabled)
    : shells
        .filter((status) => status.enabled && (!status.detected || status.loader_status !== "installed"))
        .map((status) => status.shell);
  const newlyDetected = Array.isArray(source.newly_detected)
    ? listed(source.newly_detected)
    : shells.filter((status) => status.detected && !status.enabled && !status.wsl).map((status) => status.shell);
  return {
    firstRun: source.first_run === true,
    autoSync: source.auto_sync !== false,
    shells,
    invalidEnabled,
    newlyDetected,
  };
}

export type ShellOptionState = "enabled" | "loader-invalid" | "disabled" | "not-installed" | "wsl";

/** User-facing state of one shell option in the wizard/settings list. */
export function shellOptionState(status: ShellStatus): ShellOptionState {
  if (status.wsl) return "wsl";
  if (!status.detected) return "not-installed";
  if (status.enabled) {
    return status.loader_status === "installed" ? "enabled" : "loader-invalid";
  }
  return "disabled";
}

export interface ShellPathOverride {
  executable?: string;
  profile?: string;
}

/**
 * Default wizard selection: shells the user already enabled or that already
 * have a valid loader. Merely detecting a shell never checks or enables it.
 */
export function defaultWizardSelection(shells: ShellStatus[]): string[] {
  return shells
    .filter((status) => !status.wsl && (status.enabled || status.loader_status === "installed"))
    .map((status) => status.shell);
}

export interface WizardReadiness {
  ready: boolean;
  /** Selected WSL rows cannot be set up from Windows. */
  blocked: string[];
  /** Selected rows that still need a manual executable/profile path. */
  needsPath: string[];
}

/**
 * A first-run preview/apply is only allowed when every selected shell either
 * has a detected executable and a resolvable profile, or the user supplied a
 * path override. WSL rows block the action instead of writing a Windows
 * profile for a Linux home.
 */
export function wizardReadiness(
  shells: ShellStatus[],
  selected: string[],
  overrides: Record<string, ShellPathOverride> = {},
): WizardReadiness {
  const blocked: string[] = [];
  const needsPath: string[] = [];
  for (const shell of selected) {
    const status = shells.find((entry) => entry.shell === shell);
    if (!status) continue;
    const override = overrides[shell] ?? {};
    const executable = Boolean(override.executable?.trim()) || status.detected;
    const profile = Boolean(override.profile?.trim()) || Boolean(status.loader_target);
    // A WSL row is only usable when the user supplies a native executable; a
    // bare WSL launcher must be integrated inside the distribution instead.
    if (status.wsl && !override.executable?.trim()) {
      blocked.push(shell);
      continue;
    }
    if (!executable || !profile) needsPath.push(shell);
  }
  return {
    ready: selected.length > 0 && blocked.length === 0 && needsPath.length === 0,
    blocked,
    needsPath,
  };
}

export interface ShellSetupPlan {
  shell: string;
  detected: boolean;
  executable: string | null;
  source: string;
  version: string | null;
  wsl: boolean;
  enabled: boolean;
  action: string;
  loaderStatus: string;
  loaderTarget: string | null;
  loaderError: string | null;
  previewTarget: string | null;
  loaderBlock: string | null;
  defaultProfile: string | null;
}

export function normalizeShellSetupPlan(value: unknown): ShellSetupPlan {
  const source = isRecord(value) ? value : {};
  return {
    shell: normalizeShellValue(source.shell),
    detected: source.detected === true,
    executable: textOrNull(source.executable),
    source: typeof source.source === "string" ? source.source : "not_found",
    version: textOrNull(source.version),
    wsl: source.wsl === true,
    enabled: source.enabled === true,
    action: typeof source.action === "string" ? source.action : "needs_path",
    loaderStatus: typeof source.loader_status === "string" ? source.loader_status : "missing",
    loaderTarget: textOrNull(source.loader_target),
    loaderError: textOrNull(source.loader_error),
    previewTarget: textOrNull(source.preview_target),
    loaderBlock: textOrNull(source.loader_block),
    defaultProfile: textOrNull(source.default_profile),
  };
}

export interface ShellSetupOutcome {
  shell: string;
  enabled: boolean;
  executable: string | null;
  loaderTarget: string | null;
  loaderChanged: boolean;
  loaderInstalled: boolean;
  loaderError: string | null;
  loaderErrorCode: string | null;
  sync: SyncRun | null;
  syncError: string | null;
  warnings: string[];
  reloadCommand: string | null;
}

export function normalizeShellSetupOutcome(value: unknown): ShellSetupOutcome {
  const source = isRecord(value) ? value : {};
  return {
    shell: normalizeShellValue(source.shell),
    enabled: source.enabled === true,
    executable: textOrNull(source.executable),
    loaderTarget: textOrNull(source.loader_target),
    loaderChanged: source.loader_changed === true,
    loaderInstalled: source.loader_installed === true,
    loaderError: textOrNull(source.loader_error),
    loaderErrorCode: textOrNull(source.loader_error_code),
    sync: source.sync ? normalizeSyncRun(source.sync) : null,
    syncError: textOrNull(source.sync_error),
    warnings: toStringList(source.warnings),
    reloadCommand: textOrNull(source.reload_command),
  };
}

export function setupOutcomeFailed(outcome: ShellSetupOutcome): boolean {
  return Boolean(outcome.loaderError || outcome.syncError || outcome.sync?.failed);
}

/** Argument envelope for `shell_setup_plan`/`shell_setup_apply`. */
export function shellSetupArgs(
  shell: string,
  override: ShellPathOverride = {},
): { shell: string; executable?: string; profile?: string } {
  const args: { shell: string; executable?: string; profile?: string } = { shell };
  const executable = override.executable?.trim();
  const profile = override.profile?.trim();
  if (executable) args.executable = executable;
  if (profile) args.profile = profile;
  return args;
}

export function shellEnableArgs(shells: string[]): { shells: string[] } {
  return { shells: normalizeShells(shells) };
}

/** Stable prefix parsing for setup rejections (`code: message`). */
export function setupErrorMessage(reason: unknown, language: Language = DEFAULT_LANGUAGE): string {
  const text = typeof reason === "string" ? reason : String(reason);
  const separator = text.indexOf(":");
  const code = separator >= 0 ? text.slice(0, separator) : text;
  switch (code) {
    case "setup_not_ready": return t(language, "shellSetup.errorNotReady");
    case "setup_unsupported": return t(language, "shellSetup.errorUnsupported");
    case "setup_config": return t(language, "shellSetup.errorConfig");
    case "setup_unsafe": return loaderErrorMessage("loader_unsafe", language);
    case "setup_permission": return t(language, "shellSetup.errorPermission");
    case "setup_missing": return t(language, "shellSetup.errorMissing");
    case "setup_io": return t(language, "shellSetup.errorIo");
    default: return text || t(language, "shellSetup.errorFailed");
  }
}

export interface AliasShellProblem {
  shell: string;
  /** shell_disabled | loader_missing | loader_* */
  code: string;
  message: string;
  loaderTarget: string | null;
}

export interface AliasPreflight {
  ok: boolean;
  problems: AliasShellProblem[];
}

export function normalizeAliasPreflight(value: unknown): AliasPreflight {
  const source = isRecord(value) ? value : {};
  const problems = Array.isArray(source.problems)
    ? source.problems.filter(isRecord).map((entry) => ({
        shell: normalizeShellValue(entry.shell),
        code: typeof entry.code === "string" ? entry.code : "unknown",
        message: typeof entry.message === "string" ? entry.message : "",
        loaderTarget: textOrNull(entry.loader_target),
      }))
    : [];
  return {
    ok: source.ok === true && problems.length === 0,
    problems,
  };
}

/** Localized explanation for one preflight problem; unknown codes stay raw. */
export function preflightProblemText(problem: AliasShellProblem, language: Language = DEFAULT_LANGUAGE): string {
  const label = shellLabel(problem.shell, language);
  switch (problem.code) {
    case "shell_disabled": return t(language, "preflight.shellDisabled", { shell: label });
    case "loader_missing": return t(language, "preflight.loaderMissing", { shell: label });
    default:
      if (problem.code.startsWith("loader_")) {
        return t(language, "preflight.loaderInvalid", {
          shell: label,
          reason: loaderErrorMessage(problem.code, language),
        });
      }
      return problem.message || t(language, "preflight.unknown", { shell: label });
  }
}

export function preflightMessage(preflight: AliasPreflight, language: Language = DEFAULT_LANGUAGE): string {
  if (preflight.ok) return "";
  return `${t(language, "preflight.blocked")} ${preflight.problems.map((problem) => preflightProblemText(problem, language)).join(" ")}`;
}

export interface AutoSync {
  attempted: boolean;
  skippedReason: string | null;
  run: SyncRun | null;
  error: string | null;
  errorCode: string | null;
}

export function normalizeAutoSync(value: unknown): AutoSync {
  const source = isRecord(value) ? value : {};
  return {
    attempted: source.attempted === true,
    skippedReason: textOrNull(source.skipped_reason),
    run: source.run ? normalizeSyncRun(source.run) : null,
    error: textOrNull(source.error),
    errorCode: textOrNull(source.error_code),
  };
}

export interface AutoSyncNotice {
  tone: "ok" | "warning" | "none";
  text: string;
  /** Reload commands for the current session; new terminals use the loader. */
  reloadCommands: string[];
}

/** Render an auto-sync result; failures are never hidden behind a success. */
export function autoSyncNotice(autoSync: AutoSync, language: Language = DEFAULT_LANGUAGE): AutoSyncNotice {
  if (!autoSync.attempted) {
    return { tone: "none", text: "", reloadCommands: [] };
  }
  const run = autoSync.run;
  const reloadCommands = run
    ? run.results
        .filter((result) => result.status !== "failed" && result.reload_command)
        .map((result) => `${shellLabel(result.shell, language)}: ${result.reload_command}`)
    : [];
  if (autoSync.error) {
    return {
      tone: "warning",
      text: t(language, "autoSync.error", { error: autoSync.error }),
      reloadCommands,
    };
  }
  if (!run) return { tone: "none", text: "", reloadCommands: [] };
  if (run.failed) {
    const failures = run.results
      .filter((result) => result.status === "failed")
      .map((result) => `${shellLabel(result.shell, language)}: ${syncFailureMessage(result.error_code, language)}`);
    return {
      tone: "warning",
      text: t(language, "autoSync.partial", { failures: failures.join("; ") }),
      reloadCommands,
    };
  }
  if (run.results.length === 0) return { tone: "none", text: "", reloadCommands: [] };
  return {
    tone: "ok",
    text: t(language, "autoSync.success", {
      count: run.results.length,
      revision: run.revision,
    }),
    reloadCommands,
  };
}

export function shellOptionStateLabel(state: ShellOptionState, language: Language = DEFAULT_LANGUAGE): string {
  switch (state) {
    case "enabled": return t(language, "shellSetup.status.enabled");
    case "loader-invalid": return t(language, "shellSetup.status.loaderInvalid");
    case "disabled": return t(language, "shellSetup.status.disabled");
    case "not-installed": return t(language, "shellSetup.status.notInstalled");
    case "wsl": return t(language, "shellSetup.status.wsl");
    default: return state;
  }
}

/** Localized loader-state text; unknown values stay raw. */
export function shellLoaderLabel(status: string, language: Language = DEFAULT_LANGUAGE): string {
  switch (status) {
    case "installed": return t(language, "shellSetup.loader.installed");
    case "missing": return t(language, "shellSetup.loader.missing");
    case "unavailable": return t(language, "shellSetup.loader.unavailable");
    case "error": return t(language, "shellSetup.loader.error");
    default: return status;
  }
}

/** Localized detection-source text; unknown values stay raw. */
export function shellSourceLabel(source: string, language: Language = DEFAULT_LANGUAGE): string {
  switch (source) {
    case "manual": return t(language, "shellSetup.source.manual");
    case "path": return t(language, "shellSetup.source.path");
    case "known_path": return t(language, "shellSetup.source.knownPath");
    case "wsl": return t(language, "shellSetup.source.wsl");
    case "not_found": return t(language, "shellSetup.source.notFound");
    default: return source;
  }
}

/** Per-step lines for a confirmed setup outcome (used by both setup UIs). */
export function shellSetupOutcomeLines(
  outcome: ShellSetupOutcome,
  language: Language = DEFAULT_LANGUAGE,
): string[] {
  const label = shellLabel(outcome.shell, language);
  const lines = [
    `${t(language, "shellSetup.resultConfig")}: ${t(language, "shellSetup.resultConfigSaved", { shell: label })}`,
  ];
  if (outcome.loaderError) {
    lines.push(
      `${t(language, "shellSetup.resultLoader")}: ${t(language, "shellSetup.loaderNotInstalled")} ${loaderErrorMessage(outcome.loaderErrorCode, language)}`,
    );
  } else if (outcome.loaderChanged) {
    lines.push(
      `${t(language, "shellSetup.resultLoader")}: ${t(language, "shellSetup.resultLoaderInstalled", { target: outcome.loaderTarget ?? "" })}`,
    );
  } else {
    lines.push(
      `${t(language, "shellSetup.resultLoader")}: ${t(language, "shellSetup.resultLoaderUnchanged", { target: outcome.loaderTarget ?? "" })}`,
    );
  }
  if (outcome.syncError) {
    lines.push(
      `${t(language, "shellSetup.resultSync")}: ${t(language, "shellSetup.resultSyncError", { error: outcome.syncError })}`,
    );
  } else if (outcome.sync) {
    const failures = outcome.sync.results
      .filter((result) => result.status === "failed")
      .map((result) => syncFailureMessage(result.error_code, language));
    lines.push(
      `${t(language, "shellSetup.resultSync")}: ${
        failures.length > 0
          ? t(language, "shellSetup.resultSyncFailed", { error: failures.join("; ") })
          : t(language, "shellSetup.resultSyncOk", { count: outcome.sync.results.length })
      }`,
    );
  }
  if (outcome.reloadCommand && !setupOutcomeFailed(outcome)) {
    lines.push(
      `${t(language, "shellSetup.resultReload")}: ${t(language, "shellSetup.reloadHint", { command: outcome.reloadCommand })}`,
    );
  }
  return lines;
}

export interface SaveAliasOutcome {
  error: string | null;
  preflight: AliasPreflight | null;
}

export interface SaveAliasResult {
  saved: boolean;
  alias: AliasDto | null;
  preflight: AliasPreflight;
  autoSync: AutoSync;
}

export function normalizeSaveAliasResult(value: unknown): SaveAliasResult {
  const source = isRecord(value) ? value : {};
  return {
    saved: source.saved === true,
    alias: isRecord(source.alias) ? (source.alias as unknown as AliasDto) : null,
    preflight: normalizeAliasPreflight(source.preflight),
    autoSync: normalizeAutoSync(source.auto_sync),
  };
}

export interface DeleteAliasResult {
  deleted: boolean;
  autoSync: AutoSync;
}

export function normalizeDeleteAliasResult(value: unknown): DeleteAliasResult {
  const source = isRecord(value) ? value : {};
  return {
    deleted: source.deleted === true,
    autoSync: normalizeAutoSync(source.auto_sync),
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

/** Localized headline for a per-shell sync failure code. */
export function syncFailureMessage(code: string | null | undefined, language: Language = DEFAULT_LANGUAGE): string {
  switch (code) {
    case "sync_permission": return t(language, "sync.error.permission");
    case "sync_missing": return t(language, "sync.error.missing");
    case "sync_io": return t(language, "sync.error.io");
    case "sync_failed": return t(language, "sync.error.render");
    default: return t(language, "sync.error.failed");
  }
}

/** Localized message for a sync command rejection (`sync_*` code). */
export function syncErrorMessage(reason: unknown, language: Language = DEFAULT_LANGUAGE): string {
  const code = typeof reason === "string" ? reason : String(reason);
  switch (code) {
    case "sync_permission": return t(language, "sync.error.permission");
    case "sync_missing": return t(language, "sync.error.missing");
    case "sync_io": return t(language, "sync.error.io");
    case "sync_config": return t(language, "sync.error.config");
    case "sync_database": return t(language, "sync.error.database");
    case "sync_locked": return t(language, "sync.error.locked");
    case "sync_unsafe": return t(language, "sync.error.unsafe");
    case "sync_failed": return t(language, "sync.error.failed");
    default: return t(language, "sync.error.failed");
  }
}

/** Localized message for a loader install rejection (`loader_*` code). */
export function loaderErrorMessage(reason: unknown, language: Language = DEFAULT_LANGUAGE): string {
  const code = typeof reason === "string" ? reason : String(reason);
  switch (code) {
    case "loader_permission": return t(language, "loader.error.permission");
    case "loader_missing": return t(language, "loader.error.missing");
    case "loader_io": return t(language, "loader.error.io");
    case "loader_unreadable": return t(language, "loader.error.unreadable");
    case "loader_unsafe": return t(language, "loader.error.unsafe");
    case "loader_unsupported": return t(language, "loader.error.unsupported");
    case "loader_config": return t(language, "loader.error.config");
    case "loader_shared_path": return t(language, "loader.error.sharedPath");
    default: return t(language, "loader.error.failed");
  }
}

/**
 * Uninstall failures may be loader-code failures or other backend errors.
 * Localize the loader codes and leave other messages visible.
 */
export function uninstallErrorMessage(reason: unknown, language: Language = DEFAULT_LANGUAGE): string {
  const text = typeof reason === "string" ? reason : String(reason);
  return text.startsWith("loader_") ? loaderErrorMessage(text, language) : text;
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
  /** Post-confirm sync outcome; preview leaves it unattempted. */
  auto_sync: AutoSync;
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
    auto_sync: normalizeAutoSync(source.auto_sync),
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
  auto_sync: boolean;
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
    auto_sync: source.auto_sync !== false,
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
    || loaded.language !== draft.language
    || loaded.auto_sync !== draft.auto_sync;
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
