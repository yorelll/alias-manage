import { useEffect, useRef, useState } from "react";
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
import { useI18n } from "../language";

interface SelectedFile {
  name: string;
  content: string;
}

function ReportList({ title, items }: { title: string; items: string[] }) {
  const { t } = useI18n();
  return (
    <div className="report-card">
      <h3>{title} ({items.length})</h3>
      {items.length > 0
        ? <ul>{items.map((item, index) => <li key={`${item}-${index}`}>{item}</li>)}</ul>
        : <p className="hint">{t("common.none")}</p>}
    </div>
  );
}

function SkipList({ entries }: { entries: ImportSkipEntry[] }) {
  const { t } = useI18n();
  return (
    <div className="report-card">
      <h3>{t("import.skipped")} ({entries.length})</h3>
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
        : <p className="hint">{t("common.none")}</p>}
    </div>
  );
}

export function ImportPanel({ onImported }: { onImported?: () => void }) {
  const { language, t } = useI18n();
  const [file, setFile] = useState<SelectedFile | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"preview" | "confirm" | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const requestToken = useRef(0);

  // Invalid-file/import errors are localized at render time; clear stale copy.
  useEffect(() => { setError(null); }, [language]);

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
      setError(t("import.invalidFile"));
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
      <h2 id="import-title">{t("import.title")}</h2>
      <p className="hint">
        {t("import.hint")}
      </p>

      <label className="field file-picker">
        <span>{t("import.fileLabel")}</span>
        <input
          ref={inputRef}
          id="import-file-input"
          type="file"
          accept=".json,.toml"
          onChange={(event) => void selectFile(event)}
          disabled={busy !== null}
        />
      </label>

      {file && (
        <p className="hint" role="status">
          {t("import.fileStats", { name: file.name, count: file.content.length })}
        </p>
      )}

      {busy && (
        <p role="status">
          {busy === "confirm" ? t("import.importing") : t("import.reading")}
        </p>
      )}
      {error && <p className="form-error" role="alert">{error}</p>}

      {report && counts && (
        <div className="import-report">
          <p role="status" className="import-summary">
            {t("import.summary", {
              status: t(confirmed ? "import.complete" : "import.previewOnly"),
              imported: counts.imported,
              skipped: counts.skipped,
              warnings: counts.warnings,
              unsupported: counts.unsupported,
            })}
          </p>
          <div className="report-grid">
            <ReportList title={t("import.imported")} items={report.imported} />
            <SkipList entries={importSkipEntries(report, language)} />
            <ReportList title={t("import.warnings")} items={report.warnings} />
            <ReportList title={t("import.unsupported")} items={report.unsupported} />
          </div>
        </div>
      )}

      <div className="form-actions">
        <button type="button" onClick={reset} disabled={busy === "confirm"}>
          {busy === "confirm" ? t("import.importingShort") : t("common.cancel")}
        </button>
        <button
          type="button"
          className="btn-primary"
          onClick={confirm}
          disabled={!report || confirmed || busy !== null}
        >
          {t("import.confirm")}
        </button>
      </div>
    </section>
  );
}

export default ImportPanel;
