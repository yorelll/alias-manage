import { useEffect, useRef, useState } from "react";
import "./styles.css";
import { AliasTable } from "./components/AliasTable";
import { SearchBar } from "./components/SearchBar";
import { TagFacet } from "./components/TagFacet";
import { AliasWizard } from "./components/AliasWizard";
import { DoctorPanel } from "./components/DoctorPanel";
import { GeneratedFilePanel } from "./components/GeneratedFilePanel";
import { ImportPanel } from "./components/ImportPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { UninstallPanel } from "./components/UninstallPanel";
import type { AliasDto } from "./lib";
import {
  type StartupStatus,
  type Page,
  NAV_ITEMS,
  createAliasPayload,
  mergeEditedAlias,
  searchAliasesArgs,
  statusSummaryText,
  tauriInvoke,
  toggleAliasEnabled,
  usesFilteredSearch,
} from "./lib";

export type { StartupStatus, Page };
export { NAV_ITEMS, statusSummaryText };
export { tauriInvoke, setTauriInvoke } from "./lib";

const SEARCH_LIMIT = 200;
/** Grace period before "Confirm delete" can fire, so a double-click cannot delete. */
const DELETE_ARM_DELAY_MS = 450;

interface StatusDrawerProps {
  status: StartupStatus | null;
  error: boolean;
}

export function StatusDrawer({ status, error }: StatusDrawerProps) {
  const [expanded, setExpanded] = useState(false);
  const summaryText = statusSummaryText(status, error);

  return (
    <div className="status-drawer" data-testid="status-drawer">
      <div
        className="status-drawer-header"
        role="button"
        aria-expanded={expanded}
        aria-controls="status-drawer-body"
        tabIndex={0}
        onClick={() => setExpanded((value) => !value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            setExpanded((value) => !value);
          }
        }}
      >
        <span className="status-drawer-summary" data-testid="status-summary">
          {summaryText}
        </span>
        <button className="status-drawer-toggle" tabIndex={-1} aria-hidden="true">
          {expanded ? "▲ collapse" : "▼ expand"}
        </button>
      </div>
      {expanded && (
        <div id="status-drawer-body" className="status-drawer-body" data-testid="status-drawer-body">
          {error ? (
            <p className="status-unavailable" role="alert">Startup status unavailable. The application is still usable.</p>
          ) : status ? (
            <>
              <span className="status-label">Version</span><span className="status-value">{status.version}</span>
              <span className="status-label">Shell</span><span className="status-value">{status.detected_shell ?? "none detected"} {status.detected_shell && `(${status.detection_source})`}</span>
              <span className="status-label">Config dir</span><span className="status-value">{status.config_directory}</span>
            </>
          ) : <span className="status-value">Loading…</span>}
        </div>
      )}
    </div>
  );
}

type WizardState = { mode: "create" } | { mode: "edit"; alias: AliasDto };

