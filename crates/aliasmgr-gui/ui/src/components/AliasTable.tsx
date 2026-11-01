import { Fragment, useState } from "react";
import type { AliasDto } from "../lib";
import { pathModeLabel, shellLabel, targetTypeLabel } from "../lib";

interface AliasTableProps {
  aliases: AliasDto[];
  onEdit?: (alias: AliasDto) => void;
  onToggle?: (alias: AliasDto) => void;
  onDelete?: (alias: AliasDto) => void;
  onCancelDelete?: () => void;
  pendingDeleteId?: string | null;
  confirmDeleteDisabled?: boolean;
}

function environmentEntries(alias: AliasDto): Array<[string, string]> {
  return Object.entries(alias.environment ?? {});
}

export function AliasTable({ aliases, onEdit, onToggle, onDelete, onCancelDelete, pendingDeleteId, confirmDeleteDisabled }: AliasTableProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);

  return (
    <div className="table-scroll">
      <table className="alias-table">
        <thead>
          <tr>
            <th scope="col"><span className="visually-hidden">Details</span></th>
            <th scope="col">Name</th>
            <th scope="col">Target</th>
            <th scope="col">Target type</th>
            <th scope="col">Shells</th>
            <th scope="col">Description</th>
            <th scope="col">Tags</th>
            <th scope="col">Status</th>
            <th scope="col">Actions</th>
          </tr>
        </thead>
        <tbody>
          {aliases.map((alias) => {
            const pending = pendingDeleteId === alias.id;
            const expanded = expandedId === alias.id;
            const detailsId = `alias-details-${alias.id}`;
            const env = environmentEntries(alias);
            return (
              <Fragment key={alias.id}>
                <tr className={pending ? "alias-row-pending-delete" : undefined}>
                  <td>
                    <button
                      type="button"
                      className="btn-details"
                      aria-expanded={expanded}
                      aria-controls={detailsId}
                      aria-label={`${expanded ? "Hide" : "Show"} details for ${alias.name}`}
                      onClick={() => setExpandedId(expanded ? null : alias.id)}
                    >
                      {expanded ? "Hide" : "Details"}
                    </button>
                  </td>
                  <td className="cell-name">{alias.name}</td>
                  <td className="cell-target">{alias.executable}</td>
                  <td>{targetTypeLabel(alias.target_type)}</td>
                  <td>{alias.shells.map(shellLabel).join(", ") || "—"}</td>
                  <td className="cell-description" title={alias.description}>{alias.description || "—"}</td>
                  <td>{alias.tags.length > 0
                    ? alias.tags.map((tag) => <span className="tag-chip" key={tag}>{tag}</span>)
                    : "—"}</td>
                  <td><span className={`status-text ${alias.enabled ? "is-enabled" : "is-disabled"}`}>{alias.enabled ? "enabled" : "disabled"}</span></td>
                  <td className="alias-actions">
                    {onToggle && (
                      <button type="button" onClick={() => onToggle(alias)} aria-label={`${alias.enabled ? "Disable" : "Enable"} ${alias.name}`}>
                        {alias.enabled ? "Disable" : "Enable"}
                      </button>
                    )}
                    {onEdit && <button type="button" onClick={() => onEdit(alias)} aria-label={`Edit ${alias.name}`}>Edit</button>}
                    {onDelete && (pending ? (
                      <>
                        <button type="button" className="btn-danger" disabled={confirmDeleteDisabled} onClick={() => onDelete(alias)}>Confirm delete</button>
                        {onCancelDelete && <button type="button" onClick={onCancelDelete}>Cancel</button>}
                      </>
                    ) : (
                      <button type="button" className="btn-danger" onClick={() => onDelete(alias)} aria-label={`Delete ${alias.name}`}>Delete</button>
                    ))}
                  </td>
                </tr>
                {expanded && (
                  <tr key={`${alias.id}-details`} id={detailsId} className="alias-details-row">
                    <td colSpan={9}>
                      <dl className="detail-grid">
                        <dt>Target type</dt><dd>{targetTypeLabel(alias.target_type)}</dd>
                        <dt>Shells</dt><dd>{alias.shells.map(shellLabel).join(", ") || "—"}</dd>
                        <dt>Pass user arguments</dt><dd>{alias.pass_args ? "yes" : "no"}</dd>
                        <dt>Fixed arguments</dt>
                        <dd>{alias.fixed_args.length > 0
                          ? <ol className="detail-args">{alias.fixed_args.map((argument, index) => <li key={index}><code>{argument}</code></li>)}</ol>
                          : "—"}</dd>
                        <dt>Working directory</dt><dd>{alias.working_directory ?? "—"}</dd>
                        <dt>Environment</dt>
                        <dd>{env.length > 0
                          ? <ul className="detail-env">{env.map(([key, value]) => <li key={key}><code>{key}</code> = <code>{value}</code></li>)}</ul>
                          : "—"}</dd>
                        <dt>Path mode</dt><dd>{pathModeLabel(alias.path_mode)}</dd>
                        <dt>Path origin</dt><dd>{alias.path_origin ?? "—"}</dd>
                        <dt>Revision</dt><dd>{alias.revision}</dd>
                        <dt>ID</dt><dd className="mono">{alias.id}</dd>
                      </dl>
                    </td>
                  </tr>
                )}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
