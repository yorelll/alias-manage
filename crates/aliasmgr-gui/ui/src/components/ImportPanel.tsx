import { useRef, useState } from "react";
import type { ChangeEvent } from "react";
import type { ImportReport } from "../lib";
import {
  importContentArgs,
  importReportCounts,
  isImportableFilename,
  normalizeImportReport,
  tauriInvoke,
} from "../lib";

interface SelectedFile {
  name: string;
  content: string;
}

function ReportList({ title, items }: { title: string; items: string[] }) {
  return (
    <div className="report-card">
      <h3>{title} ({items.length})</h3>
      {items.length > 0
        ? <ul>{items.map((item, index) => <li key={`${item}-${index}`}>{item}</li>)}</ul>
        : <p className="hint">None</p>}
    </div>
  );
}

export function ImportPanel() {
  const [file, setFile] = useState<SelectedFile | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const requestToken = useRef(0);

  const reset = () => {
    requestToken.current += 1;
    setFile(null);
    setReport(null);
    setConfirmed(false);
    setError(null);
    setBusy(false);
    if (inputRef.current) inputRef.current.value = "";
  };

  const selectFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const selected = event.target.files?.[0];
    if (!selected) return;
    const token = ++requestToken.current;
    setReport(null);
    setConfirmed(false);
    setError(null);
    if (!isImportableFilename(selected.name)) {
      setFile(null);
      setBusy(false);
      setError("Choose a .json or .toml file.");
      event.target.value = "";
      return;
    }
    setBusy(true);
    try {
      const content = await selected.text();
      if (requestToken.current !== token) return;
      setFile({ name: selected.name, content });
      const result = await tauriInvoke("import_preview_content", importContentArgs(content, selected.name));
      if (requestToken.current !== token) return;
      setReport(normalizeImportReport(result));
    } catch (reason) {
      if (requestToken.current === token) {
        setReport(null);
        setError(String(reason));
      }
    } finally {
      if (requestToken.current === token) setBusy(false);
    }
  };

  const confirm = async () => {
    if (!file || !report || busy) return;
    const token = ++requestToken.current;
    setBusy(true);
    setError(null);
    try {
      const result = await tauriInvoke("import_confirm_content", importContentArgs(file.content, file.name));
      if (requestToken.current !== token) return;
      setReport(normalizeImportReport(result));
      setConfirmed(true);
    } catch (reason) {
      if (requestToken.current === token) setError(String(reason));
    } finally {
      if (requestToken.current === token) setBusy(false);
    }
  };

  const counts = report ? importReportCounts(report) : null;

  return (
    <section className="panel" aria-labelledby="import-title">
      <h2 id="import-title">Import</h2>
      <p className="hint">
        Choose a JSON or TOML file. The preview is read-only; nothing is written until you confirm.
      </p>

      <label className="field file-picker">
        <span>Import file (.json or .toml)</span>
        <input
          ref={inputRef}
          type="file"
          accept=".json,.toml"
          onChange={(event) => void selectFile(event)}
          disabled={busy}
        />
      </label>

      {file && (
        <p className="hint" role="status">
          {file.name} · {file.content.length} characters
        </p>
      )}

      {busy && <p role="status">Reading and validating file…</p>}
      {error && <p className="form-error" role="alert">{error}</p>}

      {report && counts && (
        <div className="import-report">
          <p role="status" className="import-summary">
            {confirmed ? "Import complete." : "Preview only — not yet imported."} {counts.imported} to import,
            {" "}{counts.skipped} skipped, {counts.warnings} warnings, {counts.unsupported} unsupported.
          </p>
          <div className="report-grid">
            <ReportList title="Imported" items={report.imported} />
            <ReportList title="Skipped" items={report.skipped} />
            <ReportList title="Warnings" items={report.warnings} />
            <ReportList title="Unsupported" items={report.unsupported} />
          </div>
        </div>
      )}

      <div className="form-actions">
        <button type="button" onClick={reset}>Cancel</button>
        <button
          type="button"
          className="btn-primary"
          onClick={confirm}
          disabled={!report || confirmed || busy}
        >
          Confirm import
        </button>
      </div>
    </section>
  );
}

export default ImportPanel;
