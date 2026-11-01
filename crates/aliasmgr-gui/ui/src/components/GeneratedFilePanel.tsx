import { useRef, useState } from "react";
import { SHELLS, copyToClipboard, shellLabel, tauriInvoke } from "../lib";

type RunKind = "generated_preview" | "reload_command";

export function GeneratedFilePanel() {
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
      await copyToClipboard(text);
      setCopied(`${label} copied.`);
    } catch (reason) {
      setCopied(null);
      setError(String(reason));
    }
  };

  return (
    <section className="panel" aria-labelledby="generated-title">
      <h2 id="generated-title">Generated file</h2>
      <p className="hint">Generated definitions and reload commands are shown for the selected shell. Nothing is executed.</p>

      <label className="field">
        <span>Shell</span>
        <select value={shell} onChange={(event) => changeShell(event.target.value)}>
          {SHELLS.map((value) => <option key={value} value={value}>{shellLabel(value)}</option>)}
        </select>
      </label>

      <div className="form-actions">
        <button type="button" onClick={() => void run("generated_preview")} disabled={busy !== null}>
          {busy === "generated_preview" ? "Loading…" : "Show generated file"}
        </button>
        <button type="button" onClick={() => void run("reload_command")} disabled={busy !== null}>
          {busy === "reload_command" ? "Loading…" : "Show reload command"}
        </button>
      </div>

      {error && <p className="form-error" role="alert">{error}</p>}

      {preview && (
        <div className="preview-snippet">
          <div className="preview-header">
            <h3>Generated file preview</h3>
            <button type="button" onClick={() => void copy(preview, "Generated file preview")}>Copy</button>
          </div>
          <pre className="code-block">{preview}</pre>
        </div>
      )}

      {reload && (
        <div className="preview-snippet">
          <div className="preview-header">
            <h3>Reload command</h3>
            <button type="button" onClick={() => void copy(reload, "Reload command")}>Copy</button>
          </div>
          <pre className="code-block">{reload}</pre>
        </div>
      )}

      {copied && <p className="hint" role="status">{copied}</p>}
    </section>
  );
}

export default GeneratedFilePanel;
