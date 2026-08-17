import { useState } from "react";
import type { AliasDto } from "../lib";
import { normalizeTags } from "../lib";
import { TagInput } from "./TagInput";

export function AliasWizard({ initial, tags, onCancel, onSave }: { initial?: AliasDto; tags: string[]; onCancel: () => void; onSave: (alias: AliasDto) => Promise<string | null> }) {
  const [advanced, setAdvanced] = useState(false);
  const [draft, setDraft] = useState<AliasDto>(initial ?? { id: "00000000-0000-0000-0000-000000000000", name: "", description: "", executable: "", target_type: "native_executable", fixed_args: [], pass_args: true, working_directory: null, environment: {}, shells: ["bash"], enabled: true, tags: [], revision: 1 });
  const [error, setError] = useState<string | null>(null);
  const update = (patch: Partial<AliasDto>) => setDraft((value) => ({ ...value, ...patch }));
  const save = async () => { if (!/^[A-Za-z_][A-Za-z0-9_-]{0,63}$/.test(draft.name)) { setError("invalid alias name"); return; } const result = await onSave({ ...draft, tags: normalizeTags(draft.tags) }); if (result) setError(result); };
  return <section className="alias-wizard" aria-label="Alias wizard"><h2>{initial ? "Edit alias" : "Add alias"}</h2>{error && <p role="alert">Unable to save: {error}</p>}<label>Name<input value={draft.name} onChange={(event) => update({ name: event.target.value })} /></label><label>Description<textarea value={draft.description} onChange={(event) => update({ description: event.target.value })} /></label><label>Executable<input value={draft.executable} onChange={(event) => update({ executable: event.target.value })} /></label><label>Shell<select value={draft.shells[0] ?? "bash"} onChange={(event) => update({ shells: [event.target.value] })}><option>bash</option><option>zsh</option><option>powershell5</option><option>powershell7</option></select></label><TagInput tags={draft.tags} suggestions={tags} onChange={(next) => update({ tags: next })} /><button type="button" onClick={() => setAdvanced((value) => !value)}>{advanced ? "Hide advanced" : "Show advanced"}</button>{advanced && <div><label>Fixed args<input value={draft.fixed_args.join(" ")} onChange={(event) => update({ fixed_args: event.target.value.split(" ").filter(Boolean) })} /></label><label>Working directory<input value={draft.working_directory ?? ""} onChange={(event) => update({ working_directory: event.target.value || null })} /></label></div>}<pre>{JSON.stringify(draft, null, 2)}</pre><button type="button" onClick={onCancel}>Cancel</button><button type="button" onClick={save}>Save</button></section>;
}
