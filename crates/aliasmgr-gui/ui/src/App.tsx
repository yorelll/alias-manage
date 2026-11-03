import { useEffect, useMemo, useRef, useState } from "react";
import "./styles.css";
import { AliasTable } from "./components/AliasTable";
import { SearchBar } from "./components/SearchBar";
import { TagFacet } from "./components/TagFacet";
import { AliasWizard } from "./components/AliasWizard";
import { DoctorPanel } from "./components/DoctorPanel";
import { GeneratedFilePanel } from "./components/GeneratedFilePanel";
import { ImportPanel } from "./components/ImportPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { ShellSetupWizard } from "./components/ShellSetupWizard";
import { ShellStartupBanner } from "./components/ShellStartupBanner";
import { UninstallPanel } from "./components/UninstallPanel";
import type { AliasDto, AutoSyncNotice, SaveAliasOutcome, Settings, ShellStartupState } from "./lib";
import {
  type StartupStatus,
  type Page,
  NAV_ITEMS,
  autoSyncNotice,
  createAliasPayload,
  aliasFiltersActive,
  clearAliasFilters,
  mergeEditedAlias,
  nextDataVersion,
  normalizeDeleteAliasResult,
  normalizeSaveAliasResult,
  normalizeSettings,
  normalizeShellStartupState,
  preflightMessage,
  searchAliasesArgs,
  statusShellText,
  statusSummaryText,
  tauriInvoke,
  toggleAliasEnabled,
  usesFilteredSearch,
} from "./lib";
import { DEFAULT_LANGUAGE, type Language, htmlLang } from "./i18n";
import { I18nContext, useI18n } from "./language";
import type { I18nContextValue } from "./language";
import { t as translate } from "./i18n";

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
  const { language, t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  const summaryText = statusSummaryText(status, error, language);

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
          {expanded ? t("status.collapse") : t("status.expand")}
        </button>
      </div>
      {expanded && (
        <div id="status-drawer-body" className="status-drawer-body" data-testid="status-drawer-body">
          {error ? (
            <p className="status-unavailable" role="alert">{t("status.startupUnavailable")}</p>
          ) : status ? (
            <>
              <span className="status-label">{t("status.version")}</span><span className="status-value">{status.version}</span>
              <span className="status-label">{t("status.shell")}</span><span className="status-value">{statusShellText(status, language)}</span>
              <span className="status-label">{t("status.configDir")}</span><span className="status-value">{status.config_directory}</span>
            </>
          ) : <span className="status-value">{t("common.loading")}</span>}
        </div>
      )}
    </div>
  );
}

type WizardState = { mode: "create" } | { mode: "edit"; alias: AliasDto };

