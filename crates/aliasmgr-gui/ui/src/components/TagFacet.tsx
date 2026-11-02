import { toggleTag } from "../lib";
import { useI18n } from "../language";

export function TagFacet({ counts, selected, onChange }: { counts: Record<string, number>; selected: string[]; onChange: (tags: string[]) => void }) {
  const { t } = useI18n();
  return <div className="tag-facet" aria-label={t("aliases.tagFilters")}>{Object.entries(counts).map(([tag, count]) => <button className={selected.includes(tag) ? "selected" : ""} key={tag} onClick={() => onChange(toggleTag(selected, tag))}>{tag} ({count})</button>)}</div>;
}
