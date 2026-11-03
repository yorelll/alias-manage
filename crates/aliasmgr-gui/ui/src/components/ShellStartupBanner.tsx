import { useState } from "react";
import type { ShellSetupOutcome, ShellStartupState } from "../lib";
import {
  normalizeShellSetupOutcome,
  shellDismissDetectedArgs,
  shellLabel,
  shellSetShellEnabledArgs,
  shellSetupArgs,
  shellSetupOutcomeLines,
  setupErrorMessage,
  setupOutcomeFailed,
  tauriInvoke,
} from "../lib";
import { useI18n } from "../language";

interface ShellStartupBannerProps {
  state: ShellStartupState;
  /** Shells whose warning was dismissed for this session only. */
  ignored: string[];
  onIgnore: (shell: string) => void;
  onChanged: () => void;
  onOpenSetup: () => void;
}

/**
 * Non-blocking startup notices. The full wizard is shown only for a brand-new
 * configuration; later launches report invalid enabled loaders and newly
 * detected shells here. Setup outcomes (including loader and sync failures)
 * stay visible until the user acknowledges them, then startup state refreshes.
 */
export function ShellStartupBanner({
  state,
  ignored,
  onIgnore,
  onChanged,
  onOpenSetup,
}: ShellStartupBannerProps) {
  const { language, t } = useI18n();
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [outcomes, setOutcomes] = useState<ShellSetupOutcome[]>([]);

  const invalid = state.invalidEnabled.filter((shell) => !ignored.includes(shell));
  const detected = state.newlyDetected.filter((shell) => !ignored.includes(shell));
  const notConfigured = state.shells.length > 0 && state.shells.every((status) => !status.enabled);

  if (state.firstRun) return null;
  if (invalid.length === 0 && detected.length === 0 && !notConfigured && outcomes.length === 0) {
    return null;
  }

  /** `refresh` is false while setup results are waiting for acknowledgement. */
  const run = async (key: string, action: () => Promise<boolean>) => {
    setBusy(key);
    setError(null);
    try {
      if (await action()) onChanged();
    } catch (reason) {
      setError(t("shellSetup.bannerError", { error: setupErrorMessage(reason, language) }));
    } finally {
      setBusy(null);
    }
  };

  const recordOutcomes = (next: ShellSetupOutcome[]) => {
    setOutcomes((current) => {
      const merged = [...current];
      for (const outcome of next) {
        const index = merged.findIndex((entry) => entry.shell === outcome.shell);
        if (index >= 0) merged[index] = outcome;
        else merged.push(outcome);
      }
      return merged;
    });
    return false;
  };

  const acknowledge = () => {
    setOutcomes([]);
    setError(null);
    onChanged();
  };

  const disable = (shell: string) => run(`disable-${shell}`, async () => {
    // Per-shell backend toggle re-reads shared config, so a shell enabled by
    // another entry point is preserved. Disabling also records `shells.seen`.
    await tauriInvoke("shell_set_shell_enabled", shellSetShellEnabledArgs(shell, false));
    return true;
  });

  const reenable = (shell: string) => run(`enable-${shell}`, async () => {
    const outcome = normalizeShellSetupOutcome(
      await tauriInvoke("shell_setup_apply", shellSetupArgs(shell)),
    );
    return recordOutcomes([outcome]);
  });

  const enableDetected = () => run("detected", async () => {
    const next: ShellSetupOutcome[] = [];
    let firstError: unknown = null;
    for (const shell of detected) {
      try {
        next.push(normalizeShellSetupOutcome(
          await tauriInvoke("shell_setup_apply", shellSetupArgs(shell)),
        ));
      } catch (reason) {
        // Continue with the remaining shells so completed setup outcomes are
        // not lost; report the first rejection after recording them.
        if (firstError === null) firstError = reason;
      }
    }
    recordOutcomes(next);
    if (firstError !== null) throw firstError;
    return false;
  });

  const ignoreDetected = () => run("detected-ignore", async () => {
    // Persist the dismissed executable so the prompt does not reappear on the
    // next launch; a reinstall at a different path prompts again.
    await tauriInvoke("shell_dismiss_detected", shellDismissDetectedArgs(detected));
    detected.forEach((shell) => onIgnore(shell));
    return true;
  });

  const outcomeFailed = outcomes.some(setupOutcomeFailed);

  return (
    <div className="shell-startup-banners" data-testid="shell-startup-banner">
      {error && <p className="form-error" role="alert">{error}</p>}

      {outcomes.length > 0 && (
        <div
          className={`shell-banner ${outcomeFailed ? "shell-banner-warning" : "shell-banner-info"}`}
          role={outcomeFailed ? "alert" : "status"}
          data-testid="shell-setup-outcomes"
        >
          <div>
            <strong>{t("shellSetup.resultTitle")}</strong>
            {outcomes.map((outcome) => (
              <div className="shell-result-item" key={outcome.shell}>
                <h4>{shellLabel(outcome.shell, language)}</h4>
                <ul>
                  {shellSetupOutcomeLines(outcome, language).map((line) => (
                    <li key={line}>{line}</li>
                  ))}
                </ul>
                {outcome.loaderError && <p className="form-error">{outcome.loaderError}</p>}
                {outcome.syncError && <p className="form-error">{outcome.syncError}</p>}
              </div>
            ))}
          </div>
          <button type="button" onClick={acknowledge} disabled={busy !== null}>
            {t("common.close")}
          </button>
        </div>
      )}

      {notConfigured && (
        <div className="shell-banner shell-banner-info" role="status">
          <div>
            <strong>{t("shellSetup.notConfiguredTitle")}</strong>
            <p className="hint">{t("shellSetup.notConfiguredBody")}</p>
          </div>
          <button type="button" onClick={onOpenSetup}>{t("shellSetup.openSetup")}</button>
        </div>
      )}

      {invalid.map((shell) => (
        <div className="shell-banner shell-banner-warning" role="alert" key={`invalid-${shell}`}>
          <div>
            <strong>{t("shellSetup.invalidTitle")}</strong>
            <p className="hint">{t("shellSetup.invalidBody", { shell: shellLabel(shell, language) })}</p>
          </div>
          <div className="form-actions">
            <button
              type="button"
              className="btn-primary"
              onClick={() => void reenable(shell)}
              disabled={busy !== null}
            >
              {t("shellSetup.reenable")}
            </button>
            <button type="button" onClick={() => onIgnore(shell)} disabled={busy !== null}>
              {t("shellSetup.ignore")}
            </button>
            <button type="button" onClick={() => void disable(shell)} disabled={busy !== null}>
              {t("shellSetup.disableShell")}
            </button>
          </div>
        </div>
      ))}

      {!notConfigured && detected.length > 0 && (
        <div className="shell-banner shell-banner-info" role="status">
          <div>
            <strong>{t("shellSetup.newDetectedTitle")}</strong>
            <p className="hint">
              {t("shellSetup.newDetectedBody", {
                shells: detected.map((shell) => shellLabel(shell, language)).join(", "),
              })}
            </p>
          </div>
          <div className="form-actions">
            <button
              type="button"
              className="btn-primary"
              onClick={() => void enableDetected()}
              disabled={busy !== null}
            >
              {t("shellSetup.enableDetected")}
            </button>
            <button
              type="button"
              onClick={() => void ignoreDetected()}
              disabled={busy !== null}
            >
              {t("shellSetup.ignoreDetected")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default ShellStartupBanner;
