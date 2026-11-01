import { useRef, useState } from "react";
import {
  toStringList,
  tauriInvoke,
  uninstallRiskSummary,
} from "../lib";

type Stage = "idle" | "reviewed" | "done";

export function UninstallPanel() {
  const [purge, setPurge] = useState(false);
  const [stage, setStage] = useState<Stage>("idle");
  const [previewItems, setPreviewItems] = useState<string[]>([]);
  const [resultItems, setResultItems] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const requestToken = useRef(0);
  const risk = uninstallRiskSummary(purge);

  const reset = () => {
    requestToken.current += 1;
    setStage("idle");
    setPreviewItems([]);
    setResultItems([]);
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
      const items = toStringList(await tauriInvoke("uninstall_preview", { purge }));
      if (requestToken.current !== token) return;
      setPreviewItems(items);
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
      const items = toStringList(await tauriInvoke("uninstall_confirm", { purge }));
      if (requestToken.current !== token) return;
      setResultItems(items);
      setStage("done");
    } catch (reason) {
      if (requestToken.current === token) setError(String(reason));
    } finally {
      if (requestToken.current === token) setBusy(false);
    }
  };

  return (
    <section className="panel danger-zone" aria-labelledby="uninstall-title">
      <h2 id="uninstall-title">Uninstall</h2>
      <p className="hint">
        Uninstalling always requires two steps: review the exact impact, then confirm. Nothing is removed on a single click.
      </p>

      <fieldset className="field-group">
        <legend>Mode</legend>
        <label className="check-row">
          <input type="radio" name="uninstall-mode" checked={!purge} onChange={() => selectMode(false)} />
          <span>Retain aliases</span>
        </label>
        <label className="check-row">
          <input type="radio" name="uninstall-mode" checked={purge} onChange={() => selectMode(true)} />
          <span>Purge aliases</span>
        </label>
      </fieldset>

      <div className="risk-summary" aria-label="Risk summary">
        <div className="risk-column">
          <h4>Kept</h4>
          <ul>{risk.kept.map((item) => <li key={item}>{item}</li>)}</ul>
        </div>
        <div className="risk-column">
          <h4>Removed</h4>
          <ul>{risk.removed.map((item) => <li key={item}>{item}</li>)}</ul>
        </div>
      </div>
      <p className="boundary-note">{risk.protected.join(" ")}</p>

      {stage === "reviewed" && previewItems.length > 0 && (
        <div className="review-block">
          <h3>Review: items affected</h3>
          <ul>{previewItems.map((item, index) => <li key={`${item}-${index}`}>{item}</li>)}</ul>
          <p className="form-error" role="alert">
            {purge ? "Confirm purge to continue." : "Confirm uninstall to continue."}
          </p>
        </div>
      )}

      {stage === "done" && (
        <div className="review-block" role="status">
          <h3>Uninstall cleanup complete</h3>
          {resultItems.length > 0
            ? <ul>{resultItems.map((item, index) => <li key={`${item}-${index}`}>{item}</li>)}</ul>
            : <p>No further details were returned.</p>}
        </div>
      )}

      {error && <p className="form-error" role="alert">{error}</p>}

      <div className="form-actions">
        {stage === "reviewed" && <button type="button" onClick={reset} disabled={busy}>Cancel</button>}
        {stage === "done" && <button type="button" onClick={reset}>Start over</button>}
        {stage === "idle" && (
          <button type="button" onClick={review} disabled={busy}>
            {busy ? "Loading impact…" : purge ? "Review purge" : "Review uninstall"}
          </button>
        )}
        {stage === "reviewed" && (
          <button type="button" className="btn-danger" onClick={confirm} disabled={busy}>
            {busy ? "Working…" : purge ? "Confirm purge" : "Confirm uninstall"}
          </button>
        )}
      </div>
    </section>
  );
}

export default UninstallPanel;