export function AliasesPage() {
  const [aliases, setAliases] = useState<AliasDto[]>([]);
  const [tagCounts, setTagCounts] = useState<Record<string, number>>({});
  const [query, setQuery] = useState("");
  const [fuzzy, setFuzzy] = useState(false);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [wizard, setWizard] = useState<WizardState | null>(null);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [deleteArmed, setDeleteArmed] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const deleteTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (deleteTimer.current) clearTimeout(deleteTimer.current);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    const load = async () => {
      try {
        const result = usesFilteredSearch(query, selectedTags)
          ? await tauriInvoke("search_aliases", searchAliasesArgs(query, fuzzy, SEARCH_LIMIT, selectedTags))
          : await tauriInvoke("list_aliases");
        if (!cancelled) {
          setAliases(result as AliasDto[]);
          setError(null);
        }
      } catch (reason) {
        if (!cancelled) setError(String(reason));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    void load();
    return () => { cancelled = true; };
  }, [query, fuzzy, selectedTags, reloadToken]);

  useEffect(() => {
    let cancelled = false;
    tauriInvoke("tag_counts")
      .then((value) => { if (!cancelled) setTagCounts(value as Record<string, number>); })
      .catch((reason) => { if (!cancelled) setError(String(reason)); });
    return () => { cancelled = true; };
  }, [reloadToken]);

  const refresh = () => setReloadToken((value) => value + 1);

  const cancelDelete = () => {
    if (deleteTimer.current) {
      clearTimeout(deleteTimer.current);
      deleteTimer.current = null;
    }
    setPendingDeleteId(null);
    setDeleteArmed(false);
  };

  const executeDelete = async (alias: AliasDto) => {
    try {
      await tauriInvoke("delete_alias", { id: alias.id });
      cancelDelete();
      setError(null);
      refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const requestDelete = (alias: AliasDto) => {
    if (pendingDeleteId !== alias.id) {
      if (deleteTimer.current) clearTimeout(deleteTimer.current);
      setPendingDeleteId(alias.id);
      setDeleteArmed(false);
      deleteTimer.current = setTimeout(() => {
        setDeleteArmed(true);
        deleteTimer.current = null;
      }, DELETE_ARM_DELAY_MS);
      return;
    }
    if (!deleteArmed) return;
    void executeDelete(alias);
  };

  const saveAlias = async (draft: AliasDto): Promise<string | null> => {
    try {
      if (wizard?.mode === "edit") {
        await tauriInvoke("update_alias", { alias: mergeEditedAlias(wizard.alias, draft) });
      } else {
        await tauriInvoke("create_alias", { alias: createAliasPayload(draft) });
      }
      cancelDelete();
      setWizard(null);
      setError(null);
      refresh();
      return null;
    } catch (reason) {
      const message = String(reason);
      setError(message);
      return message;
    }
  };

  const toggleEnabled = async (alias: AliasDto) => {
    try {
      await tauriInvoke("update_alias", { alias: toggleAliasEnabled(alias) });
      cancelDelete();
      setError(null);
      refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const filtered = usesFilteredSearch(query, selectedTags);

  return (
    <div data-testid="aliases-page">
      <div className="page-header">
        <h1 className="page-title">Aliases</h1>
        <button className="btn-add-alias" onClick={() => { cancelDelete(); setWizard({ mode: "create" }); }}>+ Add alias</button>
      </div>
      {error && <p role="alert">{error}</p>}
      <div className="alias-toolbar">
        <SearchBar
          query={query}
          fuzzy={fuzzy}
          limit={SEARCH_LIMIT}
          onChange={(value) => { setQuery(value.query); setFuzzy(value.fuzzy); }}
        />
        <TagFacet counts={tagCounts} selected={selectedTags} onChange={setSelectedTags} />
      </div>
      {wizard && (
        <AliasWizard
          key={wizard.mode === "edit" ? wizard.alias.id : "create"}
          initial={wizard.mode === "edit" ? wizard.alias : undefined}
          tags={Object.keys(tagCounts)}
          onCancel={() => setWizard(null)}
          onSave={saveAlias}
        />
      )}
      {loading ? (
        <p className="alias-loading" role="status">Loading aliases…</p>
      ) : aliases.length === 0 ? (
        filtered ? (
          <ul className="alias-list"><li className="alias-list-empty"><span>No matches for current filters.</span></li></ul>
        ) : (
          <ul className="alias-list"><li className="alias-list-empty"><span aria-hidden="true">☰</span><span>No aliases yet. Create your first alias to get started.</span></li></ul>
        )
      ) : (
        <>
          <AliasTable
            aliases={aliases}
            onEdit={(alias) => { cancelDelete(); setWizard({ mode: "edit", alias }); }}
            onToggle={toggleEnabled}
            onDelete={requestDelete}
            onCancelDelete={cancelDelete}
            pendingDeleteId={pendingDeleteId}
            confirmDeleteDisabled={!deleteArmed}
          />
          {aliases.length === SEARCH_LIMIT && (
            <p className="result-limit-notice" role="status">Showing first {SEARCH_LIMIT} results. Refine your search to see more.</p>
          )}
        </>
      )}
    </div>
  );
}

export function PlaceholderPage({ name }: { name: string }) {
  return <div className="placeholder-page">{name} – coming soon</div>;
}

export function App() {
  const [page, setPage] = useState<Page>("aliases");
  const [status, setStatus] = useState<StartupStatus | null>(null);
  const [statusError, setStatusError] = useState(false);

  useEffect(() => {
    tauriInvoke("startup_status").then((value) => setStatus(value as StartupStatus)).catch(() => setStatusError(true));
  }, []);

  return (
    <div className="app-layout">
      <nav className="sidebar" aria-label="Main navigation">
        <span className="sidebar-title">Alias Manager</span>
        <ul className="sidebar-nav">
          {NAV_ITEMS.map(({ id, label }) => <li key={id}><button className={`sidebar-nav-item${page === id ? " active" : ""}`} onClick={() => setPage(id)} aria-current={page === id ? "page" : undefined}>{label}</button></li>)}
        </ul>
      </nav>
      <div className="main-content">
        <div className="page-area">
          {page === "aliases" && <AliasesPage />}
          {page === "doctor" && <DoctorPanel />}
          {page === "settings" && <><SettingsPanel /><UninstallPanel /></>}
          {page === "sync" && <><ImportPanel /><GeneratedFilePanel /></>}
          {page !== "aliases" && page !== "doctor" && page !== "settings" && page !== "sync" && <PlaceholderPage name={NAV_ITEMS.find((item) => item.id === page)?.label ?? page} />}
        </div>
        <StatusDrawer status={status} error={statusError} />
      </div>
    </div>
  );
}
