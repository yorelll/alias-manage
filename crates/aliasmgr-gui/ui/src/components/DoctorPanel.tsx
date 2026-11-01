import { useCallback, useEffect, useState } from "react";
import type { DoctorFinding, OverriddenDefinition } from "../lib";
import {
  MANUAL_ONLY_BOUNDARY,
  doctorFindingText,
  normalizeDoctorFindings,
  normalizeOverriddenDefinitions,
  orderedSeverities,
  overrideRecoveryText,
  overrideSummary,
  severityCounts,
  shellLabel,
  tauriInvoke,
} from "../lib";

function severityClass(severity: string): string {
  return `severity-badge severity-${severity}`;
}

export function DoctorPanel() {
  const [findings, setFindings] = useState<DoctorFinding[]>([]);
  const [overrides, setOverrides] = useState<OverriddenDefinition[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [overrideError, setOverrideError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [copyNotice, setCopyNotice] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setOverrideError(null);

    Promise.allSettled([
      tauriInvoke("doctor_status"),
      tauriInvoke("overridden_definitions"),
    ]).then(([doctorResult, overrideResult]) => {
      if (cancelled) return;
      if (doctorResult.status === "fulfilled") {
        setFindings(normalizeDoctorFindings(doctorResult.value));
      } else {
        setError(String(doctorResult.reason));
      }
      if (overrideResult.status === "fulfilled") {
        setOverrides(normalizeOverriddenDefinitions(overrideResult.value));
      } else {
        setOverrideError(String(overrideResult.reason));
      }
      setLoading(false);
    });

    return () => { cancelled = true; };
  }, [reloadToken]);

  const copy = useCallback(async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopyNotice(`${label} copied. Paste it into your own terminal or notes; nothing was executed.`);
    } catch (reason) {
      setCopyNotice(null);
      setError(`Unable to copy ${label}: ${String(reason)}`);
    }
  }, []);

  const counts = severityCounts(findings);
  const severities = orderedSeverities(counts);

  return (
    <section className="panel">
      <div className="panel-header">
        <h1>Doctor</h1>
        <button type="button" onClick={() => setReloadToken((value) => value + 1)} disabled={loading}>
          {loading ? "Scanning…" : "Rescan"}
        </button>
      </div>

      <p className="boundary-note" data-testid="manual-boundary">{MANUAL_ONLY_BOUNDARY}</p>

      {error && <p className="form-error" role="alert">Unable to load diagnostics: {error}</p>}

      {severities.length > 0 && (
        <div className="severity-summary" aria-label="Finding severity counts">
          {severities.map((severity) => (
            <span className={severityClass(severity)} key={severity}>{severity}: {counts[severity]}</span>
          ))}
        </div>
      )}

      {loading && findings.length === 0 && <p role="status">Scanning shells and configuration…</p>}
      {!loading && findings.length === 0 && !error && <p>No diagnostic findings.</p>}

      <div className="finding-list">
        {findings.map((finding, index) => {
          const text = doctorFindingText(finding);
          return (
            <details className="finding" key={`${finding.shell ?? "global"}-${finding.code}-${index}`}>
              <summary>
                <span className={severityClass(finding.severity)}>{finding.severity}</span>
                <span className="finding-scope">{finding.shell ? shellLabel(finding.shell) : "Global"}</span>
                <span className="finding-title">{text.title}</span>
                <span className="finding-code">{finding.code}</span>
              </summary>
              <div className="finding-body">
                <p>{text.message}</p>
                {text.action && (
                  <div className="finding-action">
                    <p><strong>Suggested manual action:</strong> {text.action}</p>
                    <button type="button" onClick={() => void copy(text.action ?? "", "suggestion")}>
                      Copy suggestion
                    </button>
                  </div>
                )}
                {finding.detail && <pre className="code-block">{finding.detail}</pre>}
                <p className="hint">Actions listed here are manual. The GUI never runs them for you.</p>
              </div>
            </details>
          );
        })}
      </div>

      <section className="overridden-section" aria-labelledby="overridden-title">
        <h2 id="overridden-title">Overridden definitions</h2>
        <p className="hint">
          These user definitions were shadowed by managed aliases. Recovery text is copied as data only and is never executed.
        </p>
        {overrideError && <p className="form-error" role="alert">Unable to load overridden definitions: {overrideError}</p>}
        {!overrideError && !loading && overrides.length === 0 && <p>No overridden definitions recorded.</p>}
        {overrides.length > 0 && (
          <ul className="override-list">
            {overrides.map((entry, index) => {
              const recovery = overrideRecoveryText(entry);
              return (
                <li className="override-item" key={`${entry.name}-${entry.shell}-${index}`}>
                  <div className="override-header">
                    <strong>{overrideSummary(entry)}</strong>
                    <span className={entry.recoverable ? "badge badge-ok" : "badge badge-warn"}>
                      {entry.recoverable ? "recoverable" : "not recoverable"}
                    </span>
                  </div>
                  <p className="hint">
                    {entry.definition_kind} · captured {entry.captured_at ?? "unknown time"}
                  </p>
                  {recovery
                    ? (
                      <>
                        <pre className="code-block">{recovery}</pre>
                        <button type="button" onClick={() => void copy(recovery, "recovery text")}>
                          Copy recovery text
                        </button>
                      </>
                    )
                    : <p className="hint">No recovery text was captured for this definition, so copying is unavailable.</p>}
                </li>
              );
            })}
          </ul>
        )}
      </section>

      {copyNotice && <p className="hint" role="status">{copyNotice}</p>}
    </section>
  );
}

export default DoctorPanel;
