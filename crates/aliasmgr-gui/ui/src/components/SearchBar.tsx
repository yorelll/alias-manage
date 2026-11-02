import { useI18n } from "../language";

export function SearchBar({ query, fuzzy, limit, onChange }: { query: string; fuzzy: boolean; limit: number; onChange: (value: { query: string; fuzzy: boolean; limit: number }) => void }) {
  const { t } = useI18n();
  return <div className="search-bar"><input aria-label={t("aliases.search")} value={query} onChange={(event) => onChange({ query: event.target.value, fuzzy, limit })} placeholder={t("aliases.search")} /><label><input type="checkbox" checked={fuzzy} onChange={(event) => onChange({ query, fuzzy: event.target.checked, limit })} /> {t("aliases.fuzzy")}</label></div>;
}
