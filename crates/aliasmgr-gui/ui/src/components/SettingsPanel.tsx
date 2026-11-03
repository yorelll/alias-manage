import { useEffect, useState } from "react";
import type { Settings, ShellStatus } from "../lib";
import {
  SHELLS,
  configDirExampleCommand,
  configDirExplanation,
  configDirPlatformLabel,
  copyToClipboard,
  detectPathPlatform,
  normalizeSettings,
  parseKeepCountInput,
  settingsDirty,
  settingsValidation,
  shellLabel,
  tauriInvoke,
} from "../lib";
import type { Language } from "../i18n";
import { LANGUAGES, isLanguage } from "../i18n";
import { useI18n } from "../language";
import { ShellSettingsPanel } from "./ShellSettingsPanel";

export function SettingsPanel({
  focusShellsToken = 0,
  onShellsChanged,
  shellStatuses = [],
  active = true,
}: {
  focusShellsToken?: number;
  onShellsChanged?: () => void;
  shellStatuses?: ShellStatus[];
  active?: boolean;
}) {
  const { language, setLanguage, t } = useI18n();
  const [loaded, setLoaded] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [platform] = useState(() => detectPathPlatform());
  const [exampleCopied, setExampleCopied] = useState<string | null>(null);
  const [exampleError, setExampleError] = useState<string | null>(null);
  const exampleCommand = configDirExampleCommand(platform);

  // Localized copy notices must not survive a language switch.
  useEffect(() => {
    setExampleCopied(null);
    setExampleError(null);
  }, [language]);

  const load = async () => {
    setError(null);
    setSaved(null);
    try {
      const value = normalizeSettings(await tauriInvoke("config_get"));
      setLoaded(value);
      setDraft({ ...value });
      setLanguage(value.language);
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

  const changeLanguage = (value: string) => {
    if (!isLanguage(value)) return;
    // Apply immediately to the whole UI; Cancel restores the loaded language.
    const next: Language = value;
    edit({ language: next });
    setLanguage(next);
  };

  const cancel = () => {
    if (!loaded) return;
    setDraft({ ...loaded });
    setLanguage(loaded.language);
    setError(null);
    setSaved(null);
  };

  const copyExample = async () => {
    setExampleError(null);
    setExampleCopied(null);
    try {
      await copyToClipboard(exampleCommand, language);
      setExampleCopied(t("settings.exampleCopied"));
    } catch (reason) {
      setExampleError(t("settings.exampleCopyFailed", { error: String(reason) }));
    }
  };

  const save = async () => {
    if (!draft || busy) return;
    const errors = settingsValidation(draft, language);
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
      setLanguage(stored.language);
      setSaved(t("settings.saved", { directory: stored.config_directory }));
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
        <h1>{t("settings.title")}</h1>
        <p>{error ? t("settings.loadError", { error }) : t("common.loading")}</p>
      </section>
    );
  }

  return (
    <section className="panel" aria-labelledby="settings-title">
      <h1 id="settings-title">{t("settings.title")}</h1>
      <p className="hint">{t("settings.hint")}</p>

      <div className="settings-grid">
        <div className="field field-full">
          <span title={configDirExplanation(language)}>{t("settings.configDirectory")}</span>
          <input
            value={draft.config_directory}
            readOnly
            aria-readonly="true"
            aria-label={t("settings.configDirectory")}
            spellCheck={false}
            aria-describedby="config-directory-hint"
            title={configDirExplanation(language)}
          />
          <span id="config-directory-hint" className="hint" title={configDirExplanation(language)}>
            {t("settings.configDirHintBefore")}
            {" "}<code>ALIASMGR_CONFIG_DIR=&lt;path&gt;</code> {t("settings.configDirHintAfter")}
          </span>
          <span className="hint" title={configDirExplanation(language)}>
            {t("settings.configDirExample", {
              platform: configDirPlatformLabel(platform, language),
              command: exampleCommand,
            })}
          </span>
          <span>
            <button type="button" onClick={() => void copyExample()} disabled={busy}>
              {t("settings.copyExample")}
            </button>
          </span>
          {exampleCopied && <span className="hint" role="status">{exampleCopied}</span>}
          {exampleError && <span className="form-error" role="alert">{exampleError}</span>}
        </div>

        <label className="field">
          <span>{t("settings.language")}</span>
          <select
            value={draft.language}
            onChange={(event) => changeLanguage(event.target.value)}
          >
            {LANGUAGES.map((option) => <option key={option.value} value={option.value}>{t(option.labelKey)}</option>)}
          </select>
        </label>

        <label className="field">
          <span>{t("settings.defaultShell")}</span>
          <select value={draft.default_shell} onChange={(event) => edit({ default_shell: event.target.value })}>
            {SHELLS.map((shell) => <option key={shell} value={shell}>{shellLabel(shell, language)}</option>)}
          </select>
        </label>

        <label className="field">
          <span>{t("settings.backupKeep")}</span>
          <input
            type="number"
            min={0}
            step={1}
            value={draft.backup_keep}
            onChange={(event) => edit({ backup_keep: parseKeepCountInput(event.target.value, draft.backup_keep) })}
          />
        </label>

        <label className="field">
          <span>{t("settings.logKeep")}</span>
          <input
            type="number"
            min={0}
            step={1}
            value={draft.log_keep}
            onChange={(event) => edit({ log_keep: parseKeepCountInput(event.target.value, draft.log_keep) })}
          />
        </label>

        <label className="check-row field-full">
          <input
            type="checkbox"
            checked={draft.allow_relative_paths}
            onChange={(event) => edit({ allow_relative_paths: event.target.checked })}
          />
          <span>{t("settings.allowRelativePaths")}</span>
        </label>

        <label className="check-row field-full">
          <input
            type="checkbox"
            checked={draft.auto_sync}
            onChange={(event) => edit({ auto_sync: event.target.checked })}
          />
          <span>{t("settings.autoSync")}</span>
        </label>
      </div>

      {loaded && settingsDirty(loaded, draft) && <p className="hint" role="status">{t("settings.unsaved")}</p>}
      {saved && <p role="status">{saved}</p>}
      {error && <p className="form-error" role="alert">{error}</p>}

      <div className="form-actions">
        <button type="button" onClick={cancel} disabled={busy}>{t("common.cancel")}</button>
        <button type="button" onClick={() => void load()} disabled={busy}>{t("settings.reload")}</button>
        <button type="button" className="btn-primary" onClick={save} disabled={busy}>
          {busy ? t("common.saving") : t("settings.save")}
        </button>
      </div>

      <ShellSettingsPanel
        focusToken={focusShellsToken}
        onChanged={onShellsChanged}
        initialStatuses={shellStatuses}
        active={active}
      />
    </section>
  );
}

export default SettingsPanel;
