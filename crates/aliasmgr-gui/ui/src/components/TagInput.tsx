import { useId, useState } from "react";
import { normalizeTags } from "../lib";

interface TagInputProps {
  tags: string[];
  suggestions: string[];
  onChange: (tags: string[]) => void;
  label?: string;
  id?: string;
}

export function TagInput({ tags, suggestions, onChange, label = "Tags", id }: TagInputProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const listId = `${inputId}-suggestions`;
  const [value, setValue] = useState("");
  const add = () => {
    const next = normalizeTags([...tags, value]);
    setValue("");
    onChange(next);
  };
  return (
    <div className="tag-input">
      <label htmlFor={inputId}>{label}</label>
      <div className="tag-chip-list">
        {tags.map((tag) => (
          <button
            type="button"
            key={tag}
            aria-label={`Remove tag ${tag}`}
            onClick={() => onChange(tags.filter((current) => current !== tag))}
          >
            {tag} ×
          </button>
        ))}
      </div>
      <input
        id={inputId}
        aria-label={label}
        value={value}
        list={listId}
        placeholder="Add a tag and press Enter"
        autoComplete="off"
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === ",") {
            event.preventDefault();
            add();
          }
        }}
      />
      <datalist id={listId}>
        {suggestions.map((tag) => <option key={tag} value={tag} />)}
      </datalist>
    </div>
  );
}
