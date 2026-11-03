import { useState } from "react";
import type { ShellStartupState } from "../lib";
import {
  shellEnableArgs,
  shellLabel,
  shellSetupArgs,
  setupErrorMessage,
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
 * detected shells here. "Ignore" is session-only; "Disable" persists intent.
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

  const invalid = state.invalidEnabled.filter((shell) => !ignored.includes(shell));
  const detected = state.newlyDetected.filter((shell) => !ignored.includes(shell));
  const notConfigured = state.shells.length > 0 && state.shells.every((status) => !status.enabled);

  if (state.firstRun) return null;
  if (invalid.length === 0 && detected.length === 0 && !notConfigured) return null;

  const run = async (key: string, action: () => Promise<void>) => {
    setBusy(key);
    setError(null);
    try {
      await action();
      onChanged();
    } catch (reason) {
      setError(t("shellSetup.bannerError", { error: setupErrorMessage(reason, language) }));
    } finally {
      setBusy(null);
    }
  };

  const disable = (shell: string) => run(`disable-${shell}`, async () => {
    const remaining = state.shells.filter((status) => status.enabled && status.shell !== shell).map((status) => status.shell);
    await tauriInvoke("shell_set_enabled", shellEnableArgs(remaining));
  });

  const reenable = (shell: string) => run(`enable-${shell}`, async () => {
    await tauriInvoke("shell_setup_apply", shellSetupArgs(shell));
  });

  const enableDetected = () => run("detected", async () => {
    for (const shell of detected) {
      await tauriInvoke("shell_setup_apply", shellSetupArgs(shell));
    }
  });

  return (
    <div className="shell-startup-banners" data-testid="shell-startup-banner">
      {error && <p className="form-error" role="alert">{error}</p>}

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
              onClick={() => detected.forEach((shell) => onIgnore(shell))}
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
