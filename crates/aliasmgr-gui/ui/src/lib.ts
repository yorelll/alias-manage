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
  return { ...draft, id: NEW_ALIAS_ID, tags: normalizeTags(draft.tags) };
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
  return { request: { query, fuzzy, limit, tag_filter: tagFilter } };
}

export let tauriInvoke: (command: string, args?: Record<string, unknown>) => Promise<unknown> = (command, args) => {
  const tauri = (globalThis as { __TAURI__?: { core?: { invoke?: (name: string, args?: Record<string, unknown>) => Promise<unknown> } } }).__TAURI__;
  if (tauri?.core?.invoke) return tauri.core.invoke(command, args);
  return import("@tauri-apps/api/core").then((module) => module.invoke(command, args));
};

export function setTauriInvoke(invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>): void {
  tauriInvoke = invoke;
}
