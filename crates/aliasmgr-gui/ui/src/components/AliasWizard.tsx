import { useEffect, useRef, useState } from "react";
import type { AliasDto, EnvRow, PreviewSnippet } from "../lib";
import {
  FIXED_ARGS_PLACEHOLDER_HINT,
  SHELLS,
  TARGET_TYPES,
  emptyAliasDraft,
  environmentToRows,
  finalizeAliasDraft,
  latestPreviewResponse,
  normalizePreviewSnippets,
  previewAliasArgs,
  rowsToEnvironment,
  shellLabel,
  tauriInvoke,
  toggleShell,
  validateAliasDraft,
} from "../lib";
import { TagInput } from "./TagInput";

interface AliasWizardProps {
  initial?: AliasDto;
  tags: string[];
  onCancel: () => void;
  onSave: (alias: AliasDto) => Promise<string | null>;
}

export function AliasWizard({ initial, tags, onCancel, onSave }: AliasWizardProps) {
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

  const currentDraft = () => ({ ...draft, environment: rowsToEnvironment(envRows) });

  const save = async () => {
    const finalized = finalizeAliasDraft(currentDraft());
    const errors = validateAliasDraft(finalized);
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
    const errors = validateAliasDraft(finalized);
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

  return (
    <section className="alias-wizard" aria-labelledby="alias-wizard-title" data-testid="alias-wizard">
      <h2 id="alias-wizard-title">{initial ? `Edit alias: ${initial.name}` : "Add alias"}</h2>

      {error && <p className="form-error" role="alert">Unable to save: {error}</p>}

      <div className="field-grid">
        <label className="field">
          <span>Name</span>
          <input
            ref={nameRef}
            value={draft.name}
            onChange={(event) => update({ name: event.target.value })}
            placeholder="my-alias"
            autoComplete="off"
            aria-invalid={Boolean(error) && !/^[A-Za-z_][A-Za-z0-9_-]{0,63}$/.test(draft.name)}
          />
        </label>

        <label className="field">
          <span>Target type</span>
          <select value={draft.target_type} onChange={(event) => update({ target_type: event.target.value })}>
            {TARGET_TYPES.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
          {!TARGET_TYPES.some((option) => option.value === draft.target_type) && (
            <span className="hint">
              This alias uses “{draft.target_type}”, which cannot be edited here. Choose a supported target type to save.
            </span>
          )}
        </label>

        <label className="field field-full">
          <span>Target (executable or command)</span>
          <input
            value={draft.executable}
            onChange={(event) => update({ executable: event.target.value })}
            placeholder="git"
            autoComplete="off"
            spellCheck={false}
          />
        </label>

        <label className="field field-full">
          <span>Description</span>
          <textarea value={draft.description} onChange={(event) => update({ description: event.target.value })} rows={2} />
        </label>

        <fieldset className="field-group field-full">
          <legend>Shells</legend>
          <div className="check-group">
            {SHELLS.map((shell) => (
              <label className="check-row" key={shell}>
                <input
                  type="checkbox"
                  checked={draft.shells.includes(shell)}
                  onChange={() => update({ shells: toggleShell(draft.shells, shell) })}
                />
                <span>{shellLabel(shell)}</span>
              </label>
            ))}
          </div>
          {unknownShells.length > 0 && (
            <p className="hint">Also targets (preserved): {unknownShells.map(shellLabel).join(", ")}</p>
          )}
        </fieldset>

        <label className="check-row field-full">
          <input type="checkbox" checked={draft.pass_args} onChange={(event) => update({ pass_args: event.target.checked })} />
          <span>Pass user arguments to the target</span>
        </label>

        <div className="field field-full">
          <TagInput label="Tags" tags={draft.tags} suggestions={tags} onChange={(next) => update({ tags: next })} />
        </div>
      </div>

      <button
        type="button"
        className="btn-secondary advanced-toggle"
        aria-expanded={advanced}
        aria-controls="alias-wizard-advanced"
        onClick={() => setAdvanced((value) => !value)}
      >
        {advanced ? "Hide advanced" : "Show advanced"}
      </button>

      {advanced && (
        <div id="alias-wizard-advanced" className="advanced-section">
          <div className="field-grid">
            <label className="field field-full">
              <span>Working directory</span>
              <input
                value={draft.working_directory ?? ""}
                onChange={(event) => update({ working_directory: event.target.value.trim() ? event.target.value : null })}
                placeholder="C:\\projects\\example"
                autoComplete="off"
                spellCheck={false}
              />
            </label>
          </div>

          <div className="field">
            <span>Fixed arguments</span>
            <p className="hint">{FIXED_ARGS_PLACEHOLDER_HINT}</p>
            <div className="row-editor">
              {draft.fixed_args.length === 0 && <p className="hint">No fixed arguments.</p>}
              {draft.fixed_args.map((argument, index) => (
                <div className="row-editor-row" key={index}>
                  <input
                    aria-label={`Fixed argument ${index + 1}`}
                    value={argument}
                    onChange={(event) => {
                      const next = [...draft.fixed_args];
                      next[index] = event.target.value;
                      update({ fixed_args: next });
                    }}
                    placeholder={"argument or {{args}}"}
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <button
                    type="button"
                    onClick={() => update({ fixed_args: draft.fixed_args.filter((_, position) => position !== index) })}
                  >
                    Remove
                  </button>
                </div>
              ))}
              <button type="button" onClick={() => update({ fixed_args: [...draft.fixed_args, ""] })}>
                Add argument
              </button>
            </div>
          </div>

          <div className="field">
            <span>Environment variables</span>
            <div className="row-editor">
              {envRows.length === 0 && <p className="hint">No environment variables.</p>}
              {envRows.map((row, index) => (
                <div className="row-editor-row" key={index}>
                  <input
                    aria-label={`Environment key ${index + 1}`}
                    value={row.key}
                    onChange={(event) => {
                      const next = [...envRows];
                      next[index] = { ...next[index], key: event.target.value };
                      updateEnvRows(next);
                    }}
                    placeholder="KEY"
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <input
                    aria-label={`Environment value ${index + 1}`}
                    value={row.value}
                    onChange={(event) => {
                      const next = [...envRows];
                      next[index] = { ...next[index], value: event.target.value };
                      updateEnvRows(next);
                    }}
                    placeholder="value"
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <button
                    type="button"
                    onClick={() => updateEnvRows(envRows.filter((_, position) => position !== index))}
                  >
                    Remove
                  </button>
                </div>
              ))}
              <button type="button" onClick={() => updateEnvRows([...envRows, { key: "", value: "" }])}>
                Add variable
              </button>
            </div>
          </div>
        </div>
      )}

      <div className="preview-block" aria-live="polite">
        <div className="preview-header">
          <h3>Preview</h3>
          <button type="button" onClick={runPreview} disabled={previewing}>
            {previewing ? "Rendering…" : "Preview"}
          </button>
        </div>
        <p className="hint">Preview renders the generated shell snippets without executing the target or writing files.</p>
        {previewError && <p className="form-error" role="alert">Preview failed: {previewError}</p>}
        {previews && previews.length === 0 && <p className="hint">No preview returned for the selected shells.</p>}
        {previews?.map((snippet, index) => (
          <section className="preview-snippet" key={`${snippet.shell}-${index}`}>
            <h4>{snippet.shell ? shellLabel(snippet.shell) : "Shell"}</h4>
            {snippet.error
              ? <p className="form-error" role="alert">{snippet.error}</p>
              : <pre className="code-block">{snippet.content}</pre>}
          </section>
        ))}
      </div>

      <div className="wizard-actions">
        <button type="button" onClick={onCancel} disabled={saving}>
          {saving ? "Saving…" : "Cancel"}
        </button>
        <button type="button" className="btn-primary" onClick={save} disabled={saving} aria-busy={saving}>
          {saving ? "Saving…" : "Save"}
        </button>
      </div>
    </section>
  );
}
