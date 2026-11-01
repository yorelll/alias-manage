import type { AliasDto } from "../lib";

interface AliasTableProps {
  aliases: AliasDto[];
  onEdit?: (alias: AliasDto) => void;
  onToggle?: (alias: AliasDto) => void;
  onDelete?: (alias: AliasDto) => void;
  onCancelDelete?: () => void;
  pendingDeleteId?: string | null;
}

export function AliasTable({ aliases, onEdit, onToggle, onDelete, onCancelDelete, pendingDeleteId }: AliasTableProps) {
  return (
    <table className="alias-table">
      <thead>
        <tr><th>Name</th><th>Target</th><th>Shells</th><th>Description</th><th>Tags</th><th>Status</th><th>Actions</th></tr>
      </thead>
      <tbody>
        {aliases.map((alias) => {
          const pending = pendingDeleteId === alias.id;
          return (
            <tr key={alias.id} className={pending ? "alias-row-pending-delete" : undefined}>
              <td>{alias.name}</td>
              <td>{alias.executable}</td>
              <td>{alias.shells.join(", ")}</td>
              <td title={alias.description}>{alias.description}</td>
              <td>{alias.tags.map((tag) => <span className="tag-chip" key={tag}>{tag}</span>)}</td>
              <td>{alias.enabled ? "enabled" : "disabled"}</td>
              <td className="alias-actions">
                {onToggle && (
                  <button onClick={() => onToggle(alias)} aria-label={`${alias.enabled ? "Disable" : "Enable"} ${alias.name}`}>
                    {alias.enabled ? "Disable" : "Enable"}
                  </button>
                )}
                {onEdit && <button onClick={() => onEdit(alias)}>Edit</button>}
                {onDelete && (pending ? (
                  <>
                    <button className="btn-danger" onClick={() => onDelete(alias)}>Confirm delete</button>
                    {onCancelDelete && <button onClick={onCancelDelete}>Cancel</button>}
                  </>
                ) : (
                  <button className="btn-danger" onClick={() => onDelete(alias)}>Delete</button>
                ))}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
