import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ShellPathOverride,
  ShellSetupOutcome,
  ShellSetupPlan,
  ShellStatus,
} from "../lib";
import {
  normalizeShellSetupOutcome,
  normalizeShellSetupPlan,
  normalizeShellStatuses,
  shellLabel,
  shellLoaderLabel,
  shellOptionState,
  shellOptionStateLabel,
  shellSetShellEnabledArgs,
  shellSetupArgs,
  shellSetupOutcomeLines,
  shellSourceLabel,
  setupErrorMessage,
  tauriInvoke,
} from "../lib";
import { useI18n } from "../language";

interface ShellSettingsPanelProps {
  /** Bumped by "Open shell setup" so the panel scrolls into view and focuses. */
  focusToken?: number;
  /** Notify the app so startup banners refresh after enable/disable/setup. */
  onChanged?: () => void;
  /** Status rows already loaded by App; avoids a second startup probe. */
  initialStatuses?: ShellStatus[];
  /** Only fetch when the Settings page is visible. */
  active?: boolean;
}

/** Settings panel for enable/disable, rescan, path overrides and re-setup. */
export function ShellSettingsPanel({
  focusToken = 0,
  onChanged,
  initialStatuses = [],
  active = true,
}: ShellSettingsPanelProps) {
  const { language, t } = useI18n();
  const [statuses, setStatuses] = useState<ShellStatus[]>(() => normalizeShellStatuses(initialStatuses));
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [overrides, setOverrides] = useState<Record<string, ShellPathOverride>>({});
  const [plan, setPlan] = useState<ShellSetupPlan | null>(null);
  const [outcome, setOutcome] = useState<ShellSetupOutcome | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setStatuses(normalizeShellStatuses(await tauriInvoke("shell_status")));
    } catch (reason) {
      setError(t("shellSetup.rescanFailed", { error: String(reason) }));
    } finally {
      setLoading(false);
    }
  }, [t]);

  // Adopt every fresh App startup/banner refresh (new array reference), not
  // just the first one, so wizard/banner actions are reflected without a
  // manual Rescan. Postpone while the user is editing or an action is running.
  useEffect(() => {
    if (initialStatuses.length === 0) return;
    if (editing !== null || busy !== null) return;
    setStatuses(normalizeShellStatuses(initialStatuses));
    setLoading(false);
  }, [initialStatuses, editing, busy]);

  useEffect(() => {
    if (active && statuses.length === 0) {
      void load();
    }
  }, [active, statuses.length, load]);

  useEffect(() => {
    if (focusToken > 0) {
      headingRef.current?.scrollIntoView?.({ block: "start" });
      headingRef.current?.focus?.();
      if (statuses.length === 0) void load();
    }
  }, [focusToken, statuses.length, load]);

  const replaceStatuses = (next: ShellStatus[]) => {
    setStatuses(normalizeShellStatuses(next));
    setEditing(null);
    setPlan(null);
    setOutcome(null);
  };

  const disable = async (shell: string) => {
    setBusy(`disable-${shell}`);
    setError(null);
    setNotice(null);
    try {
      // Per-shell toggle re-reads shared config, so shells enabled by another
      // entry point are preserved and `shells.seen` suppresses the prompt.
      const next = await tauriInvoke(
        "shell_set_shell_enabled",
        shellSetShellEnabledArgs(shell, false),
      );
      replaceStatuses(next as ShellStatus[]);
      setNotice(t("shellSetup.status.disabled"));
      onChanged?.();
    } catch (reason) {
      setError(setupErrorMessage(reason, language));
    } finally {
      setBusy(null);
    }
  };

  const preview = async (shell: string, override: ShellPathOverride) => {
    setBusy(`preview-${shell}`);
    setError(null);
    setNotice(null);
    setPlan(null);
    setOutcome(null);
    try {
      setPlan(normalizeShellSetupPlan(
        await tauriInvoke("shell_setup_plan", shellSetupArgs(shell, override)),
      ));
    } catch (reason) {
      setError(t("shellSetup.applyFailed", { error: setupErrorMessage(reason, language) }));
    } finally {
      setBusy(null);
    }
  };

  const startEditing = (status: ShellStatus) => {
    // Empty fields mean "use the detected/default path"; only typed values are
    // persisted as overrides. Placeholders show the current resolution.
    const override: ShellPathOverride = { executable: "", profile: "" };
    setEditing(status.shell);
    setOverrides((current) => ({ ...current, [status.shell]: override }));
    void preview(status.shell, override);
  };

  /** Explicitly remove a stored override and preview the platform default. */
  const clearOverride = (shell: string, field: "executable" | "profile") => {
    // Flags (not the sentinel string) drive the UI, so the visible field stays
    // blank and the default/profile hint reflects the effective config.
    const next: ShellPathOverride = field === "executable"
      ? { ...overrides[shell], executable: "", clearExecutable: true }
      : { ...overrides[shell], profile: "", clearProfile: true };
    setOverrides((current) => ({ ...current, [shell]: next }));
    setPlan(null);
    void preview(shell, next);
  };

  const apply = async (shell: string) => {
    setBusy(`apply-${shell}`);
    setError(null);
    setNotice(null);
    try {
      const result = normalizeShellSetupOutcome(
        await tauriInvoke("shell_setup_apply", shellSetupArgs(shell, overrides[shell])),
      );
      setOutcome(result);
      await load();
      setNotice(t("shellSetup.resultConfigSaved", { shell: shellLabel(shell, language) }));
      onChanged?.();
    } catch (reason) {
      setError(t("shellSetup.applyFailed", { error: setupErrorMessage(reason, language) }));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="panel shell-settings" aria-labelledby="shell-settings-title" data-testid="shell-settings-panel">
      <div className="panel-header">
        <h2 id="shell-settings-title" ref={headingRef} tabIndex={-1}>{t("shellSetup.title")}</h2>
        <button type="button" onClick={() => void load()} disabled={loading || busy !== null}>
          {loading ? t("shellSetup.scanning") : t("shellSetup.rescan")}
        </button>
      </div>
      <p className="hint">{t("shellSetup.settingsHint")}</p>

      {notice && <p role="status">{notice}</p>}
      {error && <p className="form-error" role="alert">{error}</p>}
      {loading && statuses.length === 0 && <p role="status">{t("shellSetup.scanning")}</p>}

      <ul className="shell-list">
        {statuses.map((status) => {
          const state = shellOptionState(status);
          const override = overrides[status.shell] ?? {};
          const isEditing = editing === status.shell;
          return (
            <li className="shell-row" key={status.shell} data-shell={status.shell}>
              <div className="shell-row-header">
                <strong>{shellLabel(status.shell, language)}</strong>
                <span className={`badge badge-${state}`}>{shellOptionStateLabel(state, language)}</span>
                <span className="hint">{shellLoaderLabel(status.loader_status, language)}</span>
              </div>
              <p className="hint">
                {status.detected
                  ? `${status.executable ?? ""} · ${shellSourceLabel(status.source, language)}${
                      status.version ? ` · ${t("shellSetup.version")} ${status.version}` : ""
                    }`
                  : t("shellSetup.notDetected")}
              </p>
              <p className="hint">
                {status.loader_target
                  ? t("shellSetup.loaderTarget", { path: status.loader_target })
                  : t("shellSetup.noLoaderTarget")}
              </p>
              {status.wsl && <p className="hint warning">{t("shellSetup.wslHint")}</p>}

              <div className="form-actions">
                {status.enabled ? (
                  <button
                    type="button"
                    onClick={() => void disable(status.shell)}
                    disabled={busy !== null}
                  >
                    {t("shellSetup.disable")}
                  </button>
                ) : (
                  <button
                    type="button"
                    className="btn-primary"
                    onClick={() => startEditing(status)}
                    disabled={busy !== null}
                  >
                    {t("shellSetup.enable")}
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => startEditing(status)}
                  disabled={busy !== null}
                >
                  {t("shellSetup.update")}
                </button>
              </div>

              {isEditing && (
                <div className="shell-path-editor" data-testid={`shell-editor-${status.shell}`}>
                  <div className="field-grid">
                    <label className="field">
                      <span>{t("shellSetup.executable")}</span>
                      <input
                        value={override.executable ?? ""}
                        placeholder={status.executable ?? ""}
                        onChange={(event) => setOverrides((current) => ({
                          ...current,
                          [status.shell]: {
                            ...current[status.shell],
                            executable: event.target.value,
                            clearExecutable: false,
                          },
                        }))}
                        onBlur={() => setPlan(null)}
                        autoComplete="off"
                        spellCheck={false}
                      />
                    </label>
                    <label className="field">
                      <span>{t("shellSetup.profile")}</span>
                      <input
                        value={override.profile ?? ""}
                        placeholder={status.loader_target ?? plan?.defaultProfile ?? ""}
                        onChange={(event) => setOverrides((current) => ({
                          ...current,
                          [status.shell]: {
                            ...current[status.shell],
                            profile: event.target.value,
                            clearProfile: false,
                          },
                        }))}
                        onBlur={() => setPlan(null)}
                        autoComplete="off"
                        spellCheck={false}
                      />
                    </label>
                    <p className="hint field-full">
                      {t("shellSetup.useDefault")}
                      {plan?.defaultProfile ? ` ${t("shellSetup.defaultProfile", { path: plan.defaultProfile })}` : ""}
                    </p>
                    <div className="form-actions field-full">
                      <button
                        type="button"
                        onClick={() => clearOverride(status.shell, "executable")}
                        disabled={busy !== null}
                      >
                        {t("shellSetup.clearExecutable")}
                      </button>
                      <button
                        type="button"
                        onClick={() => clearOverride(status.shell, "profile")}
                        disabled={busy !== null}
                      >
                        {t("shellSetup.clearProfile")}
                      </button>
                    </div>
                    {(override.clearExecutable || override.clearProfile) && (
                      <p className="hint field-full" role="status">
                        {t("shellSetup.overrideCleared")}
                        {plan?.defaultProfile ? ` ${t("shellSetup.defaultProfile", { path: plan.defaultProfile })}` : ""}
                      </p>
                    )}
                  </div>

                  {plan && plan.shell === status.shell && (
                    <div className="shell-preview">
                      <p>
                        <strong>{t("shellSetup.target")}:</strong>{" "}
                        <code>{plan.previewTarget ?? t("shellSetup.noLoaderTarget")}</code>
                      </p>
                      {plan.loaderBlock
                        ? <pre className="code-block">{plan.loaderBlock}</pre>
                        : <p className="hint">{t("shellSetup.noPreview")}</p>}
                    </div>
                  )}

                  {outcome && outcome.shell === status.shell && (
                    <div className="shell-result">
                      <ul>
                        {shellSetupOutcomeLines(outcome, language).map((line) => (
                          <li key={line}>{line}</li>
                        ))}
                      </ul>
                      {outcome.loaderError && (
                        <p className="form-error" role="alert">{t("shellSetup.loaderNotInstalled")}</p>
                      )}
                    </div>
                  )}

                  <div className="form-actions">
                    <button
                      type="button"
                      onClick={() => setEditing(null)}
                      disabled={busy !== null}
                    >
                      {t("common.cancel")}
                    </button>
                    <button
                      type="button"
                      onClick={() => void preview(status.shell, overrides[status.shell] ?? {})}
                      disabled={busy !== null}
                    >
                      {busy === `preview-${status.shell}` ? t("shellSetup.previewing") : t("shellSetup.preview")}
                    </button>
                    <button
                      type="button"
                      className="btn-primary"
                      onClick={() => void apply(status.shell)}
                      disabled={busy !== null || !plan || plan.shell !== status.shell}
                    >
                      {busy === `apply-${status.shell}` ? t("shellSetup.applying") : t("shellSetup.apply")}
                    </button>
                  </div>
                </div>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

export default ShellSettingsPanel;
