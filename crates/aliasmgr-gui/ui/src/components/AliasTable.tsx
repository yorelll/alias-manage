import type { AliasDto } from "../lib";

export function AliasTable({ aliases, onEdit }: { aliases: AliasDto[]; onEdit?: (alias: AliasDto) => void }) {
  return <table className="alias-table"><thead><tr><th>Name</th><th>Target</th><th>Shells</th><th>Description</th><th>Tags</th><th>Status</th><th /></tr></thead><tbody>{aliases.map((alias) => <tr key={alias.id}><td>{alias.name}</td><td>{alias.executable}</td><td>{alias.shells.join(", ")}</td><td title={alias.description}>{alias.description}</td><td>{alias.tags.map((tag) => <span className="tag-chip" key={tag}>{tag}</span>)}</td><td>{alias.enabled ? "enabled" : "disabled"}</td><td>{onEdit && <button onClick={() => onEdit(alias)}>Edit</button>}</td></tr>)}</tbody></table>;
}
