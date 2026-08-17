import { useEffect, useState } from "react";
import { tauriInvoke } from "../lib";

type Settings = { config_directory: string; default_shell: string; backup_keep: number; log_keep: number; allow_relative_paths: boolean };

export function SettingsPanel() {
  const [draft, setDraft] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { tauriInvoke("config_get").then((value) => setDraft(value as Settings)).catch((reason) => setError(String(reason))); }, []);
  const save = async () => { if (!draft) return; try { setDraft(await tauriInvoke("config_save") as Settings); setError(null); } catch (reason) { setError(String(reason)); } };
  if (!draft) return <section className="panel"><h1>Settings</h1><p>{error ?? "Loading…"}</p></section>;
  return <section className="panel"><h1>Settings</h1><label>Config directory<input value={draft.config_directory} readOnly /></label><label>Default Shell<select value={draft.default_shell} onChange={(event) => setDraft({ ...draft, default_shell: event.target.value })}><option>bash</option><option>zsh</option><option>powershell5</option><option>powershell7</option></select></label><label>Backup keep<input type="number" value={draft.backup_keep} onChange={(event) => setDraft({ ...draft, backup_keep: Number(event.target.value) })} /></label><label>Log keep<input type="number" value={draft.log_keep} onChange={(event) => setDraft({ ...draft, log_keep: Number(event.target.value) })} /></label><label><input type="checkbox" checked={draft.allow_relative_paths} onChange={(event) => setDraft({ ...draft, allow_relative_paths: event.target.checked })} /> Allow relative paths</label>{error && <p role="alert">{error}</p>}<button onClick={save}>Save settings</button></section>;
}

export default SettingsPanel;