export function AliasesPage({ refreshToken = 0, onOpenImport, onOpenShellSetup }: { refreshToken?: number; onOpenImport?: () => void; onOpenShellSetup?: () => void }) {
  const { language, t } = useI18n();
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
  const [syncNotice, setSyncNotice] = useState<AutoSyncNotice | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const deleteTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hasLoaded = useRef(false);

  useEffect(() => () => {
    if (deleteTimer.current) clearTimeout(deleteTimer.current);
  }, []);

  useEffect(() => {
    let cancelled = false;
    if (!hasLoaded.current) setLoading(true);
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
        if (!cancelled) {
          hasLoaded.current = true;
          setLoading(false);
        }
      }
    };
    void load();
    return () => { cancelled = true; };
  }, [query, fuzzy, selectedTags, reloadToken, refreshToken]);

  useEffect(() => {
    let cancelled = false;
    tauriInvoke("tag_counts")
      .then((value) => { if (!cancelled) setTagCounts(value as Record<string, number>); })
      .catch((reason) => { if (!cancelled) setError(String(reason)); });
    return () => { cancelled = true; };
  }, [reloadToken, refreshToken]);

  const refresh = () => setReloadToken((value) => value + 1);

  const cancelDelete = () => {
    if (deleteTimer.current) {
      clearTimeout(deleteTimer.current);
      deleteTimer.current = null;
    }
    setPendingDeleteId(null);
    setDeleteArmed(false);
  };

  const showAutoSync = (notice: AutoSyncNotice) => {
    setSyncNotice(notice.tone === "none" ? null : notice);
  };

  const executeDelete = async (alias: AliasDto) => {
    try {
      const result = normalizeDeleteAliasResult(
        await tauriInvoke("delete_alias", { id: alias.id }),
      );
      showAutoSync(autoSyncNotice(result.autoSync, language));
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

  const saveAlias = async (draft: AliasDto): Promise<SaveAliasOutcome> => {
    try {
      const result = normalizeSaveAliasResult(
        wizard?.mode === "edit"
          ? await tauriInvoke("update_alias", { alias: mergeEditedAlias(wizard.alias, draft) })
          : await tauriInvoke("create_alias", { alias: createAliasPayload(draft) }),
      );
      if (!result.saved) {
        // Keep the wizard (and its draft) open; the preflight block offers the
        // shared setup flow. Nothing was written to the database.
        setError(null);
        return { error: null, preflight: result.preflight };
      }
      showAutoSync(autoSyncNotice(result.autoSync, language));
      cancelDelete();
      setWizard(null);
      setError(null);
      refresh();
      return { error: null, preflight: null };
    } catch (reason) {
      const message = String(reason);
      setError(message);
      return { error: message, preflight: null };
    }
  };

  const toggleEnabled = async (alias: AliasDto) => {
    try {
      const result = normalizeSaveAliasResult(
        await tauriInvoke("update_alias", { alias: toggleAliasEnabled(alias) }),
      );
      if (!result.saved) {
        setError(preflightMessage(result.preflight, language));
        return;
      }
      showAutoSync(autoSyncNotice(result.autoSync, language));
      cancelDelete();
      setError(null);
      refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const filtered = usesFilteredSearch(query, selectedTags);

  return (
    <div data-testid="aliases-page" tabIndex={-1}>
      <div className="page-header">
        <h1 className="page-title">{t("aliases.title")}</h1>
        <div className="page-actions">
          {onOpenImport && (
            <button type="button" className="btn-secondary" onClick={onOpenImport}>{t("aliases.import")}</button>
          )}
          <button type="button" className="btn-add-alias" onClick={() => { cancelDelete(); setWizard({ mode: "create" }); }}>{t("aliases.add")}</button>
        </div>
      </div>
      {error && <p role="alert">{error}</p>}
      {syncNotice && syncNotice.tone !== "none" && (
        <div
          className={`auto-sync-notice auto-sync-${syncNotice.tone}`}
          role={syncNotice.tone === "warning" ? "alert" : "status"}
          data-testid="auto-sync-notice"
        >
          <p>{syncNotice.text}</p>
          {syncNotice.reloadCommands.length > 0 && (
            <details>
              <summary>{t("autoSync.reloadTitle")}</summary>
              <ul>
                {syncNotice.reloadCommands.map((command) => (
                  <li key={command}><code>{command}</code></li>
                ))}
              </ul>
            </details>
          )}
        </div>
      )}
      <div className="alias-toolbar">
        <SearchBar
          query={query}
          fuzzy={fuzzy}
          limit={SEARCH_LIMIT}
          onChange={(value) => { setQuery(value.query); setFuzzy(value.fuzzy); }}
        />
        <TagFacet counts={tagCounts} selected={selectedTags} onChange={setSelectedTags} />
        <button
          type="button"
          className="btn-secondary"
          disabled={!aliasFiltersActive(query, fuzzy, selectedTags)}
          onClick={() => {
            const cleared = clearAliasFilters();
            setQuery(cleared.query);
            setFuzzy(cleared.fuzzy);
            setSelectedTags(cleared.tags);
          }}
        >
          {t("aliases.clearFilters")}
        </button>
      </div>
      {wizard && (
        <AliasWizard
          key={wizard.mode === "edit" ? wizard.alias.id : "create"}
          initial={wizard.mode === "edit" ? wizard.alias : undefined}
          tags={Object.keys(tagCounts)}
          onCancel={() => setWizard(null)}
          onSave={saveAlias}
          onOpenShellSetup={onOpenShellSetup}
        />
      )}
      {loading ? (
        <p className="alias-loading" role="status">{t("aliases.loading")}</p>
      ) : aliases.length === 0 ? (
        error ? null : filtered ? (
          <ul className="alias-list"><li className="alias-list-empty"><span>{t("aliases.noMatches")}</span></li></ul>
        ) : (
          <ul className="alias-list"><li className="alias-list-empty"><span aria-hidden="true">☰</span><span>{t("aliases.empty")}</span></li></ul>
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
          {filtered && aliases.length === SEARCH_LIMIT && (
            <p className="result-limit-notice" role="status">{t("aliases.resultLimit", { limit: SEARCH_LIMIT })}</p>
          )}
        </>
      )}
    </div>
  );
}

export function PlaceholderPage({ name }: { name: string }) {
  const { t } = useI18n();
  return <div className="placeholder-page">{t("placeholder.comingSoon", { name })}</div>;
}

export function App() {
  const [page, setPage] = useState<Page>("aliases");
  const [status, setStatus] = useState<StartupStatus | null>(null);
  const [statusError, setStatusError] = useState(false);
  const [dataVersion, setDataVersion] = useState(0);
  const [language, setLanguage] = useState<Language>(DEFAULT_LANGUAGE);
  const [shellState, setShellState] = useState<ShellStartupState | null>(null);
  const [shellStateError, setShellStateError] = useState<string | null>(null);
  const [shellStateLoading, setShellStateLoading] = useState(true);
  const [showShellWizard, setShowShellWizard] = useState(false);
  const [ignoredShellWarnings, setIgnoredShellWarnings] = useState<string[]>([]);
  const [shellPanelToken, setShellPanelToken] = useState(0);
  const syncPageRef = useRef<HTMLDivElement | null>(null);
  const focusSyncAfterNavigation = useRef(false);

  const invalidateAliasData = () => setDataVersion((value) => nextDataVersion(value));

  const loadShellState = async () => {
    setShellStateLoading(true);
    try {
      const state = normalizeShellStartupState(await tauriInvoke("shell_startup_state"));
      setShellState(state);
      setShellStateError(null);
      return state;
    } catch (reason) {
      setShellStateError(String(reason));
      return null;
    } finally {
      setShellStateLoading(false);
    }
  };

  const openShellSetup = () => {
    setShellPanelToken((value) => value + 1);
    setPage("settings");
  };

  const openImportPage = () => {
    focusSyncAfterNavigation.current = true;
    setPage("sync");
  };

  const i18n = useMemo<I18nContextValue>(() => ({
    language,
    setLanguage,
    t: (key, params) => translate(language, key, params),
  }), [language]);

  useEffect(() => {
    document.documentElement.lang = htmlLang(language);
  }, [language]);

  useEffect(() => {
    let cancelled = false;
    // Default stays Chinese when config cannot be read.
    tauriInvoke("config_get")
      .then((value) => {
        if (!cancelled) setLanguage(normalizeSettings(value as Settings).language);
      })
      .catch(() => undefined);
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (page === "sync" && focusSyncAfterNavigation.current) {
      focusSyncAfterNavigation.current = false;
      const fileInput = document.getElementById("import-file-input") as HTMLInputElement | null;
      if (fileInput && !fileInput.disabled) fileInput.focus();
      else syncPageRef.current?.focus();
    }
  }, [page]);

  useEffect(() => {
    tauriInvoke("startup_status").then((value) => setStatus(value as StartupStatus)).catch(() => setStatusError(true));
  }, []);

  useEffect(() => {
    let cancelled = false;
    tauriInvoke("shell_startup_state")
      .then((value) => {
        if (cancelled) return;
        const state = normalizeShellStartupState(value);
        setShellState(state);
        setShellStateError(null);
        // The full wizard is a fresh-install experience only. Skipping writes
        // an empty configuration, so later launches show a compact notice.
        if (state.firstRun) setShowShellWizard(true);
      })
      .catch((reason) => { if (!cancelled) setShellStateError(String(reason)); })
      .finally(() => { if (!cancelled) setShellStateLoading(false); });
    return () => { cancelled = true; };
  }, []);

  const ignoreShellWarning = (shell: string) => {
    setIgnoredShellWarnings((current) => (
      current.includes(shell) ? current : [...current, shell]
    ));
  };

  const refreshShellState = () => { void loadShellState(); };

  return (
    <I18nContext.Provider value={i18n}>
      <div className="app-layout">
        <nav className="sidebar" aria-label={i18n.t("nav.aria")}>
          <span className="sidebar-title">{i18n.t("app.title")}</span>
          <ul className="sidebar-nav">
            {NAV_ITEMS.map(({ id, labelKey }) => <li key={id}><button className={`sidebar-nav-item${page === id ? " active" : ""}`} onClick={() => setPage(id)} aria-current={page === id ? "page" : undefined}>{i18n.t(labelKey)}</button></li>)}
          </ul>
        </nav>
        <div className="main-content">
          {showShellWizard && (
            <ShellSetupWizard
              statuses={shellState?.shells ?? []}
              loading={shellStateLoading}
              error={shellStateError}
              firstRun
              onRescan={() => void loadShellState()}
              onClose={() => setShowShellWizard(false)}
              onApplied={() => void loadShellState()}
            />
          )}
          {!showShellWizard && shellState && (
            <ShellStartupBanner
              state={shellState}
              ignored={ignoredShellWarnings}
              onIgnore={ignoreShellWarning}
              onChanged={refreshShellState}
              onOpenSetup={openShellSetup}
            />
          )}
          <div className="page-area">
            {/* Pages stay mounted so drafts survive navigation; inactive pages
                are hidden from layout, the accessibility tree, and tab order. */}
            <div hidden={page !== "aliases"}><AliasesPage refreshToken={dataVersion} onOpenImport={openImportPage} onOpenShellSetup={openShellSetup} /></div>
            <div hidden={page !== "sync"} ref={syncPageRef} tabIndex={-1}><ImportPanel onImported={invalidateAliasData} /><GeneratedFilePanel /></div>
            <div hidden={page !== "doctor"}><DoctorPanel /></div>
            <div hidden={page !== "settings"}><SettingsPanel focusShellsToken={shellPanelToken} onShellsChanged={refreshShellState} /><UninstallPanel onUninstalled={invalidateAliasData} /></div>
          </div>
          <StatusDrawer status={status} error={statusError} />
        </div>
      </div>
    </I18nContext.Provider>
  );
}
