import { useRef, useState } from "react";
import type { ChangeEvent } from "react";
import type { ImportReport, ImportSkipEntry } from "../lib";
import {
  importContentArgs,
  importReportCounts,
  importSkipEntries,
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

function SkipList({ entries }: { entries: ImportSkipEntry[] }) {
  return (
    <div className="report-card">
      <h3>Skipped ({entries.length})</h3>
      {entries.length > 0
        ? (
          <ul>
            {entries.map((entry, index) => (
              <li key={`${entry.name}-${index}`}>
                <strong>{entry.name}</strong>
                {entry.reason && <span> — {entry.reason}</span>}
              </li>
            ))}
          </ul>
        )
        : <p className="hint">None</p>}
    </div>
  );
}

export function ImportPanel({ onImported }: { onImported?: () => void }) {
  const [file, setFile] = useState<SelectedFile | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"preview" | "confirm" | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const requestToken = useRef(0);

  const reset = () => {
    requestToken.current += 1;
    setFile(null);
    setReport(null);
    setConfirmed(false);
    setError(null);
    setBusy(null);
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
      setBusy(null);
      setError("Choose a .json or .toml file.");
      event.target.value = "";
      return;
    }
    setBusy("preview");
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
      if (requestToken.current === token) setBusy(null);
    }
  };

  const confirm = async () => {
    if (!file || !report || busy) return;
    const token = ++requestToken.current;
    setBusy("confirm");
    setError(null);
    try {
      const result = await tauriInvoke("import_confirm_content", importContentArgs(file.content, file.name));
      if (requestToken.current !== token) return;
      setReport(normalizeImportReport(result));
      setConfirmed(true);
      onImported?.();
    } catch (reason) {
      if (requestToken.current === token) setError(String(reason));
    } finally {
      if (requestToken.current === token) setBusy(null);
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
          disabled={busy !== null}
        />
      </label>

      {file && (
        <p className="hint" role="status">
          {file.name} · {file.content.length} characters
        </p>
      )}

      {busy && (
        <p role="status">
          {busy === "confirm" ? "Importing aliases…" : "Reading and validating file…"}
        </p>
      )}
      {error && <p className="form-error" role="alert">{error}</p>}

      {report && counts && (
        <div className="import-report">
          <p role="status" className="import-summary">
            {confirmed ? "Import complete." : "Preview only — not yet imported."} {counts.imported} to import,
            {" "}{counts.skipped} skipped, {counts.warnings} warnings, {counts.unsupported} unsupported.
          </p>
          <div className="report-grid">
            <ReportList title="Imported" items={report.imported} />
            <SkipList entries={importSkipEntries(report)} />
            <ReportList title="Warnings" items={report.warnings} />
            <ReportList title="Unsupported" items={report.unsupported} />
          </div>
        </div>
      )}

      <div className="form-actions">
        <button type="button" onClick={reset} disabled={busy === "confirm"}>
          {busy === "confirm" ? "Importing…" : "Cancel"}
        </button>
        <button
          type="button"
          className="btn-primary"
          onClick={confirm}
          disabled={!report || confirmed || busy !== null}
        >
          Confirm import
        </button>
      </div>
    </section>
  );
}

export default ImportPanel;
