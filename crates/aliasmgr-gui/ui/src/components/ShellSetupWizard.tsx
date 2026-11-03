import { useEffect, useRef, useState } from "react";
import type {
  ShellPathOverride,
  ShellSetupOutcome,
  ShellSetupPlan,
  ShellStatus,
} from "../lib";
import {
  defaultWizardSelection,
  normalizeShellSetupOutcome,
  normalizeShellSetupPlan,
  shellLabel,
  shellLoaderLabel,
  shellOptionState,
  shellOptionStateLabel,
  shellSetupArgs,
  shellSetupOutcomeLines,
  shellSourceLabel,
  setupErrorMessage,
  tauriInvoke,
  wizardReadiness,
} from "../lib";
import { useI18n } from "../language";

interface ShellSetupWizardProps {
  statuses: ShellStatus[];
  loading: boolean;
  error: string | null;
  firstRun?: boolean;
  onRescan: () => void;
  onClose: () => void;
  onApplied: () => void;
}

export function ShellSetupWizard({
  statuses,
  loading,
  error,
  firstRun = false,
  onRescan,
  onClose,
  onApplied,
}: ShellSetupWizardProps) {
  const { language, t } = useI18n();
  const [selected, setSelected] = useState<string[]>([]);
  const [overrides, setOverrides] = useState<Record<string, ShellPathOverride>>({});
  const [plans, setPlans] = useState<ShellSetupPlan[]>([]);
  const [outcomes, setOutcomes] = useState<ShellSetupOutcome[]>([]);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"preview" | "apply" | "skip" | null>(null);
  const [applied, setApplied] = useState(false);
  const initialized = useRef(false);

  // Default checks come from persisted intent/valid loaders, never from mere
  // detection alone. A rescan keeps the user's selection for surviving rows.
  useEffect(() => {
    if (statuses.length === 0) return;
    if (!initialized.current) {
      initialized.current = true;
      setSelected(defaultWizardSelection(statuses));
      return;
    }
    setSelected((current) => current.filter((shell) => statuses.some((status) => status.shell === shell)));
  }, [statuses]);

  const readiness = wizardReadiness(statuses, selected, overrides);

  const invalidatePreview = () => {
    setPlans([]);
    setOutcomes([]);
    setApplied(false);
    setActionError(null);
  };

  const toggle = (shell: string) => {
    setSelected((current) => (
      current.includes(shell) ? current.filter((value) => value !== shell) : [...current, shell]
    ));
    invalidatePreview();
  };

  const setOverride = (shell: string, patch: ShellPathOverride) => {
    setOverrides((current) => ({ ...current, [shell]: { ...current[shell], ...patch } }));
    invalidatePreview();
  };

  const readinessMessage = (): string => {
    if (selected.length === 0) return t("shellSetup.noneSelected");
    if (readiness.blocked.length > 0) return t("shellSetup.wslHint");
    return t("shellSetup.notDetectedHint");
  };

  const preview = async () => {
    setActionError(null);
    if (!readiness.ready) {
      setActionError(readinessMessage());
      return;
    }
    setBusy("preview");
    setPlans([]);
    setOutcomes([]);
    setApplied(false);
    try {
      const next: ShellSetupPlan[] = [];
      for (const shell of selected) {
        const plan = normalizeShellSetupPlan(
          await tauriInvoke("shell_setup_plan", shellSetupArgs(shell, overrides[shell])),
        );
        next.push(plan);
      }
      setPlans(next);
    } catch (reason) {
      setActionError(t("shellSetup.applyFailed", { error: setupErrorMessage(reason, language) }));
    } finally {
      setBusy(null);
    }
  };

  const apply = async () => {
    setActionError(null);
    if (!readiness.ready) {
      setActionError(readinessMessage());
      return;
    }
    if (plans.length !== selected.length) {
      setActionError(t("shellSetup.previewNeeded"));
      return;
    }
    setBusy("apply");
    const results: ShellSetupOutcome[] = [];
    try {
      for (const shell of selected) {
        try {
          const outcome = normalizeShellSetupOutcome(
            await tauriInvoke("shell_setup_apply", shellSetupArgs(shell, overrides[shell])),
          );
          results.push(outcome);
        } catch (reason) {
          setActionError(t("shellSetup.applyFailed", { error: setupErrorMessage(reason, language) }));
        }
      }
      setOutcomes(results);
      setApplied(results.length > 0);
      if (results.length > 0) onApplied();
    } finally {
      setBusy(null);
    }
  };

  const skip = async () => {
    setBusy("skip");
    setActionError(null);
    try {
      await tauriInvoke("shell_setup_skip");
      onClose();
    } catch (reason) {
      setActionError(t("shellSetup.applyFailed", { error: String(reason) }));
      setBusy(null);
    }
  };

  return (
    <section
      className="panel shell-setup-wizard"
      aria-labelledby="shell-setup-title"
      data-testid="shell-setup-wizard"
    >
      <div className="panel-header">
        <h1 id="shell-setup-title">
          {firstRun ? t("shellSetup.firstRunTitle") : t("shellSetup.title")}
        </h1>
        <button type="button" onClick={onRescan} disabled={loading || busy !== null}>
          {loading ? t("shellSetup.scanning") : t("shellSetup.rescan")}
        </button>
      </div>
      <p className="hint">{firstRun ? t("shellSetup.firstRunIntro") : t("shellSetup.settingsHint")}</p>
      <p className="hint">{t("shellSetup.selectHint")}</p>

      {error && <p className="form-error" role="alert">{t("shellSetup.wizardFailed", { error })}</p>}
      {actionError && <p className="form-error" role="alert">{actionError}</p>}
      {loading && statuses.length === 0 && <p role="status">{t("shellSetup.scanning")}</p>}

      <ul className="shell-list">
        {statuses.map((status) => {
          const state = shellOptionState(status);
          const checked = selected.includes(status.shell);
          const override = overrides[status.shell] ?? {};
          const editable = checked && (status.action === "needs_path" || !status.detected || status.wsl);
          return (
            <li className="shell-row" key={status.shell} data-shell={status.shell}>
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={checked}
                  disabled={busy !== null}
                  onChange={() => toggle(status.shell)}
                  aria-label={`${shellLabel(status.shell, language)} — ${shellOptionStateLabel(state, language)}`}
                />
                <strong>{shellLabel(status.shell, language)}</strong>
                <span className={`badge badge-${state}`}>{shellOptionStateLabel(state, language)}</span>
              </label>

              <p className="hint">
                {status.detected
                  ? `${status.executable ?? ""} · ${shellSourceLabel(status.source, language)}${
                      status.version ? ` · ${t("shellSetup.version")} ${status.version}` : ""
                    }`
                  : t("shellSetup.notDetected")}
              </p>
              <p className="hint">
                {shellLoaderLabel(status.loader_status, language)}
                {status.loader_target ? ` — ${status.loader_target}` : ""}
              </p>

              {status.wsl && <p className="hint warning">{t("shellSetup.wslHint")}</p>}

              {editable && (
                <div className="field-grid shell-path-editor">
                  <label className="field">
                    <span>{t("shellSetup.executable")}</span>
                    <input
                      value={override.executable ?? ""}
                      placeholder={status.executable ?? ""}
                      onChange={(event) => setOverride(status.shell, { executable: event.target.value })}
                      autoComplete="off"
                      spellCheck={false}
                    />
                  </label>
                  <label className="field">
                    <span>{t("shellSetup.profile")}</span>
                    <input
                      value={override.profile ?? ""}
                      placeholder={status.loader_target ?? ""}
                      onChange={(event) => setOverride(status.shell, { profile: event.target.value })}
                      autoComplete="off"
                      spellCheck={false}
                    />
                  </label>
                  <p className="hint field-full">
                    {t("shellSetup.useDefault")} {t("shellSetup.notDetectedHint")}
                  </p>
                </div>
              )}
            </li>
          );
        })}
      </ul>

      {plans.length > 0 && (
        <section className="shell-preview" aria-labelledby="shell-preview-title">
          <h2 id="shell-preview-title">{t("shellSetup.previewTitle")}</h2>
          {plans.map((plan) => (
            <div className="shell-preview-item" key={plan.shell}>
              <h3>{shellLabel(plan.shell, language)}</h3>
              {plan.previewTarget ? (
                <>
                  <p>
                    <strong>{t("shellSetup.target")}:</strong> <code>{plan.previewTarget}</code>
                  </p>
                  <pre className="code-block">{plan.loaderBlock ?? ""}</pre>
                </>
              ) : (
                <p className="hint">{t("shellSetup.noPreview")}</p>
              )}
            </div>
          ))}
        </section>
      )}

      {outcomes.length > 0 && (
        <section className="shell-result" aria-labelledby="shell-result-title">
          <h2 id="shell-result-title">{t("shellSetup.resultTitle")}</h2>
          {outcomes.map((outcome) => (
            <div className="shell-result-item" key={outcome.shell}>
              <h3>{shellLabel(outcome.shell, language)}</h3>
              <ul>
                {shellSetupOutcomeLines(outcome, language).map((line) => (
                  <li key={line}>{line}</li>
                ))}
              </ul>
              {outcome.loaderError && (
                <p className="form-error" role="alert">{t("shellSetup.loaderNotInstalled")}</p>
              )}
            </div>
          ))}
        </section>
      )}

      <div className="form-actions">
        {!applied && (
          <button type="button" onClick={() => void skip()} disabled={busy !== null}>
            {busy === "skip" ? t("shellSetup.applying") : t("shellSetup.skip")}
          </button>
        )}
        <button type="button" onClick={() => void preview()} disabled={busy !== null || !readiness.ready}>
          {busy === "preview" ? t("shellSetup.previewing") : t("shellSetup.preview")}
        </button>
        <button
          type="button"
          className="btn-primary"
          onClick={() => void apply()}
          disabled={busy !== null || !readiness.ready || plans.length !== selected.length}
        >
          {busy === "apply" ? t("shellSetup.applying") : t("shellSetup.apply")}
        </button>
        {applied && (
          <button type="button" onClick={onClose}>
            {t("common.close")}
          </button>
        )}
      </div>
    </section>
  );
}

export default ShellSetupWizard;
