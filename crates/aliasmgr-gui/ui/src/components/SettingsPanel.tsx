import { useEffect, useState } from "react";
import type { Settings } from "../lib";
import {
  SHELLS,
  normalizeSettings,
  parseKeepCount,
  settingsDirty,
  settingsValidation,
  shellLabel,
  tauriInvoke,
} from "../lib";

export function SettingsPanel() {
  const [loaded, setLoaded] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = async () => {
    setError(null);
    setSaved(null);
    try {
      const value = normalizeSettings(await tauriInvoke("config_get"));
      setLoaded(value);
      setDraft({ ...value });
    } catch (reason) {
      setError(String(reason));
    }
  };

  useEffect(() => { void load(); }, []);

  const edit = (patch: Partial<Settings>) => {
    setDraft((value) => (value ? { ...value, ...patch } : value));
    setSaved(null);
    setError(null);
  };

  const cancel = () => {
    if (!loaded) return;
    setDraft({ ...loaded });
    setError(null);
    setSaved(null);
  };

  const save = async () => {
    if (!draft || busy) return;
    const errors = settingsValidation(draft);
    if (errors.length > 0) {
      setSaved(null);
      setError(errors.join(" "));
      return;
    }
    setBusy(true);
    setSaved(null);
    setError(null);
    try {
      const stored = normalizeSettings(await tauriInvoke("config_save", { settings: draft }));
      setLoaded({ ...stored });
      setDraft({ ...stored });
      setSaved(`Settings saved for ${stored.config_directory}.`);
    } catch (reason) {
      // Keep the draft intact so the user can correct the path and retry.
      setSaved(null);
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  if (!draft) {
    return (
      <section className="panel">
        <h1>Settings</h1>
        <p>{error ? `Unable to load settings: ${error}` : "Loading…"}</p>
      </section>
    );
  }

  return (
    <section className="panel" aria-labelledby="settings-title">
      <h1 id="settings-title">Settings</h1>
      <p className="hint">Changes are applied only when you save. Cancel restores the last loaded values.</p>

      <div className="settings-grid">
        <label className="field field-full">
          <span>Config directory</span>
          <input
            value={draft.config_directory}
            onChange={(event) => edit({ config_directory: event.target.value })}
            autoComplete="off"
            spellCheck={false}
            aria-describedby="config-directory-hint"
          />
          <span id="config-directory-hint" className="hint">
            The directory must exist and be writable. Invalid or read-only paths are rejected on save.
          </span>
        </label>

        <label className="field">
          <span>Default shell</span>
          <select value={draft.default_shell} onChange={(event) => edit({ default_shell: event.target.value })}>
            {SHELLS.map((shell) => <option key={shell} value={shell}>{shellLabel(shell)}</option>)}
          </select>
        </label>

        <label className="field">
          <span>Backup keep</span>
          <input
            type="number"
            min={0}
            step={1}
            value={draft.backup_keep}
            onChange={(event) => edit({ backup_keep: parseKeepCount(event.target.value) })}
          />
        </label>

        <label className="field">
          <span>Log keep</span>
          <input
            type="number"
            min={0}
            step={1}
            value={draft.log_keep}
            onChange={(event) => edit({ log_keep: parseKeepCount(event.target.value) })}
          />
        </label>

        <label className="check-row field-full">
          <input
            type="checkbox"
            checked={draft.allow_relative_paths}
            onChange={(event) => edit({ allow_relative_paths: event.target.checked })}
          />
          <span>Allow relative paths</span>
        </label>
      </div>

      {loaded && settingsDirty(loaded, draft) && <p className="hint" role="status">Unsaved changes.</p>}
      {saved && <p role="status">{saved}</p>}
      {error && <p className="form-error" role="alert">{error}</p>}

      <div className="form-actions">
        <button type="button" onClick={cancel}>Cancel</button>
        <button type="button" onClick={() => void load()} disabled={busy}>Reload persisted values</button>
        <button type="button" className="btn-primary" onClick={save} disabled={busy}>
          {busy ? "Saving…" : "Save settings"}
        </button>
      </div>
    </section>
  );
}

export default SettingsPanel;
