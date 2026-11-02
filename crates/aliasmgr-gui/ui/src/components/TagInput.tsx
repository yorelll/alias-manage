import { useId, useState } from "react";
import { normalizeTags } from "../lib";
import { useI18n } from "../language";

interface TagInputProps {
  tags: string[];
  suggestions: string[];
  onChange: (tags: string[]) => void;
  label?: string;
  id?: string;
}

export function TagInput({ tags, suggestions, onChange, label, id }: TagInputProps) {
  const { t } = useI18n();
  const text = label ?? t("wizard.tags");
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
      <label htmlFor={inputId}>{text}</label>
      <div className="tag-chip-list">
        {tags.map((tag) => (
          <button
            type="button"
            key={tag}
            aria-label={`${t("common.remove")} ${tag}`}
            onClick={() => onChange(tags.filter((current) => current !== tag))}
          >
            {tag} ×
          </button>
        ))}
      </div>
      <input
        id={inputId}
        aria-label={text}
        value={value}
        list={listId}
        placeholder={t("wizard.tagPlaceholder")}
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
