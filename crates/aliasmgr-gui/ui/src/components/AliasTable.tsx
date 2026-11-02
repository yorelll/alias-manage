import { Fragment, useState } from "react";
import type { AliasDto } from "../lib";
import { pathModeLabel, pathOriginLabel, shellLabel, targetTypeLabel } from "../lib";
import { useI18n } from "../language";

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
  const { language, t } = useI18n();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const shellsText = (alias: AliasDto) => alias.shells.map((shell) => shellLabel(shell, language)).join(", ") || "—";

  return (
    <div className="table-scroll">
      <table className="alias-table">
        <thead>
          <tr>
            <th scope="col"><span className="visually-hidden">{t("table.detailsHidden")}</span></th>
            <th scope="col">{t("table.name")}</th>
            <th scope="col">{t("table.target")}</th>
            <th scope="col">{t("table.targetType")}</th>
            <th scope="col">{t("table.shells")}</th>
            <th scope="col">{t("table.description")}</th>
            <th scope="col">{t("table.tags")}</th>
            <th scope="col">{t("table.status")}</th>
            <th scope="col">{t("table.actions")}</th>
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
                      aria-label={t(expanded ? "table.hideDetails" : "table.showDetails", { name: alias.name })}
                      onClick={() => setExpandedId(expanded ? null : alias.id)}
                    >
                      {expanded ? t("table.hide") : t("table.details")}
                    </button>
                  </td>
                  <td className="cell-name">{alias.name}</td>
                  <td className="cell-target">{alias.executable}</td>
                  <td>{targetTypeLabel(alias.target_type, language)}</td>
                  <td>{shellsText(alias)}</td>
                  <td className="cell-description" title={alias.description}>{alias.description || "—"}</td>
                  <td>{alias.tags.length > 0
                    ? alias.tags.map((tag) => <span className="tag-chip" key={tag}>{tag}</span>)
                    : "—"}</td>
                  <td><span className={`status-text ${alias.enabled ? "is-enabled" : "is-disabled"}`}>{t(alias.enabled ? "table.enabled" : "table.disabled")}</span></td>
                  <td className="alias-actions">
                    {onToggle && (
                      <button type="button" onClick={() => onToggle(alias)} aria-label={t(alias.enabled ? "table.disableAria" : "table.enableAria", { name: alias.name })}>
                        {t(alias.enabled ? "table.disable" : "table.enable")}
                      </button>
                    )}
                    {onEdit && <button type="button" onClick={() => onEdit(alias)} aria-label={t("table.editAria", { name: alias.name })}>{t("table.edit")}</button>}
                    {onDelete && (pending ? (
                      <>
                        <button type="button" className="btn-danger" disabled={confirmDeleteDisabled} onClick={() => onDelete(alias)}>{t("table.confirmDelete")}</button>
                        {onCancelDelete && <button type="button" onClick={onCancelDelete}>{t("common.cancel")}</button>}
                      </>
                    ) : (
                      <button type="button" className="btn-danger" onClick={() => onDelete(alias)} aria-label={t("table.deleteAria", { name: alias.name })}>{t("table.delete")}</button>
                    ))}
                  </td>
                </tr>
                {expanded && (
                  <tr key={`${alias.id}-details`} id={detailsId} className="alias-details-row">
                    <td colSpan={9}>
                      <dl className="detail-grid">
                        <dt>{t("detail.targetType")}</dt><dd>{targetTypeLabel(alias.target_type, language)}</dd>
                        <dt>{t("detail.shells")}</dt><dd>{shellsText(alias)}</dd>
                        <dt>{t("detail.passArgs")}</dt><dd>{t(alias.pass_args ? "common.yes" : "common.no")}</dd>
                        <dt>{t("detail.fixedArgs")}</dt>
                        <dd>{alias.fixed_args.length > 0
                          ? <ol className="detail-args">{alias.fixed_args.map((argument, index) => <li key={index}><code>{argument}</code></li>)}</ol>
                          : "—"}</dd>
                        <dt>{t("detail.workingDirectory")}</dt><dd>{alias.working_directory ?? "—"}</dd>
                        <dt>{t("detail.environment")}</dt>
                        <dd>{env.length > 0
                          ? <ul className="detail-env">{env.map(([key, value]) => <li key={key}><code>{key}</code> = <code>{value}</code></li>)}</ul>
                          : "—"}</dd>
                        <dt>{t("detail.pathMode")}</dt><dd>{pathModeLabel(alias.path_mode, language)}</dd>
                        <dt>{t("detail.pathOrigin")}</dt><dd>{pathOriginLabel(alias.path_origin, language)}</dd>
                        <dt>{t("detail.revision")}</dt><dd>{alias.revision}</dd>
                        <dt>{t("detail.id")}</dt><dd className="mono">{alias.id}</dd>
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
