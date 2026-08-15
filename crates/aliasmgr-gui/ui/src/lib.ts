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

export function statusSummaryText(status: StartupStatus | null, error: boolean): string {
  if (status) return `v${status.version} · ${status.detected_shell ?? "no shell detected"}`;
  if (error) return "status unavailable";
  return "loading…";
}

export let tauriInvoke: (command: string) => Promise<unknown> = (command) => {
  const tauri = (globalThis as { __TAURI__?: { core?: { invoke?: (name: string) => Promise<unknown> } } }).__TAURI__;
  if (tauri?.core?.invoke) return tauri.core.invoke(command);
  return import("@tauri-apps/api/core").then((module) => module.invoke(command));
};

export function setTauriInvoke(invoke: (command: string) => Promise<unknown>): void {
  tauriInvoke = invoke;
}
