import { useCallback, useEffect, useState } from "react";
import type { DoctorFinding, OverriddenDefinition } from "../lib";
import {
  definitionKindLabel,
  doctorActionFor,
  doctorActionLabel,
  doctorFindingText,
  findingCodeLabel,
  manualOnlyBoundary,
  normalizeDoctorFindings,
  normalizeOverriddenDefinitions,
  normalizeSyncRun,
  orderedSeverities,
  overrideMetadataText,
  overrideSummary,
  severityCounts,
  severityLabel,
  shellInstallArgs,
  shellLabel,
  syncAliasesArgs,
  tauriInvoke,
} from "../lib";
import { useI18n } from "../language";

function severityClass(severity: string): string {
  return `severity-badge severity-${severity}`;
}

export function DoctorPanel() {
  const { language, t } = useI18n();
  const [findings, setFindings] = useState<DoctorFinding[]>([]);
  const [overrides, setOverrides] = useState<OverriddenDefinition[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [overrideError, setOverrideError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [overridesLoading, setOverridesLoading] = useState(false);
  const [copyNotice, setCopyNotice] = useState<string | null>(null);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState<string | null>(null);
  const [actionNotice, setActionNotice] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  // Copy/action notices and localized errors are rebuilt from i18n; drop them
  // when the language changes so no stale message survives the switch.
  useEffect(() => {
    setCopyNotice(null);
    setCopyError(null);
    setActionNotice(null);
    setActionError(null);
  }, [language]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setOverrideError(null);

    Promise.allSettled([
      tauriInvoke("doctor_status"),
      tauriInvoke("scan_overrides"),
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
    setCopyError(null);
    try {
      await navigator.clipboard.writeText(text);
      setCopyNotice(t("doctor.copied", { label }));
    } catch (reason) {
      setCopyNotice(null);
      setCopyError(t("doctor.copyFailed", { label, error: String(reason) }));
    }
  }, [t]);

  const scanOverrides = useCallback(async () => {
    setOverridesLoading(true);
    setOverrideError(null);
    setCopyNotice(null);
    try {
      const value = await tauriInvoke("scan_overrides");
      setOverrides(normalizeOverriddenDefinitions(value));
    } catch (reason) {
      setOverrideError(String(reason));
    } finally {
      setOverridesLoading(false);
    }
  }, []);

  /**
   * Run the one action a finding offers (sync or loader install), then rescan
   * Doctor automatically. The action set is decided by `doctorActionFor`, so
   * read-only findings never grow a button by accident.
   */
  const runAction = useCallback(async (finding: DoctorFinding, index: number) => {
    const action = doctorActionFor(finding);
    if (!action || actionBusy !== null) return;
    const key = `${finding.code}-${finding.shell ?? "global"}-${index}`;
    setActionBusy(key);
    setActionError(null);
    setActionNotice(null);
    setCopyNotice(null);
    setCopyError(null);
    try {
      if (action === "sync") {
        const run = normalizeSyncRun(await tauriInvoke("sync_aliases", syncAliasesArgs()));
        if (run.failed) {
          const failures = run.results
            .filter((result) => result.status === "failed")
            .map((result) => `${shellLabel(result.shell, language)}: ${result.error ?? ""}`)
            .join("; ");
          setActionError(t("doctor.syncFailedResult", { error: failures }));
        } else {
          setActionNotice(
            `${t("doctor.syncResult", {
              count: run.results.length,
              revision: run.revision,
            })} ${t("doctor.reloadAfterSync")}`,
          );
        }
      } else {
        const shell = finding.shell ?? "";
        const outcome = await tauriInvoke("shell_install", shellInstallArgs(shell)) as {
          target?: unknown;
        };
        setActionNotice(
          `${t("doctor.installResult", {
            shell: shellLabel(shell, language),
            path: typeof outcome?.target === "string" ? outcome.target : "",
          })} ${t("doctor.reloadAfterInstall")}`,
        );
      }
      // Re-scan so the finding reflects the new state without a manual click.
      setReloadToken((value) => value + 1);
    } catch (reason) {
      setActionError(t("doctor.actionFailed", { error: String(reason) }));
    } finally {
      setActionBusy(null);
    }
  }, [actionBusy, language, t]);

  const counts = severityCounts(findings);
  const severities = orderedSeverities(counts);

  return (
    <section className="panel">
      <div className="panel-header">
        <h1>{t("doctor.title")}</h1>
        <button type="button" onClick={() => setReloadToken((value) => value + 1)} disabled={loading}>
          {loading ? t("doctor.scanning") : t("doctor.rescan")}
        </button>
      </div>

      <p className="hint" data-testid="doctor-purpose" title={t("doctor.purposeHover")}>
        {t("doctor.purpose")}
      </p>
      <p className="boundary-note" data-testid="manual-boundary" title={t("doctor.purposeHover")}>{manualOnlyBoundary(language)}</p>

      {actionNotice && <p role="status">{actionNotice}</p>}
      {actionError && <p className="form-error" role="alert">{actionError}</p>}
      {error && <p className="form-error" role="alert">{t("doctor.loadError", { error })}</p>}

      {severities.length > 0 && (
        <div className="severity-summary" aria-label={t("doctor.severityCounts")}>
          {severities.map((severity) => (
            <span className={severityClass(severity)} key={severity}>{severityLabel(severity, language)}: {counts[severity]}</span>
          ))}
        </div>
      )}

      {loading && findings.length === 0 && <p role="status">{t("doctor.scanningConfig")}</p>}
      {!loading && findings.length === 0 && !error && <p>{t("doctor.noFindings")}</p>}

      <div className="finding-list">
        {findings.map((finding, index) => {
          const text = doctorFindingText(finding, language);
          const action = doctorActionFor(finding);
          const actionKey = `${finding.code}-${finding.shell ?? "global"}-${index}`;
          return (
            <details className="finding" key={`${finding.shell ?? "global"}-${finding.code}-${index}`}>
              <summary>
                <span className={severityClass(finding.severity)}>{severityLabel(finding.severity, language)}</span>
                <span className="finding-scope">{finding.shell ? shellLabel(finding.shell, language) : t("doctor.global")}</span>
                <span className="finding-title">{text.title}</span>
                <span className="finding-code" title={finding.code}>{findingCodeLabel(finding.code, language)}</span>
              </summary>
              <div className="finding-body">
                <p>{text.message}</p>
                {text.action && (
                  <div className="finding-action">
                    <p><strong>{t("doctor.suggestedAction")}</strong> {text.action}</p>
                    <button type="button" onClick={() => void copy(text.action ?? "", t("doctor.labelSuggestion"))}>
                      {t("doctor.copySuggestion")}
                    </button>
                  </div>
                )}
                {action && (
                  <div className="finding-action">
                    <button
                      type="button"
                      className="btn-primary"
                      onClick={() => void runAction(finding, index)}
                      disabled={actionBusy !== null || loading}
                      aria-busy={actionBusy === actionKey}
                    >
                      {actionBusy === actionKey ? t("doctor.actionRunning") : doctorActionLabel(action, language)}
                    </button>
                  </div>
                )}
                {finding.detail && <pre className="code-block">{finding.detail}</pre>}
                <p className="hint">{t("doctor.manualHint")}</p>
              </div>
            </details>
          );
        })}
      </div>

      <section className="overridden-section" aria-labelledby="overridden-title">
        <div className="panel-header">
          <h2 id="overridden-title">{t("doctor.overriddenTitle")}</h2>
          <button type="button" onClick={() => void scanOverrides()} disabled={overridesLoading || loading}>
            {overridesLoading ? t("doctor.scanningOverrides") : t("doctor.scanOverrides")}
          </button>
        </div>
        <p className="hint">{t("doctor.overriddenHint")}</p>
        {overrideError && <p className="form-error" role="alert">{t("doctor.overrideScanError", { error: overrideError })}</p>}
        {!overrideError && !loading && overrides.length === 0 && <p>{t("doctor.noOverrides")}</p>}
        {overrides.length > 0 && (
          <ul className="override-list">
            {overrides.map((entry, index) => {
              const metadata = overrideMetadataText(entry, language);
              return (
                <li className="override-item" key={`${entry.name}-${entry.shell}-${index}`}>
                  <div className="override-header">
                    <strong>{overrideSummary(entry, language)}</strong>
                    <span className={entry.recoverable ? "badge badge-ok" : "badge badge-warn"}>
                      {t(entry.recoverable ? "doctor.recoverable" : "doctor.notRecoverable")}
                    </span>
                  </div>
                  <p className="hint">
                    {t("doctor.captured", {
                      kind: definitionKindLabel(entry.definition_kind, language),
                      time: entry.captured_at ?? t("doctor.unknownTime"),
                    })}
                  </p>
                  <pre className="code-block">{metadata}</pre>
                  <button type="button" onClick={() => void copy(metadata, t("doctor.labelOverrideMetadata"))}>
                    {t("doctor.copyMetadata")}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      {copyError && <p className="form-error" role="alert">{copyError}</p>}
      {copyNotice && <p className="hint" role="status">{copyNotice}</p>}
    </section>
  );
}

export default DoctorPanel;
