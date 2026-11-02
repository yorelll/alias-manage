import { useRef, useState } from "react";
import {
  tauriInvoke,
  uninstallOutcomeText,
  uninstallRiskSummary,
} from "../lib";
import { useI18n } from "../language";

type Stage = "idle" | "reviewed" | "done";

export function UninstallPanel({ onUninstalled }: { onUninstalled?: () => void }) {
  const { language, t } = useI18n();
  const [purge, setPurge] = useState(false);
  const [stage, setStage] = useState<Stage>("idle");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const requestToken = useRef(0);
  const risk = uninstallRiskSummary(purge, language);

  const reset = () => {
    requestToken.current += 1;
    setStage("idle");
    setError(null);
    setBusy(false);
  };

  const selectMode = (value: boolean) => {
    reset();
    setPurge(value);
  };

  const review = async () => {
    if (stage !== "idle" || busy) return;
    const token = ++requestToken.current;
    setBusy(true);
    setError(null);
    try {
      await tauriInvoke("uninstall_preview", { purge });
      if (requestToken.current !== token) return;
      setStage("reviewed");
    } catch (reason) {
      if (requestToken.current === token) setError(String(reason));
    } finally {
      if (requestToken.current === token) setBusy(false);
    }
  };

  const confirm = async () => {
    // Two-step guard: the destructive command is unreachable until review completed.
    if (stage !== "reviewed" || busy) return;
    const token = ++requestToken.current;
    setBusy(true);
    setError(null);
    try {
      await tauriInvoke("uninstall_confirm", { purge });
      if (requestToken.current !== token) return;
      setStage("done");
      onUninstalled?.();
    } catch (reason) {
      if (requestToken.current === token) setError(String(reason));
    } finally {
      if (requestToken.current === token) setBusy(false);
    }
  };

  return (
    <section className="panel danger-zone" aria-labelledby="uninstall-title">
      <h2 id="uninstall-title">{t("uninstall.title")}</h2>
      <p className="hint">{t("uninstall.hint")}</p>

      <fieldset className="field-group">
        <legend>{t("uninstall.mode")}</legend>
        <label className="check-row">
          <input type="radio" name="uninstall-mode" checked={!purge} onChange={() => selectMode(false)} />
          <span>{t("uninstall.retain")}</span>
        </label>
        <label className="check-row">
          <input type="radio" name="uninstall-mode" checked={purge} onChange={() => selectMode(true)} />
          <span>{t("uninstall.purge")}</span>
        </label>
      </fieldset>

      <div className="risk-summary" aria-label={t("uninstall.riskSummary")}>
        <div className="risk-column">
          <h4>{t("uninstall.kept")}</h4>
          <ul>{risk.kept.map((item) => <li key={item}>{item}</li>)}</ul>
        </div>
        <div className="risk-column">
          <h4>{t("uninstall.removed")}</h4>
          <ul>{risk.removed.map((item) => <li key={item}>{item}</li>)}</ul>
        </div>
      </div>
      <p className="boundary-note">{risk.protected.join(" ")}</p>

      {stage === "reviewed" && (
        <div className="review-block">
          <h3>{t("uninstall.reviewTitle")}</h3>
          <ul>{risk.removed.map((item) => <li key={item}>{item}</li>)}</ul>
          <p className="hint">{t("uninstall.keptInline", { items: risk.kept.join("; ") })}</p>
          <p className="form-error" role="alert">
            {t(purge ? "uninstall.confirmPurgeHint" : "uninstall.confirmUninstallHint")}
          </p>
        </div>
      )}

      {stage === "done" && (
        <div className="review-block" role="status">
          <h3>{t("uninstall.cleanupComplete")}</h3>
          <p>{uninstallOutcomeText(purge, language)}</p>
          <p className="hint">{risk.protected.join(" ")}</p>
        </div>
      )}

      {error && <p className="form-error" role="alert">{error}</p>}

      <div className="form-actions">
        {stage === "reviewed" && <button type="button" onClick={reset} disabled={busy}>{t("common.cancel")}</button>}
        {stage === "done" && <button type="button" onClick={reset}>{t("uninstall.startOver")}</button>}
        {stage === "idle" && (
          <button type="button" onClick={review} disabled={busy}>
            {busy ? t("uninstall.loadingImpact") : t(purge ? "uninstall.reviewPurge" : "uninstall.reviewUninstall")}
          </button>
        )}
        {stage === "reviewed" && (
          <button type="button" className="btn-danger" onClick={confirm} disabled={busy}>
            {busy ? t("uninstall.working") : t(purge ? "uninstall.confirmPurge" : "uninstall.confirmUninstall")}
          </button>
        )}
      </div>
    </section>
  );
}

export default UninstallPanel;
