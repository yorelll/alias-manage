import { useState } from "react";
import { tauriInvoke } from "../lib";

export function ImportPanel() {
  const [file, setFile] = useState("");
  const [report, setReport] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);
  const preview = async () => { setError(null); try { setReport(await tauriInvoke("import_preview")); } catch (reason) { setError(String(reason)); } };
  const confirm = async () => { if (!file) return; setError(null); try { setReport(await tauriInvoke("import_confirm")); } catch (reason) { setError(String(reason)); } };
  return <section className="panel"><h1>Import</h1><input value={file} onChange={(event) => setFile(event.target.value)} placeholder="Path to JSON/TOML" /><button onClick={preview}>Preview</button>{error && <p role="alert">{String(error)}</p>}{report && <pre>{JSON.stringify(report, null, 2)}</pre>}<button disabled={!report} onClick={confirm}>Confirm import</button></section>;
}

export default ImportPanel;
