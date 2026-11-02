import { useEffect, useRef, useState } from "react";
import type { AliasDto, EnvRow, PathRestriction, PreviewSnippet, RestrictionMode } from "../lib";
import {
  SHELLS,
  TARGET_TYPES,
  aliasArgvPreview,
  emptyAliasDraft,
  environmentToRows,
  finalizeAliasDraft,
  fixedArgsPlaceholderHint,
  foldRestrictionPath,
  latestPreviewResponse,
  normalizePreviewSnippets,
  previewAliasArgs,
  rowsToEnvironment,
  shellLabel,
  targetExecutableWarning,
  targetTypeExplanation,
  targetTypeLabel,
  tauriInvoke,
  toggleShell,
  validateAliasDraft,
} from "../lib";
import { useI18n } from "../language";
import { TagInput } from "./TagInput";

interface AliasWizardProps {
  initial?: AliasDto;
  tags: string[];
  onCancel: () => void;
  onSave: (alias: AliasDto) => Promise<string | null>;
}

export function AliasWizard({ initial, tags, onCancel, onSave }: AliasWizardProps) {
  const { language, t } = useI18n();
  const [draft, setDraft] = useState<AliasDto>(initial ?? emptyAliasDraft());
  const [envRows, setEnvRows] = useState<EnvRow[]>(() => environmentToRows((initial ?? emptyAliasDraft()).environment));
  const [advanced, setAdvanced] = useState(() => Boolean(initial && (
    initial.working_directory || initial.fixed_args.length > 0 || Object.keys(initial.environment).length > 0
  )));
  const [error, setError] = useState<string | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previews, setPreviews] = useState<PreviewSnippet[] | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [restrictionPath, setRestrictionPath] = useState("");
  const previewToken = useRef(0);
  const nameRef = useRef<HTMLInputElement | null>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    nameRef.current?.focus();
    return () => {
      const opener = returnFocusRef.current;
      if (opener?.isConnected && opener !== document.body) {
        opener.focus();
      } else {
        document.querySelector<HTMLElement>('[data-testid="aliases-page"]')?.focus();
      }
    };
  }, []);

  // Localized validation/save/preview messages must not survive a language switch.
  useEffect(() => { setError(null); setPreviewError(null); }, [language]);

  const invalidatePreview = () => {
    previewToken.current += 1;
    setPreviews(null);
    setPreviewError(null);
    setPreviewing(false);
  };

  const update = (patch: Partial<AliasDto>) => {
    setDraft((value) => ({ ...value, ...patch }));
    setError(null);
    invalidatePreview();
  };

  const updateEnvRows = (rows: EnvRow[]) => {
    setEnvRows(rows);
    update({ environment: rowsToEnvironment(rows) });
  };

  const currentDraft = () => foldRestrictionPath(
    { ...draft, environment: rowsToEnvironment(envRows) },
    restrictionPath,
  );

  const restriction = draft.restriction ?? null;
  const setRestriction = (next: PathRestriction | null) => update({ restriction: next });
  const addRestrictionPath = () => {
    const path = restrictionPath.trim();
    if (!restriction || !path) return;
    if (!restriction.paths.includes(path)) {
      setRestriction({ ...restriction, paths: [...restriction.paths, path] });
    }
    setRestrictionPath("");
  };

  const save = async () => {
    const finalized = finalizeAliasDraft(currentDraft());
    const errors = validateAliasDraft(finalized, language);
    if (errors.length > 0) {
      setError(errors.join(" "));
      return;
    }
    setSaving(true);
    setError(null);
    const result = await onSave(finalized);
    setSaving(false);
    if (result) setError(result);
  };

  const runPreview = async () => {
    const token = ++previewToken.current;
    const finalized = finalizeAliasDraft(currentDraft());
    const errors = validateAliasDraft(finalized, language);
    if (errors.length > 0) {
      setPreviewing(false);
      setPreviewError(errors.join(" "));
      setPreviews(null);
      return;
    }
    setPreviewing(true);
    setPreviewError(null);
    setPreviews(null);
    try {
      const result = await tauriInvoke("preview_alias", previewAliasArgs(finalized));
      const snippets = normalizePreviewSnippets(result);
      const latest = latestPreviewResponse(previewToken.current, token, snippets);
      if (latest) setPreviews(latest);
    } catch (reason) {
      if (previewToken.current === token) setPreviewError(String(reason));
    } finally {
      if (previewToken.current === token) setPreviewing(false);
    }
  };

  const unknownShells = draft.shells.filter((shell) => !(SHELLS as readonly string[]).includes(shell));
  const targetTypeHint = targetTypeExplanation(language);
  const executableWarning = targetExecutableWarning(draft, language);
  const argvPreview = aliasArgvPreview(draft, t("wizard.argvUserArgs"));

  return (
    <section className="alias-wizard" aria-labelledby="alias-wizard-title" data-testid="alias-wizard">
      <h2 id="alias-wizard-title">{initial ? t("wizard.editTitle", { name: initial.name }) : t("wizard.addTitle")}</h2>

      {error && <p className="form-error" role="alert">{t("wizard.unableToSave", { error })}</p>}

      <div className="field-grid">
        <label className="field">
          <span>{t("wizard.name")}</span>
          <input
            ref={nameRef}
            value={draft.name}
            onChange={(event) => update({ name: event.target.value })}
            placeholder={t("wizard.namePlaceholder")}
            autoComplete="off"
            aria-invalid={Boolean(error) && !/^[A-Za-z_][A-Za-z0-9_-]{0,63}$/.test(draft.name)}
          />
        </label>

        <label className="field">
          <span title={targetTypeHint}>{t("wizard.targetType")}</span>
          <select value={draft.target_type} onChange={(event) => update({ target_type: event.target.value })}>
            {TARGET_TYPES.map((option) => <option key={option.value} value={option.value}>{targetTypeLabel(option.value, language)}</option>)}
          </select>
          {!TARGET_TYPES.some((option) => option.value === draft.target_type) && (
            <span className="hint">
              {t("wizard.unsupportedType", { type: targetTypeLabel(draft.target_type, language) })}
            </span>
          )}
          <span className="hint" title={targetTypeHint}>{targetTypeHint}</span>
        </label>

        <label className="field field-full">
          <span>{t("wizard.targetLabel")}</span>
          <input
            value={draft.executable}
            onChange={(event) => update({ executable: event.target.value })}
            placeholder={t("wizard.targetPlaceholder")}
            autoComplete="off"
            spellCheck={false}
            aria-describedby={executableWarning ? "target-executable-warning" : undefined}
          />
          {executableWarning && (
            <span id="target-executable-warning" className="hint warning" role="status">
              {executableWarning}
            </span>
          )}
          <span className="hint" data-testid="argv-preview">
            {t("wizard.argvPreview")}: <code>{JSON.stringify(argvPreview)}</code>
          </span>
        </label>

        <label className="field field-full">
          <span>{t("wizard.description")}</span>
          <textarea value={draft.description} onChange={(event) => update({ description: event.target.value })} rows={2} />
        </label>

        <fieldset className="field-group field-full">
          <legend>{t("wizard.shells")}</legend>
          <div className="check-group">
            {SHELLS.map((shell) => (
              <label className="check-row" key={shell}>
                <input
                  type="checkbox"
                  checked={draft.shells.includes(shell)}
                  onChange={() => update({ shells: toggleShell(draft.shells, shell) })}
                />
                <span>{shellLabel(shell, language)}</span>
              </label>
            ))}
          </div>
          {unknownShells.length > 0 && (
            <p className="hint">{t("wizard.alsoTargets", { shells: unknownShells.map((shell) => shellLabel(shell, language)).join(", ") })}</p>
          )}
        </fieldset>

        <label className="check-row field-full">
          <input type="checkbox" checked={draft.pass_args} onChange={(event) => update({ pass_args: event.target.checked })} />
          <span>{t("wizard.passArgs")}</span>
        </label>

        <div className="field field-full">
          <TagInput label={t("wizard.tags")} tags={draft.tags} suggestions={tags} onChange={(next) => update({ tags: next })} />
        </div>

        <label className="check-row field-full">
          <input
            type="checkbox"
            checked={restriction !== null}
            onChange={(event) => {
              if (!event.target.checked) setRestrictionPath("");
              update({
                restriction: event.target.checked
                  ? { mode: "whitelist", paths: [] }
                  : null,
              });
            }}
          />
          <span>{t("wizard.restriction")}</span>
        </label>

        {restriction && (
          <div className="field field-full restriction-editor" data-testid="restriction-editor">
            <p className="hint">{t("wizard.restrictionHint")}</p>
            <label className="field">
              <span>{t("wizard.restrictionMode")}</span>
              <select
                value={restriction.mode}
                onChange={(event) => setRestriction({
                  ...restriction,
                  mode: event.target.value as RestrictionMode,
                })}
              >
                <option value="whitelist">{t("wizard.restrictionWhitelist")}</option>
                <option value="blacklist">{t("wizard.restrictionBlacklist")}</option>
              </select>
            </label>
            <div className="field">
              <span>{t("wizard.restrictionPaths")}</span>
              <div className="row-editor">
                {restriction.paths.length === 0 && <p className="hint">{t("wizard.noRestrictionPaths")}</p>}
                {restriction.paths.map((path, index) => (
                  <div className="row-editor-row" key={path}>
                    <code className="restriction-path">{path}</code>
                    <button
                      type="button"
                      onClick={() => setRestriction({
                        ...restriction,
                        paths: restriction.paths.filter((_, position) => position !== index),
                      })}
                    >
                      {t("common.remove")}
                    </button>
                  </div>
                ))}
                <div className="row-editor-row">
                  <input
                    aria-label={t("wizard.restrictionPathAria", { index: restriction.paths.length + 1 })}
                    value={restrictionPath}
                    onChange={(event) => setRestrictionPath(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        addRestrictionPath();
                      }
                    }}
                    placeholder={t("wizard.restrictionPathPlaceholder")}
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <button type="button" onClick={addRestrictionPath}>
                    {t("wizard.addRestrictionPath")}
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      <button
        type="button"
        className="btn-secondary advanced-toggle"
        aria-expanded={advanced}
        aria-controls="alias-wizard-advanced"
        onClick={() => setAdvanced((value) => !value)}
      >
        {advanced ? t("wizard.hideAdvanced") : t("wizard.showAdvanced")}
      </button>

      {advanced && (
        <div id="alias-wizard-advanced" className="advanced-section">
          <div className="field-grid">
            <label className="field field-full">
              <span>{t("wizard.workingDirectory")}</span>
              <input
                value={draft.working_directory ?? ""}
                onChange={(event) => update({ working_directory: event.target.value.trim() ? event.target.value : null })}
                placeholder={t("wizard.cwdPlaceholder")}
                autoComplete="off"
                spellCheck={false}
              />
            </label>
          </div>

          <div className="field">
            <span>{t("wizard.fixedArguments")}</span>
            <p className="hint">{fixedArgsPlaceholderHint(language)}</p>
            <div className="row-editor">
              {draft.fixed_args.length === 0 && <p className="hint">{t("wizard.noFixedArguments")}</p>}
              {draft.fixed_args.map((argument, index) => (
                <div className="row-editor-row" key={index}>
                  <input
                    aria-label={t("wizard.fixedArgumentAria", { index: index + 1 })}
                    value={argument}
                    onChange={(event) => {
                      const next = [...draft.fixed_args];
                      next[index] = event.target.value;
                      update({ fixed_args: next });
                    }}
                    placeholder={t("wizard.argumentPlaceholder")}
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <button
                    type="button"
                    onClick={() => update({ fixed_args: draft.fixed_args.filter((_, position) => position !== index) })}
                  >
                    {t("common.remove")}
                  </button>
                </div>
              ))}
              <button type="button" onClick={() => update({ fixed_args: [...draft.fixed_args, ""] })}>
                {t("wizard.addArgument")}
              </button>
            </div>
          </div>

          <div className="field">
            <span>{t("wizard.environmentVariables")}</span>
            <div className="row-editor">
              {envRows.length === 0 && <p className="hint">{t("wizard.noEnvironmentVariables")}</p>}
              {envRows.map((row, index) => (
                <div className="row-editor-row" key={index}>
                  <input
                    aria-label={t("wizard.envKeyAria", { index: index + 1 })}
                    value={row.key}
                    onChange={(event) => {
                      const next = [...envRows];
                      next[index] = { ...next[index], key: event.target.value };
                      updateEnvRows(next);
                    }}
                    placeholder={t("wizard.envKeyPlaceholder")}
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <input
                    aria-label={t("wizard.envValueAria", { index: index + 1 })}
                    value={row.value}
                    onChange={(event) => {
                      const next = [...envRows];
                      next[index] = { ...next[index], value: event.target.value };
                      updateEnvRows(next);
                    }}
                    placeholder={t("wizard.envValuePlaceholder")}
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <button
                    type="button"
                    onClick={() => updateEnvRows(envRows.filter((_, position) => position !== index))}
                  >
                    {t("common.remove")}
                  </button>
                </div>
              ))}
              <button type="button" onClick={() => updateEnvRows([...envRows, { key: "", value: "" }])}>
                {t("wizard.addVariable")}
              </button>
            </div>
          </div>
        </div>
      )}

      <div className="preview-block" aria-live="polite">
        <div className="preview-header">
          <h3>{t("wizard.preview")}</h3>
          <button type="button" onClick={runPreview} disabled={previewing}>
            {previewing ? t("wizard.rendering") : t("wizard.preview")}
          </button>
        </div>
        <p className="hint">{t("wizard.previewHint")}</p>
        {previewError && <p className="form-error" role="alert">{t("wizard.previewFailed", { error: previewError })}</p>}
        {previews && previews.length === 0 && <p className="hint">{t("wizard.noPreview")}</p>}
        {previews?.map((snippet, index) => (
          <section className="preview-snippet" key={`${snippet.shell}-${index}`}>
            <h4>{snippet.shell ? shellLabel(snippet.shell, language) : t("wizard.shellFallback")}</h4>
            {snippet.error
              ? <p className="form-error" role="alert">{snippet.error}</p>
              : <pre className="code-block">{snippet.content}</pre>}
          </section>
        ))}
      </div>

      <div className="wizard-actions">
        <button type="button" onClick={onCancel} disabled={saving}>
          {saving ? t("common.saving") : t("common.cancel")}
        </button>
        <button type="button" className="btn-primary" onClick={save} disabled={saving} aria-busy={saving}>
          {saving ? t("common.saving") : t("common.save")}
        </button>
      </div>
    </section>
  );
}
