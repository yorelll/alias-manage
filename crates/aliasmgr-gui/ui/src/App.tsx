import { useEffect, useState } from "react";
import "./styles.css";
import {
  type StartupStatus,
  type Page,
  NAV_ITEMS,
  statusSummaryText,
  tauriInvoke,
} from "./lib";

export type { StartupStatus, Page };
export { NAV_ITEMS, statusSummaryText };
export { tauriInvoke, setTauriInvoke } from "./lib";

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

export function AliasesPage() {
  return (
    <div data-testid="aliases-page">
      <div className="page-header"><h1 className="page-title">Aliases</h1><button className="btn-add-alias" disabled aria-label="Add alias (not yet implemented)">+ Add alias</button></div>
      <ul className="alias-list"><li className="alias-list-empty"><span aria-hidden="true">☰</span><span>No aliases yet. Create your first alias to get started.</span></li></ul>
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
          {page !== "aliases" && <PlaceholderPage name={NAV_ITEMS.find((item) => item.id === page)?.label ?? page} />}
        </div>
        <StatusDrawer status={status} error={statusError} />
      </div>
    </div>
  );
}
