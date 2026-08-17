export function SearchBar({ query, fuzzy, limit, onChange }: { query: string; fuzzy: boolean; limit: number; onChange: (value: { query: string; fuzzy: boolean; limit: number }) => void }) {
  return <div className="search-bar"><input aria-label="Search aliases" value={query} onChange={(event) => onChange({ query: event.target.value, fuzzy, limit })} placeholder="Search aliases" /><label><input type="checkbox" checked={fuzzy} onChange={(event) => onChange({ query, fuzzy: event.target.checked, limit })} /> fuzzy</label></div>;
}
