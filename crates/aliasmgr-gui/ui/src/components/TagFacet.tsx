export function TagFacet({ counts, selected, onChange }: { counts: Record<string, number>; selected: string[]; onChange: (tags: string[]) => void }) {
  return <div className="tag-facet" aria-label="Tag filters">{Object.entries(counts).map(([tag, count]) => <button className={selected.includes(tag) ? "selected" : ""} key={tag} onClick={() => onChange(selected.includes(tag) ? selected.filter((value) => value !== tag) : [...selected, tag])}>{tag} ({count})</button>)}</div>;
}
