import { useState } from "react";
import { normalizeTags } from "../lib";

export function TagInput({ tags, suggestions, onChange }: { tags: string[]; suggestions: string[]; onChange: (tags: string[]) => void }) {
  const [value, setValue] = useState("");
  const add = () => { const next = normalizeTags([...tags, value]); setValue(""); onChange(next); };
  return <div className="tag-input"><div>{tags.map((tag) => <button type="button" key={tag} onClick={() => onChange(tags.filter((value) => value !== tag))}>{tag} ×</button>)}</div><input value={value} list="tag-suggestions" onChange={(event) => setValue(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" || event.key === ",") { event.preventDefault(); add(); } }} /><datalist id="tag-suggestions">{suggestions.map((tag) => <option key={tag} value={tag} />)}</datalist></div>;
}
