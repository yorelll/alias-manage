import { useRef, useState } from "react";
import { SHELLS, copyToClipboard, shellLabel, tauriInvoke } from "../lib";
import { useI18n } from "../language";

type RunKind = "generated_preview" | "reload_command";

export function GeneratedFilePanel() {
  const { language, t } = useI18n();
  const [shell, setShell] = useState<string>("bash");
  const [preview, setPreview] = useState("");
  const [reload, setReload] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [busy, setBusy] = useState<RunKind | null>(null);
  const requestToken = useRef(0);

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
      if (requestToken.current === token) setError(String(reason));
    } finally {
      if (requestToken.current === token) setBusy(null);
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

      <label className="field">
        <span>{t("generated.shell")}</span>
        <select value={shell} onChange={(event) => changeShell(event.target.value)}>
          {SHELLS.map((value) => <option key={value} value={value}>{shellLabel(value, language)}</option>)}
        </select>
      </label>

      <div className="form-actions">
        <button type="button" onClick={() => void run("generated_preview")} disabled={busy !== null}>
          {busy === "generated_preview" ? t("common.loading") : t("generated.showFile")}
        </button>
        <button type="button" onClick={() => void run("reload_command")} disabled={busy !== null}>
          {busy === "reload_command" ? t("common.loading") : t("generated.showReload")}
        </button>
      </div>

      {error && <p className="form-error" role="alert">{error}</p>}

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
            <button type="button" onClick={() => void copy(reload, t("generated.reloadTitle"))}>{t("common.copy")}</button>
          </div>
          <pre className="code-block">{reload}</pre>
        </div>
      )}

      {copied && <p className="hint" role="status">{copied}</p>}
    </section>
  );
}

export default GeneratedFilePanel;
