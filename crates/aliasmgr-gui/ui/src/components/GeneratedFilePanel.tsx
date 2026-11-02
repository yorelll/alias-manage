import { useEffect, useRef, useState } from "react";
import type { SyncRun } from "../lib";
import {
  SHELLS,
  copyToClipboard,
  generatedPreviewErrorMessage,
  normalizeSyncRun,
  reloadCommandExplanation,
  reloadCommandLabel,
  reloadHover,
  shellLabel,
  syncAliasesArgs,
  tauriInvoke,
} from "../lib";
import { useI18n } from "../language";

type RunKind = "generated_preview" | "reload_command" | "sync_aliases";

export function GeneratedFilePanel() {
  const { language, t } = useI18n();
  const [shell, setShell] = useState<string>("bash");
  const [preview, setPreview] = useState("");
  const [reload, setReload] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [busy, setBusy] = useState<RunKind | null>(null);
  const [syncRun, setSyncRun] = useState<SyncRun | null>(null);
  const [syncMessage, setSyncMessage] = useState<string | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const requestToken = useRef(0);

  // Copy/sync notices and localized errors are cleared on language change so a
  // stale message cannot survive a switch.
  useEffect(() => {
    setCopied(null);
    setError(null);
    setSyncMessage(null);
    setSyncError(null);
  }, [language]);

  const changeShell = (value: string) => {
    requestToken.current += 1;
    setShell(value);
    setPreview("");
    setReload("");
    setError(null);
    setCopied(null);
    setBusy(null);
  };

  const run = async (command: RunKind) => {
    const token = ++requestToken.current;
    setBusy(command);
    setError(null);
    setCopied(null);
    try {
      const result = String(await tauriInvoke(command, { shell }));
      if (requestToken.current !== token) return;
      if (command === "generated_preview") setPreview(result);
      else setReload(result);
    } catch (reason) {
      if (requestToken.current === token) {
        setError(
          command === "generated_preview"
            ? generatedPreviewErrorMessage(reason, language)
            : String(reason),
        );
      }
    } finally {
      if (requestToken.current === token) setBusy(null);
    }
  };

  const runSync = async () => {
    setBusy("sync_aliases");
    setSyncError(null);
    setSyncMessage(null);
    setCopied(null);
    try {
      const result = normalizeSyncRun(await tauriInvoke("sync_aliases", syncAliasesArgs()));
      setSyncRun(result);
      if (result.results.length === 0) {
        setSyncMessage(t("generated.syncNoShells"));
      } else if (result.failed) {
        setSyncMessage(t("generated.syncFailed"));
      } else {
        setSyncMessage(t("generated.syncComplete", { revision: result.revision }));
      }
      // The selected shell's preview is stale after a sync.
      setPreview("");
      setError(null);
    } catch (reason) {
      setSyncRun(null);
      setSyncError(t("generated.syncError", { error: String(reason) }));
    } finally {
      setBusy((current) => (current === "sync_aliases" ? null : current));
    }
  };

  const copy = async (text: string, label: string) => {
    setError(null);
    try {
      await copyToClipboard(text, language);
      setCopied(t("generated.copied", { label }));
    } catch (reason) {
      setCopied(null);
      setError(String(reason));
    }
  };

  return (
    <section className="panel" aria-labelledby="generated-title">
      <h2 id="generated-title">{t("generated.title")}</h2>
      <p className="hint">{t("generated.hint")}</p>
      <p className="hint" title={reloadHover(language)}>
        {t("generated.reloadExplanation")}
      </p>

      <label className="field">
        <span>{t("generated.shell")}</span>
        <select value={shell} onChange={(event) => changeShell(event.target.value)}>
          {SHELLS.map((value) => <option key={value} value={value}>{shellLabel(value, language)}</option>)}
        </select>
      </label>

      <div className="form-actions">
        <button type="button" className="btn-primary" onClick={() => void runSync()} disabled={busy !== null}>
          {busy === "sync_aliases" ? t("generated.syncing") : t("generated.syncNow")}
        </button>
        <button type="button" onClick={() => void run("generated_preview")} disabled={busy !== null}>
          {busy === "generated_preview" ? t("common.loading") : t("generated.showFile")}
        </button>
        <button
          type="button"
          onClick={() => void run("reload_command")}
          disabled={busy !== null}
          title={reloadHover(language)}
        >
          {busy === "reload_command" ? t("common.loading") : t("generated.showReload")}
        </button>
      </div>

      {syncMessage && <p className={syncRun?.failed ? "form-error" : "hint"} role="status">{syncMessage}</p>}
      {syncError && <p className="form-error" role="alert">{syncError}</p>}
      {error && <p className="form-error" role="alert">{error}</p>}

      {syncRun && syncRun.results.length > 0 && (
        <div className="sync-results" aria-live="polite">
          <p className="hint" title={reloadHover(language)}>{t("generated.reloadRequired")}</p>
          <ul className="sync-result-list">
            {syncRun.results.map((result) => (
              <li className={`sync-result sync-result-${result.status}`} key={result.shell} data-testid={`sync-result-${result.shell}`}>
                <div className="sync-result-header">
                  <strong>{shellLabel(result.shell, language)}</strong>
                  <span className={result.status === "ok" ? "badge badge-ok" : "badge badge-warn"}>
                    {result.status === "ok" ? t("generated.statusOk") : t("generated.statusFailed")}
                  </span>
                </div>
                {result.error && (
                  <p className="form-error" role="alert">
                    {t("generated.errorDetail", { error: result.error })}
                  </p>
                )}
                {result.generated_path && (
                  <p className="hint">{t("generated.generatedPath", { path: result.generated_path })}</p>
                )}
                {result.reload_command && (
                  <>
                    <p className="reload-command-label">{reloadCommandLabel(result.shell, language)}</p>
                    <pre className="code-block" title={reloadHover(language)}>{result.reload_command}</pre>
                    <p className="hint">{reloadCommandExplanation(result.shell, language)}</p>
                    <button
                      type="button"
                      onClick={() => void copy(result.reload_command, reloadCommandLabel(result.shell, language))}
                    >
                      {t("common.copy")}
                    </button>
                  </>
                )}
                {!result.loader_installed && <p className="hint warning">{t("generated.loaderMissing")}</p>}
              </li>
            ))}
          </ul>
        </div>
      )}

      {preview && (
        <div className="preview-snippet">
          <div className="preview-header">
            <h3>{t("generated.previewTitle")}</h3>
            <button type="button" onClick={() => void copy(preview, t("generated.previewTitle"))}>{t("common.copy")}</button>
          </div>
          <pre className="code-block">{preview}</pre>
        </div>
      )}

      {reload && (
        <div className="preview-snippet">
          <div className="preview-header">
            <h3>{t("generated.reloadTitle")}</h3>
            <button type="button" onClick={() => void copy(reload, reloadCommandLabel(shell, language))}>{t("common.copy")}</button>
          </div>
          <p className="reload-command-label">{reloadCommandLabel(shell, language)}</p>
          <pre className="code-block" title={reloadHover(language)}>{reload}</pre>
          <p className="hint">{reloadCommandExplanation(shell, language)}</p>
        </div>
      )}

      {copied && <p className="hint" role="status">{copied}</p>}
    </section>
  );
}

export default GeneratedFilePanel;
