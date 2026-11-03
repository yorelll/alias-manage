//! Application state and the keybinding state machine.
//!
//! All persistence goes through `AliasService`/`Database`; the TUI never writes
//! files, profiles, or the database directly except through the shared core.

use crate::{
    data::{self, JobKind, JobResult, Jobs, Snapshot},
    import_export::{ExportAction, ExportState, ImportAction, ImportState},
    input::{is_actionable, is_ctrl, TextInput},
    model::{DraftError, Editor, EditorOutcome, Field},
    settings::{SettingsOutcome, SettingsState},
    text,
    theme::Theme,
};
use aliasmgr_core::{
    config::AppPaths,
    doctor::DoctorFinding,
    error::AliasError,
    lock::FileLock,
    model::{AliasRecord, ShellKind, ShellStatus},
    search::{search, SearchField, SearchQuery, SortField},
    service::AliasService,
    storage::Database,
    sync::{self, SyncCoordinator, SyncRun},
    validation::{validate_alias_name, validate_powershell_name},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub kind: StatusKind,
    pub text: String,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            kind: StatusKind::Info,
            text: "Ready. Press ? for help.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterState {
    pub input: TextInput,
    pub active: bool,
    pub field: SearchField,
    pub fuzzy: bool,
    pub tags: Vec<String>,
    pub sort: SortField,
    pub descending: bool,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            input: TextInput::default(),
            active: false,
            field: SearchField::All,
            fuzzy: false,
            tags: Vec::new(),
            sort: SortField::Name,
            descending: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    Alias,
    File,
}

impl PreviewMode {
    pub fn label(self) -> &'static str {
        match self {
            PreviewMode::Alias => "alias definition",
            PreviewMode::File => "whole shell file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewState {
    pub alias: Box<AliasRecord>,
    pub shells: Vec<ShellKind>,
    pub index: usize,
    pub body: String,
    pub error: Option<String>,
    pub scroll: u16,
    pub mode: PreviewMode,
}

impl PreviewState {
    pub fn shell(&self) -> Option<&ShellKind> {
        self.shells.get(self.index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorState {
    pub findings: Vec<DoctorFinding>,
    pub selected: usize,
    pub scroll: u16,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollState {
    pub title: String,
    pub lines: Vec<String>,
    pub scroll: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReportState {
    pub run: SyncRun,
    pub scroll: u16,
    pub retry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    ConfirmDelete {
        alias: Box<AliasRecord>,
    },
    ConfirmQuit,
    ConfirmDiscard,
    Rename {
        alias: Box<AliasRecord>,
        input: TextInput,
        error: Option<String>,
    },
    TagFilter {
        entries: Vec<(String, bool)>,
        cursor: usize,
    },
    Conflict {
        base: Box<AliasRecord>,
        current: Box<AliasRecord>,
    },
    /// The record was deleted in another front end while the editor was open.
    Deleted {
        name: String,
    },
    Message {
        title: String,
        lines: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub enum Screen {
    Browse,
    Editor(Box<Editor>),
    Preview(PreviewState),
    Doctor(DoctorState),
    Help(ScrollState),
    SyncReport(SyncReportState),
    Import(ImportState),
    Export(ExportState),
    Settings(Box<SettingsState>),
}

pub struct App {
    pub root: PathBuf,
    pub database: Database,
    pub snapshot: Snapshot,
    pub visible: Vec<usize>,
    pub selected: usize,
    pub list_offset: usize,
    pub list_height: usize,
    pub detail_scroll: u16,
    pub detail_height: u16,
    pub preview_height: usize,
    pub preview_max_scroll: u16,
    pub preview_rows: usize,
    pub doctor_height: usize,
    pub doctor_max_scroll: u16,
    pub doctor_rows: usize,
    pub filter: FilterState,
    pub screen: Screen,
    pub focus: Focus,
    pub show_detail: bool,
    /// Set by the renderer; lets Enter/Esc choose between the wide two-pane
    /// and narrow single-pane semantics without a second size query.
    pub wide_layout: bool,
    pub modal: Option<Modal>,
    pub status: Status,
    pub theme: Theme,
    pub should_quit: bool,
    pub dirty: bool,
    pub jobs: Jobs,
    pub saving: bool,
    pub external_pending: bool,
    /// When the DB snapshot was last refreshed (relative time in the header).
    pub last_refresh: Instant,
    /// Set by the renderer. While the terminal is too small every key except
    /// q/Ctrl+C is ignored, so invisible destructive modals cannot fire.
    pub too_small: bool,
    last_external_check: Instant,
}

impl App {
    pub fn new(root: PathBuf) -> Result<Self, AliasError> {
        AppPaths { root: root.clone() }.ensure_directories()?;
        // A sync interrupted by a crash leaves a journal. Recover it through
        // the shared core helper under the same lock sync uses; never write a
        // second recovery implementation here.
        let journal = root.join("operation.journal");
        // The core predicate distinguishes prepared/committed work from the
        // completed journal a successful sync leaves behind, and surfaces
        // unreadable or unknown states instead of silently skipping them.
        let recovery = match sync::journal_needs_recovery(&journal) {
            Ok(false) => None,
            Ok(true) => match FileLock::acquire(root.join("sync.lock"), Duration::from_secs(2)) {
                Ok(_lock) => Some(
                    SyncCoordinator::new(root.clone())
                        .recover_pending_operations()
                        .map_err(|error| error.to_string()),
                ),
                Err(error) => Some(Err(format!("sync lock unavailable: {error}"))),
            },
            Err(error) => Some(Err(format!("cannot inspect the journal: {error}"))),
        };
        let database = Database::open(root.join("aliases.db"))?;
        let snapshot = data::load_snapshot(&database, &root)?;
        let mut app = Self {
            root,
            database,
            snapshot,
            visible: Vec::new(),
            selected: 0,
            list_offset: 0,
            list_height: 10,
            detail_scroll: 0,
            detail_height: 10,
            preview_height: 10,
            preview_max_scroll: 0,
            preview_rows: 0,
            doctor_height: 10,
            doctor_max_scroll: 0,
            doctor_rows: 0,
            filter: FilterState::default(),
            screen: Screen::Browse,
            focus: Focus::List,
            show_detail: false,
            wide_layout: false,
            modal: None,
            status: Status::default(),
            theme: Theme::detect(),
            should_quit: false,
            dirty: true,
            jobs: Jobs::new(),
            saving: false,
            external_pending: false,
            last_refresh: Instant::now(),
            too_small: false,
            last_external_check: Instant::now(),
        };
        app.recompute_visible();
        match recovery {
            Some(Ok(())) => app.set_status(
                StatusKind::Warning,
                "recovered an interrupted sync; verify with Doctor and sync again",
            ),
            Some(Err(error)) => app.set_status(
                StatusKind::Error,
                format!("pending sync journal could not be recovered: {error}; run Doctor"),
            ),
            None => {}
        }
        Ok(app)
    }

    pub fn selected_alias(&self) -> Option<&AliasRecord> {
        self.visible
            .get(self.selected)
            .and_then(|index| self.snapshot.aliases.get(*index))
    }

    pub fn selected_alias_cloned(&self) -> Option<AliasRecord> {
        self.selected_alias().cloned()
    }

    pub fn set_status(&mut self, kind: StatusKind, message: impl Into<String>) {
        self.status = Status {
            kind,
            text: message.into(),
        };
        self.dirty = true;
    }

    pub fn busy_label(&self) -> Option<String> {
        self.jobs.running().map(JobKind::label)
    }

    fn recompute_visible(&mut self) {
        let query = SearchQuery {
            query: self.filter.input.value().to_string(),
            fuzzy: self.filter.fuzzy,
            field: self.filter.field,
            limit: 0,
            sort: self.filter.sort,
            descending: self.filter.descending,
            tag_filter: self.filter.tags.clone(),
        };
        let positions: BTreeMap<uuid::Uuid, usize> = self
            .snapshot
            .aliases
            .iter()
            .enumerate()
            .map(|(index, alias)| (alias.id, index))
            .collect();
        self.visible = search(&self.snapshot.aliases, &query)
            .into_iter()
            .filter_map(|result| positions.get(&result.alias.id).copied())
            .collect();
        self.selected = if self.visible.is_empty() {
            0
        } else {
            self.selected.min(self.visible.len() - 1)
        };
    }

    pub fn reload(&mut self) -> bool {
        let selected_id = self.selected_alias().map(|alias| alias.id);
        match data::load_snapshot(&self.database, &self.root) {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                self.external_pending = false;
                self.last_refresh = Instant::now();
                self.recompute_visible();
                if let Some(id) = selected_id {
                    if let Some(position) = self
                        .visible
                        .iter()
                        .position(|index| self.snapshot.aliases[*index].id == id)
                    {
                        self.selected = position;
                    }
                }
                self.dirty = true;
                true
            }
            Err(error) => {
                self.set_status(StatusKind::Error, format!("refresh failed: {error}"));
                false
            }
        }
    }

    /// Re-reads the database on demand and reports whether another front end
    /// changed it since the last snapshot.
    pub fn manual_refresh(&mut self) {
        let before = self.snapshot.fingerprint.clone();
        if !self.reload() {
            return;
        }
        if before == self.snapshot.fingerprint {
            self.set_status(StatusKind::Info, "refreshed; no external changes");
        } else {
            self.set_status(
                StatusKind::Info,
                "refreshed; external changes applied from the shared database",
            );
        }
    }

    /// Detects writes from the CLI/GUI. The editor keeps its draft, but the
    /// revision check will refuse a silent overwrite on save.
    pub fn poll_external_changes(&mut self) {
        if self.jobs.is_busy() || self.last_external_check.elapsed() < Duration::from_millis(1500) {
            return;
        }
        self.last_external_check = Instant::now();
        let Ok(fingerprint) = data::disk_fingerprint(&self.database) else {
            return;
        };
        if fingerprint == self.snapshot.fingerprint {
            return;
        }
        let selected_id = self.selected_alias().map(|alias| alias.id);
        let editor_open = matches!(self.screen, Screen::Editor(_));
        if let Ok(snapshot) = data::load_snapshot(&self.database, &self.root) {
            self.snapshot = snapshot;
            self.recompute_visible();
            if let Some(id) = selected_id {
                if let Some(position) = self
                    .visible
                    .iter()
                    .position(|index| self.snapshot.aliases[*index].id == id)
                {
                    self.selected = position;
                }
            }
            self.external_pending = true;
            self.last_refresh = Instant::now();
            self.dirty = true;
            if editor_open {
                self.set_status(
                    StatusKind::Warning,
                    "records changed in another front end; saving will ask before overwriting",
                );
            } else {
                self.set_status(
                    StatusKind::Info,
                    "records changed in another front end; list refreshed",
                );
            }
        }
    }

    /// A terminal resize only needs a redraw; layout is recomputed from the
    /// new frame area, including the too-small guard.
    pub fn on_resize(&mut self) {
        self.dirty = true;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if !is_actionable(&key) {
            return;
        }
        if self.too_small {
            let plain_q = !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
                && matches!(key.code, KeyCode::Char('q'));
            if is_ctrl(&key, 'c') || plain_q {
                self.should_quit = true;
            }
            return;
        }
        if is_ctrl(&key, 'c') {
            self.request_quit();
            return;
        }
        if self.modal.is_some() {
            let modal = self.modal.take().expect("checked above");
            self.handle_modal(modal, key);
            return;
        }
        match &mut self.screen {
            Screen::Browse => self.handle_browser_key(key),
            Screen::Editor(_) => {
                let (outcome, dirty) = if let Screen::Editor(editor) = &mut self.screen {
                    (editor.handle_key(key), editor.is_dirty())
                } else {
                    unreachable!()
                };
                self.dirty = true;
                match outcome {
                    EditorOutcome::Dirty | EditorOutcome::None => {}
                    EditorOutcome::SaveRequested => self.save_editor(),
                    EditorOutcome::CancelRequested => {
                        if dirty {
                            self.modal = Some(Modal::ConfirmDiscard);
                        } else {
                            self.screen = Screen::Browse;
                        }
                    }
                }
            }
            Screen::Preview(_) => self.handle_preview_key(key),
            Screen::Doctor(_) => self.handle_doctor_key(key),
            Screen::Help(_) => self.handle_scroll_key(key),
            Screen::SyncReport(_) => self.handle_sync_report_key(key),
            Screen::Import(_) => self.handle_import_key(key),
            Screen::Export(_) => self.handle_export_key(key),
            Screen::Settings(_) => self.handle_settings_key(key),
        }
    }

    fn request_quit(&mut self) {
        let unsaved = match &self.screen {
            Screen::Editor(editor) => editor.is_dirty(),
            Screen::Settings(settings) => settings.is_dirty(),
            _ => false,
        };
        if unsaved {
            self.modal = Some(Modal::ConfirmQuit);
            self.dirty = true;
        } else {
            self.should_quit = true;
        }
    }

    fn move_selection(&mut self, delta: i64) {
        if self.visible.is_empty() {
            self.selected = 0;
            return;
        }
        let next = self.selected as i64 + delta;
        self.selected = next.clamp(0, self.visible.len() as i64 - 1) as usize;
        self.detail_scroll = 0;
        self.dirty = true;
    }

    fn page(&mut self, pages: i64) {
        let step = self.list_height.max(1) as i64 * pages;
        self.move_selection(step);
    }

    fn handle_browser_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return;
        }
        if self.filter.active {
            match key.code {
                KeyCode::Enter => {
                    self.filter.active = false;
                    self.recompute_visible();
                    self.dirty = true;
                    return;
                }
                KeyCode::Esc => {
                    self.filter.active = false;
                    self.clear_filters();
                    return;
                }
                KeyCode::Up => {
                    self.move_selection(-1);
                    return;
                }
                KeyCode::Down => {
                    self.move_selection(1);
                    return;
                }
                _ => {
                    let changed = self.filter.input.handle_key(key);
                    if changed {
                        self.recompute_visible();
                    }
                    self.dirty = true;
                    return;
                }
            }
        }
        if self.focus == Focus::Detail {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                    self.dirty = true;
                    return;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                    self.dirty = true;
                    return;
                }
                KeyCode::PageDown => {
                    self.detail_scroll =
                        self.detail_scroll.saturating_add(self.detail_height.max(1));
                    self.dirty = true;
                    return;
                }
                KeyCode::PageUp => {
                    self.detail_scroll =
                        self.detail_scroll.saturating_sub(self.detail_height.max(1));
                    self.dirty = true;
                    return;
                }
                KeyCode::Esc => {
                    self.focus = Focus::List;
                    if !self.wide_layout {
                        self.show_detail = false;
                    }
                    // A single Esc from detail focus both returns to the list
                    // and clears any applied filters.
                    if self.has_applied_filters() {
                        self.clear_filters();
                    } else {
                        self.dirty = true;
                    }
                    return;
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    self.focus = Focus::List;
                    self.dirty = true;
                    return;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char('q') => self.request_quit(),
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::PageDown => self.page(1),
            KeyCode::PageUp => self.page(-1),
            KeyCode::Home | KeyCode::Char('g') => self.move_selection(-(self.visible.len() as i64)),
            KeyCode::End | KeyCode::Char('G') => self.move_selection(self.visible.len() as i64),
            KeyCode::Char('/') => {
                self.filter.active = true;
                self.dirty = true;
            }
            KeyCode::Char('n') => {
                self.screen = Screen::Editor(Box::new(Editor::new(None)));
                self.dirty = true;
            }
            KeyCode::Char('e') => self.edit_selected(),
            KeyCode::Char('r') => self.open_rename(),
            KeyCode::Char('p') => self.open_preview(),
            KeyCode::Char('d') => self.open_delete(),
            KeyCode::Char('s') => self.start_sync(),
            KeyCode::Char('D') => self.start_doctor(),
            KeyCode::Char('R') => self.manual_refresh(),
            KeyCode::Char('I') => self.open_import(),
            KeyCode::Char('X') => self.open_export(),
            KeyCode::Char('S') => self.open_settings(),
            KeyCode::Char('?') => self.open_help(),
            KeyCode::Char('t') => self.open_tag_filter(),
            KeyCode::Char('o') => self.cycle_sort(false),
            KeyCode::Char('O') => self.toggle_descending(),
            KeyCode::Char('F') => {
                self.filter.fuzzy = !self.filter.fuzzy;
                self.recompute_visible();
                self.set_status(
                    StatusKind::Info,
                    format!(
                        "fuzzy search {}",
                        if self.filter.fuzzy { "on" } else { "off" }
                    ),
                );
            }
            KeyCode::Char(' ') => self.toggle_selected_enabled(),
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::List => Focus::Detail,
                    Focus::Detail => Focus::List,
                };
                self.dirty = true;
            }
            KeyCode::Enter => {
                if self.wide_layout {
                    self.focus = Focus::Detail;
                } else {
                    self.show_detail = true;
                    self.focus = Focus::Detail;
                }
                self.dirty = true;
            }
            KeyCode::Esc if self.show_detail => {
                self.show_detail = false;
                self.focus = Focus::List;
                self.dirty = true;
            }
            KeyCode::Esc if self.has_applied_filters() => {
                self.clear_filters();
            }
            _ => {}
        }
    }

    pub fn has_applied_filters(&self) -> bool {
        !self.filter.input.is_empty()
            || !self.filter.tags.is_empty()
            || self.filter.fuzzy
            || self.filter.descending
            || self.filter.sort != SortField::Name
    }

    pub fn clear_filters(&mut self) {
        self.filter = FilterState::default();
        self.selected = 0;
        self.list_offset = 0;
        self.recompute_visible();
        self.set_status(StatusKind::Info, "filters cleared; showing all aliases");
    }

    fn edit_selected(&mut self) {
        let Some(alias) = self.selected_alias_cloned() else {
            self.set_status(StatusKind::Warning, "no alias selected");
            return;
        };
        self.screen = Screen::Editor(Box::new(Editor::new(Some(&alias))));
        self.dirty = true;
    }

    fn open_rename(&mut self) {
        let Some(alias) = self.selected_alias_cloned() else {
            self.set_status(StatusKind::Warning, "no alias selected");
            return;
        };
        let error = self.rename_error(&alias, &alias.name);
        self.modal = Some(Modal::Rename {
            input: TextInput::new(alias.name.clone()),
            alias: Box::new(alias),
            error,
        });
        self.dirty = true;
    }

    fn open_delete(&mut self) {
        let Some(alias) = self.selected_alias_cloned() else {
            self.set_status(StatusKind::Warning, "no alias selected");
            return;
        };
        self.modal = Some(Modal::ConfirmDelete {
            alias: Box::new(alias),
        });
        self.dirty = true;
    }

    fn open_help(&mut self) {
        self.screen = Screen::Help(ScrollState {
            title: "Help".to_string(),
            lines: help_lines(),
            scroll: 0,
        });
        self.dirty = true;
    }

    fn open_tag_filter(&mut self) {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for alias in &self.snapshot.aliases {
            for tag in &alias.tags {
                *counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
        let entries = counts
            .into_keys()
            .map(|tag| {
                let selected = self.filter.tags.iter().any(|value| value == &tag);
                (tag, selected)
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            self.set_status(StatusKind::Info, "no tags in the database");
            return;
        }
        self.modal = Some(Modal::TagFilter { entries, cursor: 0 });
        self.dirty = true;
    }

    fn cycle_sort(&mut self, backwards: bool) {
        let fields = [
            SortField::Name,
            SortField::UpdatedAt,
            SortField::CreatedAt,
            SortField::TargetType,
            SortField::Enabled,
        ];
        let index = fields
            .iter()
            .position(|field| *field == self.filter.sort)
            .unwrap_or(0);
        let next = if backwards {
            (index + fields.len() - 1) % fields.len()
        } else {
            (index + 1) % fields.len()
        };
        self.filter.sort = fields[next];
        self.filter.descending = false;
        self.recompute_visible();
        self.set_status(
            StatusKind::Info,
            format!("sort: {} (ascending)", sort_label(self.filter.sort)),
        );
    }

    fn toggle_descending(&mut self) {
        self.filter.descending = !self.filter.descending;
        self.recompute_visible();
        self.set_status(
            StatusKind::Info,
            format!(
                "sort: {} ({})",
                sort_label(self.filter.sort),
                if self.filter.descending {
                    "descending"
                } else {
                    "ascending"
                }
            ),
        );
    }

    fn open_import(&mut self) {
        self.screen = Screen::Import(ImportState::new());
        self.dirty = true;
    }

    fn open_export(&mut self) {
        self.screen = Screen::Export(ExportState::new());
        self.dirty = true;
    }

    fn open_settings(&mut self) {
        match SettingsState::new(&self.root) {
            Ok(state) => {
                self.screen = Screen::Settings(Box::new(state));
                self.dirty = true;
            }
            Err(error) => {
                self.set_status(
                    StatusKind::Error,
                    format!("could not load config.toml: {error}"),
                );
            }
        }
    }

    fn handle_settings_key(&mut self, key: KeyEvent) {
        let (outcome, dirty) = if let Screen::Settings(state) = &mut self.screen {
            (state.handle_key(key), state.is_dirty())
        } else {
            return;
        };
        self.dirty = true;
        match outcome {
            SettingsOutcome::None | SettingsOutcome::Dirty => {}
            SettingsOutcome::SaveRequested => self.save_settings(),
            SettingsOutcome::CancelRequested => {
                if dirty {
                    self.modal = Some(Modal::ConfirmDiscard);
                } else {
                    self.screen = Screen::Browse;
                }
            }
        }
    }

    fn save_settings(&mut self) {
        if self.writes_blocked() {
            return;
        }
        let result = match &mut self.screen {
            Screen::Settings(state) => state.save(),
            _ => return,
        };
        match result {
            Ok(()) => self.set_status(
                StatusKind::Success,
                "settings saved to the shared config.toml; CLI/GUI see the same values (restart the shell after changing the default shell)",
            ),
            Err(error) => self.set_status(
                StatusKind::Error,
                format!(
                    "{}: {}",
                    error
                        .field
                        .map(|field| field.label())
                        .unwrap_or("config.toml"),
                    error.message
                ),
            ),
        }
        self.dirty = true;
    }

    fn handle_import_key(&mut self, key: KeyEvent) {
        let action = match &mut self.screen {
            Screen::Import(state) => state.handle_key(key),
            _ => return,
        };
        self.dirty = true;
        match action {
            ImportAction::None => {}
            ImportAction::Back => self.screen = Screen::Browse,
            ImportAction::Preview { path, strategy } => {
                self.start_import_preview(path, strategy);
            }
            ImportAction::Confirm { path, strategy } => {
                self.start_import_confirm(path, strategy);
            }
        }
    }

    fn handle_export_key(&mut self, key: KeyEvent) {
        let action = match &mut self.screen {
            Screen::Export(state) => state.handle_key(key),
            _ => return,
        };
        self.dirty = true;
        match action {
            ExportAction::None => {}
            ExportAction::Back => self.screen = Screen::Browse,
            ExportAction::Start { path, overwrite } => self.start_export(path, overwrite),
        }
    }

    fn start_import_preview(
        &mut self,
        path: PathBuf,
        strategy: aliasmgr_core::transfer::ConflictStrategy,
    ) {
        if self.jobs.is_busy() {
            self.set_status(StatusKind::Warning, "a background job is already running");
            return;
        }
        if let Screen::Import(state) = &mut self.screen {
            state.busy = true;
            state.error = None;
        }
        if self
            .jobs
            .start(JobKind::ImportPreview { path, strategy }, self.root.clone())
        {
            self.set_status(
                StatusKind::Info,
                "reading and validating the import file...",
            );
        } else if let Screen::Import(state) = &mut self.screen {
            state.busy = false;
            state.error = Some("could not start the import preview worker".to_string());
        }
    }

    fn start_import_confirm(
        &mut self,
        path: PathBuf,
        strategy: aliasmgr_core::transfer::ConflictStrategy,
    ) {
        if self.jobs.is_busy() {
            self.set_status(StatusKind::Warning, "a background job is already running");
            return;
        }
        if let Screen::Import(state) = &mut self.screen {
            state.busy = true;
            state.error = None;
        }
        if self
            .jobs
            .start(JobKind::ImportConfirm { path, strategy }, self.root.clone())
        {
            self.set_status(
                StatusKind::Info,
                "importing accepted records in one transaction...",
            );
        } else if let Screen::Import(state) = &mut self.screen {
            state.busy = false;
            state.error = Some("could not start the import worker".to_string());
        }
    }

    fn start_export(&mut self, path: PathBuf, overwrite: bool) {
        if self.jobs.is_busy() {
            self.set_status(StatusKind::Warning, "a background job is already running");
            return;
        }
        if let Screen::Export(state) = &mut self.screen {
            state.busy = true;
            state.error = None;
        }
        if self
            .jobs
            .start(JobKind::Export { path, overwrite }, self.root.clone())
        {
            self.set_status(StatusKind::Info, "exporting aliases...");
        } else if let Screen::Export(state) = &mut self.screen {
            state.busy = false;
            state.error = Some("could not start the export worker".to_string());
        }
    }

    fn open_preview(&mut self) {
        let Some(alias) = self.selected_alias_cloned() else {
            self.set_status(StatusKind::Warning, "no alias selected");
            return;
        };
        let shells = alias.shells.clone();
        let mut state = PreviewState {
            alias: Box::new(alias),
            shells,
            index: 0,
            body: String::new(),
            error: None,
            scroll: 0,
            mode: PreviewMode::Alias,
        };
        self.fill_preview(&mut state);
        self.screen = Screen::Preview(state);
        self.dirty = true;
    }

    fn fill_preview(&self, state: &mut PreviewState) {
        let Some(shell) = state.shell().cloned() else {
            state.error = Some("this alias has no shells".to_string());
            state.body.clear();
            return;
        };
        let result = match state.mode {
            PreviewMode::Alias => data::preview_alias(&state.alias, &shell),
            PreviewMode::File => {
                let aliases = data::with_draft(&self.snapshot.aliases, None);
                data::preview_shell(&self.root, &self.snapshot, &shell, &aliases)
            }
        };
        match result {
            Ok(body) => {
                state.body = body;
                state.error = None;
            }
            Err(error) => {
                state.body.clear();
                state.error = Some(error.to_string());
            }
        }
    }

    fn handle_preview_key(&mut self, key: KeyEvent) {
        let mut back = false;
        let mut quit = false;
        let mut switch: i64 = 0;
        let mut toggle_mode = false;
        let page = self.preview_height.max(1) as u16;
        let end = self.preview_max_scroll;
        {
            let Screen::Preview(state) = &mut self.screen else {
                return;
            };
            match key.code {
                KeyCode::Esc => back = true,
                KeyCode::Char('q') => quit = true,
                KeyCode::Char('j') | KeyCode::Down => {
                    state.scroll = state.scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    state.scroll = state.scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    state.scroll = state.scroll.saturating_add(page).min(end);
                }
                KeyCode::PageUp => {
                    state.scroll = state.scroll.saturating_sub(page);
                }
                KeyCode::Home | KeyCode::Char('g') => state.scroll = 0,
                KeyCode::End | KeyCode::Char('G') => state.scroll = end,
                KeyCode::Char('f') => toggle_mode = true,
                KeyCode::Tab | KeyCode::Right | KeyCode::Char(']') => switch = 1,
                KeyCode::BackTab | KeyCode::Left | KeyCode::Char('[') => switch = -1,
                _ => {}
            }
        }
        if back {
            self.screen = Screen::Browse;
        } else if quit {
            self.request_quit();
        } else if switch != 0 {
            if let Screen::Preview(state) = &mut self.screen {
                if !state.shells.is_empty() {
                    let len = state.shells.len() as i64;
                    state.index = (state.index as i64 + switch).rem_euclid(len) as usize;
                    state.scroll = 0;
                }
            }
            self.refresh_preview_body();
        } else if toggle_mode {
            if let Screen::Preview(state) = &mut self.screen {
                state.mode = match state.mode {
                    PreviewMode::Alias => PreviewMode::File,
                    PreviewMode::File => PreviewMode::Alias,
                };
                state.scroll = 0;
            }
            self.refresh_preview_body();
        }
        self.dirty = true;
    }

    fn refresh_preview_body(&mut self) {
        let mut state = match std::mem::replace(&mut self.screen, Screen::Browse) {
            Screen::Preview(state) => state,
            other => {
                self.screen = other;
                return;
            }
        };
        self.fill_preview(&mut state);
        self.screen = Screen::Preview(state);
    }

    fn handle_doctor_key(&mut self, key: KeyEvent) {
        let mut action = None;
        let page = self.doctor_height.max(1) as u16;
        let end = self.doctor_max_scroll;
        {
            let Screen::Doctor(state) = &mut self.screen else {
                return;
            };
            match key.code {
                KeyCode::Esc => action = Some(DoctorAction::Back),
                KeyCode::Char('q') => action = Some(DoctorAction::Quit),
                KeyCode::Char('s') => action = Some(DoctorAction::Sync),
                KeyCode::Char('r') => action = Some(DoctorAction::Rescan),
                KeyCode::Char('j') | KeyCode::Down => {
                    if !state.findings.is_empty() {
                        state.selected = (state.selected + 1).min(state.findings.len() - 1);
                        state.scroll = state.scroll.saturating_add(1);
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    state.selected = state.selected.saturating_sub(1);
                    state.scroll = state.scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    state.scroll = state.scroll.saturating_add(page).min(end);
                }
                KeyCode::PageUp => {
                    state.scroll = state.scroll.saturating_sub(page);
                }
                KeyCode::Home | KeyCode::Char('g') => state.scroll = 0,
                KeyCode::End | KeyCode::Char('G') => state.scroll = end,
                _ => {}
            }
        }
        match action {
            Some(DoctorAction::Back) => self.screen = Screen::Browse,
            Some(DoctorAction::Quit) => self.request_quit(),
            Some(DoctorAction::Sync) => self.start_sync(),
            Some(DoctorAction::Rescan) => self.start_doctor(),
            None => {}
        }
        self.dirty = true;
    }

    fn handle_sync_report_key(&mut self, key: KeyEvent) {
        let mut back = false;
        let mut quit = false;
        let mut retry = false;
        match &mut self.screen {
            Screen::SyncReport(state) => {
                let max = state.run.results.len() * 4 + 6;
                match key.code {
                    KeyCode::Char('r') | KeyCode::Char('R') => retry = true,
                    _ => match scroll_event(&key, &mut state.scroll, max) {
                        ScrollAction::Back => back = true,
                        ScrollAction::Quit => quit = true,
                        ScrollAction::None => {}
                    },
                }
            }
            _ => return,
        }
        if back {
            self.screen = Screen::Browse;
        } else if quit {
            self.request_quit();
        } else if retry {
            self.start_retry_failed();
        }
        self.dirty = true;
    }

    fn handle_scroll_key(&mut self, key: KeyEvent) {
        let mut back = false;
        let mut quit = false;
        match &mut self.screen {
            Screen::Help(state) => {
                let max = state.lines.len();
                match scroll_event(&key, &mut state.scroll, max) {
                    ScrollAction::Back => back = true,
                    ScrollAction::Quit => quit = true,
                    ScrollAction::None => {}
                }
            }
            Screen::SyncReport(state) => {
                let max = state.run.results.len() * 4 + 6;
                match scroll_event(&key, &mut state.scroll, max) {
                    ScrollAction::Back => back = true,
                    ScrollAction::Quit => quit = true,
                    ScrollAction::None => {}
                }
            }
            _ => return,
        }
        if back {
            self.screen = Screen::Browse;
        } else if quit {
            self.request_quit();
        }
        self.dirty = true;
    }

    fn handle_modal(&mut self, modal: Modal, key: KeyEvent) {
        self.dirty = true;
        match modal {
            Modal::ConfirmDelete { alias } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.perform_delete(*alias),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.set_status(StatusKind::Info, "delete cancelled");
                }
                _ => self.modal = Some(Modal::ConfirmDelete { alias }),
            },
            Modal::ConfirmQuit => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.should_quit = true,
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.set_status(StatusKind::Info, "quit cancelled");
                }
                _ => self.modal = Some(Modal::ConfirmQuit),
            },
            Modal::ConfirmDiscard => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.screen = Screen::Browse;
                    self.set_status(StatusKind::Info, "local changes discarded");
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.set_status(StatusKind::Info, "kept local changes");
                }
                _ => self.modal = Some(Modal::ConfirmDiscard),
            },
            Modal::Rename {
                alias,
                mut input,
                mut error,
            } => match key.code {
                KeyCode::Enter => {
                    error = self.rename_error(&alias, input.value());
                    if let Some(message) = &error {
                        self.set_status(StatusKind::Error, format!("rename blocked: {message}"));
                        self.modal = Some(Modal::Rename {
                            alias,
                            input,
                            error,
                        });
                    } else {
                        self.perform_rename(*alias, input.value().to_string());
                    }
                }
                KeyCode::Esc => {
                    self.set_status(StatusKind::Info, "rename cancelled");
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    self.modal = Some(Modal::Rename {
                        alias,
                        input,
                        error,
                    });
                }
                _ => {
                    input.handle_key(key);
                    error = self.rename_error(&alias, input.value());
                    self.modal = Some(Modal::Rename {
                        alias,
                        input,
                        error,
                    });
                }
            },
            Modal::Deleted { name } => match key.code {
                KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Esc => {
                    self.screen = Screen::Browse;
                    self.reload();
                    self.set_status(
                        StatusKind::Info,
                        format!("{name} was deleted externally; draft discarded"),
                    );
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if let Screen::Editor(editor) = &mut self.screen {
                        editor.is_new = true;
                        editor.base_revision = 0;
                        editor.saving = false;
                        editor.conflict_revision = None;
                    }
                    self.saving = false;
                    self.save_editor();
                }
                _ => self.modal = Some(Modal::Deleted { name }),
            },
            Modal::TagFilter {
                mut entries,
                mut cursor,
            } => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    cursor = cursor.saturating_sub(1);
                    self.modal = Some(Modal::TagFilter { entries, cursor });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    cursor = (cursor + 1).min(entries.len().saturating_sub(1));
                    self.modal = Some(Modal::TagFilter { entries, cursor });
                }
                KeyCode::Char(' ') | KeyCode::Char('x') => {
                    if let Some((_tag, checked)) = entries.get_mut(cursor) {
                        *checked = !*checked;
                    }
                    self.modal = Some(Modal::TagFilter { entries, cursor });
                }
                KeyCode::Char('c') => {
                    for (_tag, checked) in &mut entries {
                        *checked = false;
                    }
                    self.modal = Some(Modal::TagFilter { entries, cursor });
                }
                KeyCode::Enter => {
                    self.filter.tags = entries
                        .into_iter()
                        .filter_map(|(tag, checked)| checked.then_some(tag))
                        .collect();
                    self.recompute_visible();
                    let count = self.filter.tags.len();
                    self.set_status(
                        StatusKind::Info,
                        if count == 0 {
                            "tag filter cleared".to_string()
                        } else {
                            format!("tag filter: {}", self.filter.tags.join(", "))
                        },
                    );
                }
                KeyCode::Esc => {
                    self.set_status(StatusKind::Info, "tag filter unchanged");
                }
                _ => self.modal = Some(Modal::TagFilter { entries, cursor }),
            },
            Modal::Conflict { current, .. } => match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.external_pending = false;
                    if let Screen::Editor(editor) = &mut self.screen {
                        editor.reload_from(&current);
                    }
                    self.set_status(
                        StatusKind::Info,
                        "loaded the latest revision; local changes were discarded",
                    );
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.external_pending = false;
                    self.screen = Screen::Browse;
                    self.set_status(StatusKind::Info, "local changes discarded");
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    self.external_pending = false;
                    if let Screen::Editor(editor) = &mut self.screen {
                        editor.base_revision = current.revision;
                        editor.conflict_revision = None;
                        editor.saving = false;
                    }
                    self.saving = false;
                    self.save_editor();
                }
                _ => {
                    self.set_status(
                        StatusKind::Warning,
                        "conflict not resolved: reload, discard, or explicitly overwrite",
                    );
                }
            },
            Modal::Message { title, lines } => match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                    let _ = (title, lines);
                }
                _ => self.modal = Some(Modal::Message { title, lines }),
            },
        }
    }

    pub fn save_editor(&mut self) {
        if self.writes_blocked() {
            return;
        }
        if self.saving {
            self.set_status(StatusKind::Warning, "save already in progress");
            return;
        }
        if let Screen::Editor(editor) = &self.screen {
            if editor.saving {
                self.set_status(StatusKind::Warning, "save already in progress");
                return;
            }
        }
        let prepared = match &mut self.screen {
            Screen::Editor(editor) => {
                if editor.saving {
                    return;
                }
                match editor.draft.to_record(&editor.base) {
                    Ok(record) => {
                        editor.error = None;
                        editor.saving = true;
                        Some((
                            record,
                            editor.is_new,
                            editor.base_revision,
                            editor.base.clone(),
                        ))
                    }
                    Err(error) => {
                        editor.error = Some(error);
                        None
                    }
                }
            }
            _ => None,
        };
        let Some((record, is_new, base_revision, base)) = prepared else {
            if let Screen::Editor(editor) = &self.screen {
                if let Some(error) = &editor.error {
                    self.set_status(
                        StatusKind::Error,
                        format!(
                            "{}: {}",
                            error.field.map(Field::label).unwrap_or("record"),
                            error.message
                        ),
                    );
                }
            }
            return;
        };
        self.saving = true;
        let record_id = record.id;
        let service = AliasService::new(&self.database);
        let result = if is_new {
            service.create(record)
        } else {
            service.update_checked(record, base_revision)
        };
        self.saving = false;
        match result {
            Ok(saved) => {
                let name = saved.name.clone();
                let revision = saved.revision;
                self.reload();
                self.screen = Screen::Browse;
                self.set_status(
                    StatusKind::Success,
                    format!(
                        "saved {name} (revision {revision}); generated files are stale - press s to sync"
                    ),
                );
            }
            Err(AliasError::AliasConflict(message)) => {
                let current = self.database.get_alias(record_id).ok().flatten();
                if let (Screen::Editor(editor), Some(current)) = (&mut self.screen, current) {
                    editor.saving = false;
                    editor.conflict_revision = Some(current.revision);
                    self.modal = Some(Modal::Conflict {
                        base: Box::new(base),
                        current: Box::new(current),
                    });
                } else {
                    if let Screen::Editor(editor) = &mut self.screen {
                        editor.saving = false;
                    }
                }
                self.set_status(
                    StatusKind::Warning,
                    format!("not saved: {message}. Choose reload, discard, or overwrite"),
                );
            }
            Err(AliasError::NotFound(message)) if !is_new => {
                let name = match &self.screen {
                    Screen::Editor(editor) => editor.draft.name.clone(),
                    _ => message,
                };
                if let Screen::Editor(editor) = &mut self.screen {
                    editor.saving = false;
                    editor.error = None;
                }
                self.modal = Some(Modal::Deleted { name: name.clone() });
                self.set_status(
                    StatusKind::Warning,
                    format!(
                        "{name} was deleted in another front end; discard the draft or recreate it"
                    ),
                );
            }
            Err(error) => {
                let message = error.to_string();
                if let Screen::Editor(editor) = &mut self.screen {
                    editor.saving = false;
                    editor.error = Some(DraftError::from(error));
                }
                self.set_status(StatusKind::Error, format!("not saved: {message}"));
            }
        }
        self.dirty = true;
    }

    /// Background sync/doctor/loader work is the only concurrent writer. Keep
    /// editor saves and other writes out of that window so one write wins at a
    /// time and the UI never reports a result built from a moving database.
    fn writes_blocked(&mut self) -> bool {
        if self.jobs.is_busy() {
            self.set_status(
                StatusKind::Warning,
                "a background job is running; wait for it to finish before writing",
            );
            true
        } else {
            false
        }
    }

    fn perform_delete(&mut self, alias: AliasRecord) {
        if self.writes_blocked() {
            return;
        }
        let service = AliasService::new(&self.database);
        match service.delete_checked(&alias, alias.revision) {
            Ok(true) => {
                self.reload();
                self.set_status(
                    StatusKind::Success,
                    format!("deleted {}; press s to sync the removal", alias.name),
                );
            }
            Ok(false) => {
                self.reload();
                self.set_status(StatusKind::Warning, "alias was already deleted");
            }
            Err(AliasError::AliasConflict(message)) => {
                self.reload();
                self.set_status(
                    StatusKind::Error,
                    format!("not deleted: {message}. Refresh and try again"),
                );
            }
            Err(error) => {
                self.set_status(StatusKind::Error, format!("not deleted: {error}"));
            }
        }
        self.dirty = true;
    }

    /// Core-backed live validation for the rename modal. The same rules that
    /// `rename_checked` enforces are checked while typing, without touching the
    /// database.
    pub fn rename_error(&self, alias: &AliasRecord, new_name: &str) -> Option<String> {
        if new_name == alias.name {
            return None;
        }
        if let Err(error) = validate_alias_name(new_name) {
            return Some(error.to_string());
        }
        let powershell = alias
            .shells
            .iter()
            .any(|shell| matches!(shell, ShellKind::PowerShell5 | ShellKind::PowerShell7));
        if powershell {
            if let Err(error) = validate_powershell_name(new_name) {
                return Some(error.to_string());
            }
        }
        match self.database.get_alias_by_name(new_name) {
            Ok(Some(existing)) if existing.id != alias.id => {
                return Some(format!("alias name already exists: {}", existing.name));
            }
            Ok(_) => {}
            Err(error) => return Some(error.to_string()),
        }
        if powershell {
            match self
                .database
                .has_powershell_case_conflict(new_name, Some(alias.id))
            {
                Ok(true) => return Some("PowerShell case-fold alias conflict".to_string()),
                Ok(false) => {}
                Err(error) => return Some(error.to_string()),
            }
        }
        None
    }

    fn perform_rename(&mut self, alias: AliasRecord, new_name: String) {
        if self.writes_blocked() {
            return;
        }
        if new_name == alias.name {
            self.set_status(
                StatusKind::Info,
                format!("{new_name} already has this name; no rename performed"),
            );
            return;
        }
        let service = AliasService::new(&self.database);
        match service.rename_checked(&alias, &new_name, alias.revision) {
            Ok(saved) => {
                self.reload();
                self.set_status(
                    StatusKind::Success,
                    format!(
                        "renamed {} to {}; press s to sync the old-name cleanup",
                        alias.name, saved.name
                    ),
                );
            }
            Err(AliasError::AliasConflict(message)) => {
                self.reload();
                self.set_status(
                    StatusKind::Error,
                    format!("not renamed: {message}. Refresh and try again"),
                );
            }
            Err(AliasError::NotFound(_)) => {
                self.reload();
                self.set_status(
                    StatusKind::Error,
                    "not renamed: the alias was deleted in another front end; list refreshed",
                );
            }
            Err(error) => {
                self.set_status(StatusKind::Error, format!("not renamed: {error}"));
            }
        }
        self.dirty = true;
    }

    fn toggle_selected_enabled(&mut self) {
        if self.writes_blocked() {
            return;
        }
        let Some(alias) = self.selected_alias_cloned() else {
            self.set_status(StatusKind::Warning, "no alias selected");
            return;
        };
        let target = !alias.enabled;
        let service = AliasService::new(&self.database);
        match service.set_enabled_checked(&alias, target, alias.revision) {
            Ok(saved) => {
                self.reload();
                self.set_status(
                    StatusKind::Success,
                    format!(
                        "{} {}; press s to sync",
                        if saved.enabled { "enabled" } else { "disabled" },
                        saved.name
                    ),
                );
            }
            Err(error) => {
                self.reload();
                self.set_status(StatusKind::Error, format!("not changed: {error}"));
            }
        }
        self.dirty = true;
    }

    pub fn start_sync(&mut self) {
        if self.jobs.is_busy() {
            self.set_status(
                StatusKind::Warning,
                "another background job is running; wait for it to finish",
            );
            return;
        }
        if self.jobs.start(JobKind::Sync, self.root.clone()) {
            self.set_status(
                StatusKind::Info,
                "syncing in the background; the sync lock is held by core",
            );
        } else {
            self.set_status(StatusKind::Error, "could not start the sync worker");
        }
    }

    /// Retries only the failed shells of the current sync report. Successful
    /// shells are not rewritten; core applies the same lock/journal rules.
    pub fn start_retry_failed(&mut self) {
        if self.jobs.is_busy() {
            self.set_status(
                StatusKind::Warning,
                "another background job is running; wait for it to finish",
            );
            return;
        }
        let shells = match &self.screen {
            Screen::SyncReport(state) => state
                .run
                .results
                .iter()
                .filter(|result| result.status == ShellStatus::Failed)
                .map(|result| result.shell.clone())
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        if shells.is_empty() {
            self.set_status(StatusKind::Info, "no failed shells to retry");
            return;
        }
        if self
            .jobs
            .start(JobKind::SyncRetry(shells.clone()), self.root.clone())
        {
            self.set_status(
                StatusKind::Info,
                format!(
                    "retrying {} failed shell(s): {}",
                    shells.len(),
                    shells
                        .iter()
                        .map(|shell| shell.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        } else {
            self.set_status(StatusKind::Error, "could not start the retry worker");
        }
    }

    pub fn start_doctor(&mut self) {
        if self.jobs.is_busy() {
            self.set_status(
                StatusKind::Warning,
                "another background job is running; wait for it to finish",
            );
            return;
        }
        if !matches!(self.screen, Screen::Doctor(_)) {
            self.screen = Screen::Doctor(DoctorState {
                findings: Vec::new(),
                selected: 0,
                scroll: 0,
                busy: true,
                error: None,
            });
        } else if let Screen::Doctor(state) = &mut self.screen {
            state.busy = true;
            state.error = None;
        }
        if self.jobs.start(JobKind::Doctor, self.root.clone()) {
            self.set_status(StatusKind::Info, "running read-only diagnostics...");
        } else {
            if let Screen::Doctor(state) = &mut self.screen {
                state.busy = false;
                state.error = Some("could not start the doctor worker".to_string());
            }
        }
        self.dirty = true;
    }

    pub fn poll_jobs(&mut self) {
        let Some(outcome) = self.jobs.try_recv() else {
            return;
        };
        let retried = matches!(&outcome.kind, JobKind::SyncRetry(_));
        match outcome.result {
            JobResult::Sync(Ok(run)) => {
                let failed = run.failed();
                let count = run.results.len();
                let editor_open = matches!(self.screen, Screen::Editor(_));
                self.reload();
                if editor_open {
                    // Never discard an unsaved draft because a background job
                    // happened to finish. Keep the editor; the report is
                    // summarized in the status line.
                    if failed {
                        self.set_status(
                            StatusKind::Warning,
                            "sync finished with failures; editor kept; reload is still manual",
                        );
                    } else {
                        self.set_status(
                            StatusKind::Success,
                            format!("synced {count} shell(s); editor kept; reload is still manual"),
                        );
                    }
                } else {
                    self.screen = Screen::SyncReport(SyncReportState {
                        run,
                        scroll: 0,
                        retry: retried,
                    });
                    if failed {
                        self.set_status(
                            StatusKind::Warning,
                            if retried {
                                "retry finished with failures; reload is still manual"
                            } else {
                                "sync finished with failures; generated files may be stale; reload is still manual"
                            },
                        );
                    } else {
                        self.set_status(
                            StatusKind::Success,
                            if retried {
                                format!(
                                    "retried {count} failed shell(s); successful shells were not rewritten; reload is still manual"
                                )
                            } else {
                                format!(
                                    "synced {count} shell(s); run the shown reload command in this terminal"
                                )
                            },
                        );
                    }
                }
            }
            JobResult::Sync(Err(error)) => {
                self.set_status(
                    StatusKind::Error,
                    format!("sync failed: {error}. Another process may hold the sync lock"),
                );
            }
            JobResult::Doctor(Ok(findings)) => {
                if let Screen::Doctor(state) = &mut self.screen {
                    state.findings = findings;
                    state.selected = 0;
                    state.scroll = 0;
                    state.busy = false;
                    state.error = None;
                }
                let count = match &self.screen {
                    Screen::Doctor(state) => state.findings.len(),
                    _ => 0,
                };
                self.set_status(
                    StatusKind::Info,
                    format!("doctor finished with {count} finding(s)"),
                );
            }
            JobResult::Doctor(Err(error)) => {
                if let Screen::Doctor(state) = &mut self.screen {
                    state.busy = false;
                    state.error = Some(error.clone());
                }
                self.set_status(StatusKind::Error, format!("doctor failed: {error}"));
            }
            JobResult::ImportPreview(Ok(report)) => {
                let counts = (
                    report.imported.len(),
                    report.skipped.len(),
                    report.unsupported.len(),
                    report.warnings.len(),
                );
                if let Screen::Import(state) = &mut self.screen {
                    state.on_preview(report);
                }
                self.set_status(
                    StatusKind::Info,
                    format!(
                        "import preview: imported={} skipped={} unsupported={} warnings={}",
                        counts.0, counts.1, counts.2, counts.3
                    ),
                );
            }
            JobResult::ImportPreview(Err(error)) => {
                if let Screen::Import(state) = &mut self.screen {
                    state.on_error(error.clone());
                }
                self.set_status(StatusKind::Error, format!("import preview failed: {error}"));
            }
            JobResult::ImportConfirm(Ok(report)) => {
                let counts = (
                    report.imported.len(),
                    report.skipped.len(),
                    report.unsupported.len(),
                    report.warnings.len(),
                );
                self.reload();
                if let Screen::Import(state) = &mut self.screen {
                    state.on_confirmed(report);
                }
                self.set_status(
                    StatusKind::Success,
                    format!(
                        "imported {} alias(es); skipped={} unsupported={} warnings={} (single transaction)",
                        counts.0, counts.1, counts.2, counts.3
                    ),
                );
            }
            JobResult::ImportConfirm(Err(error)) => {
                if let Screen::Import(state) = &mut self.screen {
                    state.on_error(error.clone());
                }
                self.set_status(StatusKind::Error, format!("import failed: {error}"));
            }
            JobResult::Export(Ok(summary)) => {
                if let Screen::Export(state) = &mut self.screen {
                    state.on_result(summary.clone());
                }
                self.set_status(
                    StatusKind::Success,
                    format!(
                        "exported {} alias(es) to {} ({})",
                        summary.count,
                        summary.path.display(),
                        summary.format
                    ),
                );
            }
            JobResult::Export(Err(error)) => {
                if let Screen::Export(state) = &mut self.screen {
                    state.on_error(error.clone());
                }
                self.set_status(StatusKind::Error, format!("export failed: {error}"));
            }
            JobResult::WorkerFailed(message) => {
                if let Screen::Doctor(state) = &mut self.screen {
                    state.busy = false;
                    state.error = Some(message.clone());
                }
                self.set_status(
                    StatusKind::Error,
                    format!("background job failed: {message}"),
                );
            }
            JobResult::Loader(result) => match result {
                Ok(outcome) => {
                    self.reload();
                    self.set_status(
                        StatusKind::Success,
                        format!(
                            "{} loader {} ({})",
                            outcome.shell,
                            if outcome.changed {
                                "updated"
                            } else {
                                "unchanged"
                            },
                            outcome.target.display()
                        ),
                    );
                }
                Err(error) => {
                    self.set_status(
                        StatusKind::Error,
                        format!("loader operation failed: {error}"),
                    );
                }
            },
        }
        self.dirty = true;
    }
}

enum DoctorAction {
    Back,
    Quit,
    Sync,
    Rescan,
}

enum ScrollAction {
    None,
    Back,
    Quit,
}

fn scroll_event(key: &KeyEvent, scroll: &mut u16, max: usize) -> ScrollAction {
    match key.code {
        KeyCode::Esc => return ScrollAction::Back,
        KeyCode::Char('q') => return ScrollAction::Quit,
        KeyCode::Char('j') | KeyCode::Down => {
            *scroll = scroll.saturating_add(1).min(max as u16);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            *scroll = scroll.saturating_sub(1);
        }
        KeyCode::PageDown => {
            *scroll = scroll.saturating_add(10).min(max as u16);
        }
        KeyCode::PageUp => {
            *scroll = scroll.saturating_sub(10);
        }
        KeyCode::Home | KeyCode::Char('g') => *scroll = 0,
        KeyCode::End | KeyCode::Char('G') => *scroll = max as u16,
        _ => {}
    }
    ScrollAction::None
}

pub fn sort_label(field: SortField) -> &'static str {
    match field {
        SortField::Name => "name",
        SortField::UpdatedAt => "updated_at",
        SortField::CreatedAt => "created_at",
        SortField::TargetType => "target_type",
        SortField::Enabled => "enabled",
    }
}

pub fn filter_summary(filter: &FilterState) -> String {
    let mut parts = Vec::new();
    if !filter.input.is_empty() {
        parts.push(format!("search: {}", text::one_line(filter.input.value())));
    }
    if !filter.tags.is_empty() {
        parts.push(format!(
            "tags: {}",
            filter
                .tags
                .iter()
                .map(|tag| text::one_line(tag))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    parts.push(format!(
        "sort: {}{}",
        sort_label(filter.sort),
        if filter.descending { " desc" } else { "" }
    ));
    if filter.fuzzy {
        parts.push("fuzzy".to_string());
    }
    parts.join("  ")
}

/// Static help text. It documents the target-type semantics the editor renders
/// so the two cannot drift.
pub fn help_lines() -> Vec<String> {
    let mut lines = vec![
        "Alias Manager TUI".to_string(),
        String::new(),
        "BROWSE".to_string(),
        "  Up/Down or j/k    move        Enter   detail (narrow) / focus detail".to_string(),
        "  PageUp/PageDown   page        Home/End or g/G  first/last".to_string(),
        "  /                 search      F       toggle fuzzy search".to_string(),
        "  n                 new         e       edit".to_string(),
        "  r                 rename      Space   enable/disable".to_string(),
        "  d                 delete      s       sync (background)".to_string(),
        "  p                 preview     D       Doctor".to_string(),
        "  t                 tag filter  o / O   cycle sort / toggle descending".to_string(),
        "  R                 manual refresh        Esc     clear applied search/tag/sort filters"
            .to_string(),
        "  Tab               list/detail focus    ?  help    q / Ctrl+C  quit".to_string(),
        String::new(),
        "EDIT".to_string(),
        "  Tab/Shift+Tab     field       Ctrl+S  save      Esc  cancel".to_string(),
        "  text fields: printable characters are input, never shortcuts".to_string(),
        "  fixed args/tags/paths: a add, d delete, J/K reorder, Enter edit".to_string(),
        "  environment: a add key, Enter edit value (values are masked)".to_string(),
        "  Space/Enter toggles target type, shells, pass-args, path mode".to_string(),
        "  F3                show/hide advanced fields".to_string(),
        String::new(),
        "PREVIEW / DOCTOR".to_string(),
        "  j/k PgUp/PgDn Home/End scroll    Tab/[ ] switch preview shell".to_string(),
        "  preview: f toggles per-alias definition / whole shell file; rows show position"
            .to_string(),
        "  Doctor: j/k PgUp/PgDn Home/End scroll, s sync, r rescan, Esc back".to_string(),
        "  Sync results: r retries only the failed shells".to_string(),
        String::new(),
        "IMPORT / EXPORT / SETTINGS".to_string(),
        "  I: type a path, Enter previews, c changes conflict strategy, y imports, Esc cancels"
            .to_string(),
        "  X: type a path, Enter exports; an existing file requires y overwrite confirmation"
            .to_string(),
        "  S: Tab field, Left/Right or Space toggle, Ctrl+S save, Esc cancel".to_string(),
        "  Import preview is read-only; confirmation re-evaluates and persists atomically."
            .to_string(),
        "  Sensitive environment values are filtered by core; config.toml is shared with CLI/GUI."
            .to_string(),
        String::new(),
        "TARGET TYPES".to_string(),
    ];
    for target in crate::model::target_type_choices() {
        lines.push(format!(
            "  {:<20} {}",
            crate::model::target_type_label(&target),
            crate::model::target_type_semantics(&target)
        ));
    }
    lines.extend([
        String::new(),
        "STATUS LAYERS".to_string(),
        "  1 DB saved    2 generated file    3 loader installed    4 current shell unknown".to_string(),
        "  Saving only writes the database. Sync writes generated files.".to_string(),
        "  Reloading the current terminal is always manual; the TUI never runs source/Invoke-Expression.".to_string(),
        String::new(),
        "NOTES".to_string(),
        "  Environment values are masked in details, previews and errors.".to_string(),
        "  Preview renders environment values as *** before the core renderer sees them.".to_string(),
        "  Terminal smaller than 80x24 shows a clear message, ignores other keys, and clears modals; resize to continue.".to_string(),
        "  No mouse, clipboard, Nerd Font or emoji dependency; ASCII fallback via ALIASMGR_TUI_ASCII=1.".to_string(),
        "  Ctrl+C quits (asks before discarding an unsaved editor draft).".to_string(),
    ]);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_export::{ExportPhase, ImportPhase};
    use crate::model::{visible_fields, Field};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-tui-app-{label}-{}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::SeqCst),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn seed(app: &App, name: &str, executable: &str) -> AliasRecord {
        AliasService::new(&app.database)
            .create(AliasRecord {
                name: name.into(),
                executable: executable.into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap()
    }

    fn app_with_two() -> (PathBuf, App) {
        let root = temp_root("two");
        let mut app = App::new(root.clone()).unwrap();
        seed(&app, "gs", "git");
        seed(&app, "ll", "ls");
        app.reload();
        (root, app)
    }

    #[test]
    fn startup_recovers_an_interrupted_sync_journal_under_the_sync_lock() {
        let root = temp_root("journal");
        std::fs::write(
            root.join("operation.journal"),
            "revision_from=0\nrevision_to=1\nstate=prepared\nbackups_json=[]\n",
        )
        .unwrap();
        let app = App::new(root.clone()).unwrap();
        assert!(!root.join("operation.journal").exists());
        assert!(app.status.text.contains("recovered an interrupted sync"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn an_unknown_journal_state_is_surfaced_without_blocking_startup() {
        let root = temp_root("unknown-journal");
        std::fs::write(
            root.join("operation.journal"),
            "revision_from=0\nrevision_to=1\nstate=half-written\nbackups_json=[]\n",
        )
        .unwrap();
        let app = App::new(root.clone()).unwrap();
        assert!(app.status.text.contains("could not be recovered"));
        assert!(app.status.text.contains("unknown operation journal state"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn an_unreadable_journal_is_surfaced_without_blocking_startup() {
        let root = temp_root("unreadable-journal");
        std::fs::create_dir(root.join("operation.journal")).unwrap();
        let app = App::new(root.clone()).unwrap();
        assert!(app.status.text.contains("could not be recovered"));
        assert!(app.status.text.contains("cannot inspect the journal"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_completed_sync_journal_is_left_alone_and_does_not_warn() {
        let root = temp_root("completed-journal");
        let path = root.join("operation.journal");
        std::fs::write(
            &path,
            "revision_from=1\nrevision_to=1\nstate=completed\nbackups_json=[]\n",
        )
        .unwrap();
        let app = App::new(root.clone()).unwrap();
        assert!(path.exists());
        assert!(!app.status.text.contains("recovered"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rename_modal_stays_bound_to_the_alias_it_was_opened_on() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('r')));
        if let Some(Modal::Rename { input, .. }) = &mut app.modal {
            input.set_value("renamed");
        }
        // Another front end deletes the record. The poll refreshes the list and
        // the selection falls onto the remaining alias.
        let other = Database::open(root.join("aliases.db")).unwrap();
        AliasService::new(&other).delete("gs").unwrap();
        app.last_external_check = Instant::now() - Duration::from_secs(5);
        app.poll_external_changes();
        assert_eq!(app.selected_alias().unwrap().name, "ll");
        app.handle_key(key(KeyCode::Enter));
        let names = app
            .snapshot
            .aliases
            .iter()
            .map(|alias| alias.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["ll"]);
        assert!(app.status.text.contains("not renamed"));
        drop(other);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_completion_keeps_an_open_editor_and_its_draft() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.draft.description = "unsaved draft".into();
        }
        let revision = app.snapshot.revision;
        app.jobs.inject(crate::data::JobOutcome {
            kind: JobKind::Sync,
            result: JobResult::Sync(Ok(SyncRun {
                revision,
                results: Vec::new(),
            })),
        });
        app.poll_jobs();
        let Screen::Editor(editor) = &app.screen else {
            panic!("sync completion replaced the open editor");
        };
        assert_eq!(editor.draft.description, "unsaved draft");
        assert!(app.status.text.contains("editor kept"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn external_deletion_while_editing_offers_discard_or_recreate() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.draft.description = "kept".into();
        }
        let other = Database::open(root.join("aliases.db")).unwrap();
        AliasService::new(&other).delete("gs").unwrap();
        app.save_editor();
        assert!(matches!(app.modal, Some(Modal::Deleted { .. })));
        app.handle_key(key(KeyCode::Char('d')));
        assert!(matches!(app.screen, Screen::Browse));
        assert!(AliasService::new(&app.database)
            .get_by_name("gs")
            .unwrap()
            .is_none());
        drop(other);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn external_deletion_can_be_recreated_from_the_draft() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.draft.description = "recreated carefully".into();
        }
        let other = Database::open(root.join("aliases.db")).unwrap();
        AliasService::new(&other).delete("gs").unwrap();
        app.save_editor();
        assert!(matches!(app.modal, Some(Modal::Deleted { .. })));
        app.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(app.screen, Screen::Browse));
        let restored = AliasService::new(&app.database)
            .get_by_name("gs")
            .unwrap()
            .expect("recreated alias");
        assert_eq!(restored.description, "recreated carefully");
        drop(other);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ctrl_s_in_browse_does_not_start_a_background_sync() {
        let (root, mut app) = app_with_two();
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(!app.jobs.is_busy());
        assert!(matches!(app.screen, Screen::Browse));
        assert!(app.modal.is_none());
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    fn wait_for_jobs(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            app.poll_jobs();
            if !app.jobs.is_busy() {
                break;
            }
            assert!(Instant::now() < deadline, "background job did not finish");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn type_path(app: &mut App, path: &std::path::Path) {
        for character in path.to_string_lossy().chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
    }

    fn select_name(app: &mut App, name: &str) {
        let position = app
            .visible
            .iter()
            .position(|index| app.snapshot.aliases[*index].name == name)
            .unwrap_or_else(|| panic!("alias {name} is not visible"));
        app.selected = position;
    }

    fn set_rename_input(app: &mut App, value: &str) {
        if let Some(Modal::Rename { input, .. }) = &mut app.modal {
            input.set_value(value);
        } else {
            panic!("rename modal");
        }
        // Trigger the live validation path, as a real key event would.
        app.handle_key(key(KeyCode::End));
    }

    fn rename_error(app: &App) -> Option<String> {
        match &app.modal {
            Some(Modal::Rename { error, .. }) => error.clone(),
            _ => None,
        }
    }

    #[test]
    fn per_alias_preview_ignores_an_unrelated_missing_target() {
        let root = temp_root("preview-per-alias");
        let mut app = App::new(root.clone()).unwrap();
        let service = AliasService::new(&app.database);
        service
            .create(AliasRecord {
                name: "good".into(),
                executable: "tool".into(),
                fixed_args: vec!["status".into()],
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        service
            .create(AliasRecord {
                name: "zz-bad".into(),
                executable: "/definitely/missing/tool".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        app.reload();
        select_name(&mut app, "good");
        app.handle_key(key(KeyCode::Char('p')));
        let Screen::Preview(state) = &app.screen else {
            panic!("preview");
        };
        assert_eq!(state.mode, PreviewMode::Alias);
        assert!(state.error.is_none(), "{:?}", state.error);
        assert!(state.body.contains("good"));
        assert!(!state.body.contains("zz-bad"));

        // The whole-file fallback is explicit and may show the unrelated error.
        app.handle_key(key(KeyCode::Char('f')));
        let Screen::Preview(state) = &app.screen else {
            panic!("preview");
        };
        assert_eq!(state.mode, PreviewMode::File);
        assert!(state
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("target is missing"));
        app.handle_key(key(KeyCode::Char('f')));
        let Screen::Preview(state) = &app.screen else {
            panic!("preview");
        };
        assert_eq!(state.mode, PreviewMode::Alias);
        assert!(state.error.is_none());
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn retry_after_partial_sync_targets_only_failed_shells() {
        let root = temp_root("retry-failed");
        let mut app = App::new(root.clone()).unwrap();
        AliasService::new(&app.database)
            .create(AliasRecord {
                name: "gs".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap();
        app.reload();
        app.screen = Screen::SyncReport(SyncReportState {
            run: SyncRun {
                revision: 1,
                results: vec![
                    aliasmgr_core::sync::ShellSyncOutcome {
                        shell: ShellKind::Bash,
                        status: ShellStatus::Ok,
                        error: None,
                        error_code: None,
                        generated_path: root.join("generated/bash.sh"),
                        loader_installed: true,
                        loader_error_code: None,
                    },
                    aliasmgr_core::sync::ShellSyncOutcome {
                        shell: ShellKind::Zsh,
                        status: ShellStatus::Failed,
                        error: Some("synthetic failure".into()),
                        error_code: Some("sync_failed".into()),
                        generated_path: root.join("generated/zsh.sh"),
                        loader_installed: true,
                        loader_error_code: None,
                    },
                ],
            },
            scroll: 0,
            retry: false,
        });
        app.handle_key(key(KeyCode::Char('r')));
        assert!(app.jobs.is_busy());
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            app.poll_jobs();
            if !app.jobs.is_busy() {
                break;
            }
            assert!(Instant::now() < deadline, "retry did not finish");
            std::thread::sleep(Duration::from_millis(20));
        }
        let Screen::SyncReport(state) = &app.screen else {
            panic!("sync report");
        };
        assert!(state.retry);
        assert_eq!(state.run.results.len(), 1);
        assert_eq!(state.run.results[0].shell, ShellKind::Zsh);
        assert!(!root.join("generated/bash.sh").exists());
        assert!(root.join("generated/zsh.sh").exists());
        assert!(app
            .status
            .text
            .contains("successful shells were not rewritten"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn subset_retry_keeps_database_integrity_with_the_tui_connection_open() {
        let root = temp_root("retry-integrity");
        let mut app = App::new(root.clone()).unwrap();
        AliasService::new(&app.database)
            .create(AliasRecord {
                name: "gs".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap();
        app.reload();
        let failed = |shell: ShellKind| aliasmgr_core::sync::ShellSyncOutcome {
            shell,
            status: ShellStatus::Failed,
            error: Some("synthetic failure".into()),
            error_code: Some("sync_failed".into()),
            generated_path: root.join("generated/zsh.sh"),
            loader_installed: true,
            loader_error_code: None,
        };
        // C3 reproduced WAL-reset corruption on SQLite 3.46 in exactly this
        // shape: the TUI connection stays open while the retry worker opens
        // another connection and checkpoints. The in-loop check below documents
        // the pre-exit "ok" C3 also saw; the corruption is produced at the final
        // connection close/WAL reset, so the post-close check after `drop(app)`
        // is the regression assertion.
        for _ in 0..3 {
            app.screen = Screen::SyncReport(SyncReportState {
                run: SyncRun {
                    revision: 1,
                    results: vec![
                        aliasmgr_core::sync::ShellSyncOutcome {
                            shell: ShellKind::Bash,
                            status: ShellStatus::Ok,
                            error: None,
                            error_code: None,
                            generated_path: root.join("generated/bash.sh"),
                            loader_installed: true,
                            loader_error_code: None,
                        },
                        failed(ShellKind::Zsh),
                    ],
                },
                scroll: 0,
                retry: false,
            });
            app.handle_key(key(KeyCode::Char('r')));
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                app.poll_jobs();
                if !app.jobs.is_busy() {
                    break;
                }
                assert!(Instant::now() < deadline, "retry did not finish");
                std::thread::sleep(Duration::from_millis(20));
            }
            let pre_exit_integrity: String = app
                .database
                .conn
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .unwrap();
            assert_eq!(pre_exit_integrity, "ok");
        }
        assert!(!root.join("generated/bash.sh").exists());
        assert!(root.join("generated/zsh.sh").exists());

        // Close the TUI connection (and every retry connection) before
        // inspecting the database. This is the shape C3 saw fail on 3.46:
        // integrity was ok before exit and malformed afterwards.
        drop(app);
        let reopened = Database::open_read_only(root.join("aliases.db")).unwrap();
        let post_close_integrity: String = reopened
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(post_close_integrity, "ok");
        drop(reopened);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn retry_with_no_failed_shells_is_a_no_op() {
        let (root, mut app) = app_with_two();
        app.screen = Screen::SyncReport(SyncReportState {
            run: SyncRun {
                revision: 1,
                results: Vec::new(),
            },
            scroll: 0,
            retry: false,
        });
        app.start_retry_failed();
        assert!(!app.jobs.is_busy());
        assert!(app.status.text.contains("no failed shells"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn manual_refresh_updates_timestamp_and_reports_external_changes() {
        let (root, mut app) = app_with_two();
        app.last_refresh = Instant::now() - Duration::from_secs(5);
        let before = app.last_refresh;
        app.manual_refresh();
        assert!(app.last_refresh > before);
        assert!(app.status.text.contains("no external changes"));

        let other = Database::open(root.join("aliases.db")).unwrap();
        AliasService::new(&other)
            .create(AliasRecord {
                name: "newone".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        let before = app.last_refresh;
        app.handle_key(key(KeyCode::Char('R')));
        assert!(app.last_refresh > before);
        assert!(app.status.text.contains("external changes applied"));
        assert!(app
            .snapshot
            .aliases
            .iter()
            .any(|alias| alias.name == "newone"));
        drop(other);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn renaming_to_the_unchanged_name_is_a_no_op() {
        let (root, mut app) = app_with_two();
        let alias = AliasService::new(&app.database)
            .get_by_name("gs")
            .unwrap()
            .unwrap();
        select_name(&mut app, "gs");
        app.handle_key(key(KeyCode::Char('r')));
        app.handle_key(key(KeyCode::Enter));
        assert!(app.status.text.contains("no rename performed"));
        assert!(matches!(app.screen, Screen::Browse));
        let after = AliasService::new(&app.database)
            .get_by_name("gs")
            .unwrap()
            .unwrap();
        assert_eq!(after.revision, alias.revision);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rename_validation_blocks_invalid_reserved_duplicate_and_casefold_names() {
        let (root, mut app) = app_with_two();
        select_name(&mut app, "gs");
        app.handle_key(key(KeyCode::Char('r')));

        set_rename_input(&mut app, "bad name");
        assert!(rename_error(&app).unwrap().contains("invalid alias name"));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.modal, Some(Modal::Rename { .. })));
        assert!(AliasService::new(&app.database)
            .get_by_name("gs")
            .unwrap()
            .is_some());

        set_rename_input(&mut app, "ll");
        assert!(rename_error(&app).unwrap().contains("already exists"));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.modal, Some(Modal::Rename { .. })));

        set_rename_input(&mut app, "gs2");
        assert!(rename_error(&app).is_none());
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.screen, Screen::Browse));
        assert!(AliasService::new(&app.database)
            .get_by_name("gs2")
            .unwrap()
            .is_some());

        // PowerShell reserved and case-fold rules.
        {
            let service = AliasService::new(&app.database);
            service
                .create(AliasRecord {
                    name: "Build".into(),
                    executable: "tool".into(),
                    shells: vec![ShellKind::PowerShell7],
                    ..Default::default()
                })
                .unwrap();
            service
                .create(AliasRecord {
                    name: "other".into(),
                    executable: "tool".into(),
                    shells: vec![ShellKind::PowerShell7],
                    ..Default::default()
                })
                .unwrap();
        }
        app.reload();
        select_name(&mut app, "other");
        app.handle_key(key(KeyCode::Char('r')));
        set_rename_input(&mut app, "ls");
        assert!(rename_error(&app).unwrap().contains("reserved"));
        set_rename_input(&mut app, "BUILD");
        assert!(rename_error(&app).unwrap().contains("case-fold"));
        set_rename_input(&mut app, "renamed-ps");
        assert!(rename_error(&app).is_none());
        app.handle_key(key(KeyCode::Enter));
        assert!(AliasService::new(&app.database)
            .get_by_name("renamed-ps")
            .unwrap()
            .is_some());
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn esc_clears_search_tag_sort_and_fuzzy_filters() {
        let (root, mut app) = app_with_two();
        // Search.
        app.handle_key(key(KeyCode::Char('/')));
        for character in "gs".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.visible.len(), 1);
        app.handle_key(key(KeyCode::Esc));
        assert!(app.filter.input.is_empty());
        assert_eq!(app.visible.len(), 2);
        assert!(app.status.text.contains("filters cleared"));

        // Sort and fuzzy.
        app.handle_key(key(KeyCode::Char('o')));
        app.handle_key(key(KeyCode::Char('F')));
        assert_eq!(app.filter.sort, SortField::UpdatedAt);
        assert!(app.filter.fuzzy);
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.filter.sort, SortField::Name);
        assert!(!app.filter.fuzzy);
        assert!(!app.filter.descending);

        // Tag filter (and a combined search/tag/sort state).
        let tagged = app
            .snapshot
            .aliases
            .iter()
            .find(|alias| alias.name == "gs")
            .cloned()
            .unwrap();
        let mut tagged = tagged;
        tagged.tags = vec!["git".into()];
        AliasService::new(&app.database).update(tagged).unwrap();
        app.reload();
        app.handle_key(key(KeyCode::Char('t')));
        app.handle_key(key(KeyCode::Char(' ')));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.filter.tags, vec!["git".to_string()]);
        assert_eq!(app.visible.len(), 1);
        app.handle_key(key(KeyCode::Esc));
        assert!(app.filter.tags.is_empty());
        assert_eq!(app.visible.len(), 2);

        // Combination: every filter cleared by one Esc.
        app.handle_key(key(KeyCode::Char('/')));
        for character in "gs".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('o')));
        app.handle_key(key(KeyCode::Char('F')));
        app.filter.tags = vec!["git".into()];
        app.recompute_visible();
        app.handle_key(key(KeyCode::Esc));
        assert!(app.filter.input.is_empty());
        assert!(app.filter.tags.is_empty());
        assert_eq!(app.filter.sort, SortField::Name);
        assert!(!app.filter.fuzzy);
        assert_eq!(app.visible.len(), 2);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn import_preview_then_confirm_persists_and_cancel_does_not() {
        let root = temp_root("import-flow");
        let mut app = App::new(root.clone()).unwrap();
        let file = root.join("input.json");
        aliasmgr_core::transfer::export_json(
            &file,
            &[AliasRecord {
                name: "imported".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        )
        .unwrap();

        app.handle_key(key(KeyCode::Char('I')));
        assert!(matches!(app.screen, Screen::Import(_)));
        type_path(&mut app, &file);
        app.handle_key(key(KeyCode::Enter));
        wait_for_jobs(&mut app);
        let Screen::Import(state) = &app.screen else {
            panic!("import screen");
        };
        assert_eq!(state.phase, ImportPhase::Preview);
        assert_eq!(state.report.as_ref().unwrap().imported, vec!["imported"]);
        assert!(app.database.list_aliases().unwrap().is_empty());

        app.handle_key(key(KeyCode::Char('y')));
        wait_for_jobs(&mut app);
        let Screen::Import(state) = &app.screen else {
            panic!("import result");
        };
        assert_eq!(state.phase, ImportPhase::Result);
        assert!(app
            .snapshot
            .aliases
            .iter()
            .any(|alias| alias.name == "imported"));
        assert!(app.status.text.contains("single transaction"));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.screen, Screen::Browse));

        // Cancel before preview.
        let cancel_file = root.join("cancel.json");
        aliasmgr_core::transfer::export_json(
            &cancel_file,
            &[AliasRecord {
                name: "not_imported".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        )
        .unwrap();
        app.handle_key(key(KeyCode::Char('I')));
        type_path(&mut app, &cancel_file);
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.screen, Screen::Browse));
        assert!(!app
            .database
            .list_aliases()
            .unwrap()
            .iter()
            .any(|alias| alias.name == "not_imported"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn import_missing_file_and_sensitive_values_are_reported_without_secrets() {
        let root = temp_root("import-errors");
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(key(KeyCode::Char('I')));
        type_path(&mut app, &root.join("missing.json"));
        app.handle_key(key(KeyCode::Enter));
        wait_for_jobs(&mut app);
        let Screen::Import(state) = &app.screen else {
            panic!("import screen");
        };
        assert_eq!(state.phase, ImportPhase::Input);
        assert!(state.error.as_deref().unwrap().contains("file not found"));
        app.handle_key(key(KeyCode::Esc));

        let file = root.join("sensitive.json");
        let mut alias = AliasRecord {
            name: "secret".into(),
            executable: "tool".into(),
            shells: vec![ShellKind::Bash],
            ..Default::default()
        };
        alias.environment.insert("TOKEN".into(), "hunter2".into());
        alias.environment.insert("MODE".into(), "fast".into());
        let envelope = aliasmgr_core::transfer::ExportFile {
            format_version: aliasmgr_core::transfer::FORMAT_VERSION,
            exported_at: "2026-01-01T00:00:00+00:00".into(),
            exported_by_version: "test".into(),
            aliases: vec![alias],
        };
        std::fs::write(&file, serde_json::to_string(&envelope).unwrap()).unwrap();
        app.handle_key(key(KeyCode::Char('I')));
        type_path(&mut app, &file);
        app.handle_key(key(KeyCode::Enter));
        wait_for_jobs(&mut app);
        let Screen::Import(state) = &app.screen else {
            panic!("import preview");
        };
        let report = state.report.as_ref().unwrap();
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("sensitive environment variable omitted")));
        assert!(!report.warnings.join("\n").contains("hunter2"));
        app.handle_key(key(KeyCode::Char('y')));
        wait_for_jobs(&mut app);
        let saved = AliasService::new(&app.database)
            .get_by_name("secret")
            .unwrap()
            .unwrap();
        assert!(!saved.environment.contains_key("TOKEN"));
        assert_eq!(
            saved.environment.get("MODE").map(String::as_str),
            Some("fast")
        );
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn import_export_settings_keys_are_text_inside_a_dirty_editor() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        assert!(matches!(app.screen, Screen::Editor(_)));
        let before = match &app.screen {
            Screen::Editor(editor) => editor.draft.name.clone(),
            _ => String::new(),
        };
        for character in "IXS".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        let Screen::Editor(editor) = &app.screen else {
            panic!("editor must stay open");
        };
        assert_eq!(editor.draft.name, format!("{before}IXS"));
        assert!(editor.is_dirty());
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn export_flow_writes_json_and_requires_overwrite_confirmation() {
        let (root, mut app) = app_with_two();
        let file = root.join("export.json");
        app.handle_key(key(KeyCode::Char('X')));
        assert!(matches!(app.screen, Screen::Export(_)));
        type_path(&mut app, &file);
        app.handle_key(key(KeyCode::Enter));
        wait_for_jobs(&mut app);
        let Screen::Export(state) = &app.screen else {
            panic!("export result");
        };
        assert_eq!(state.phase, ExportPhase::Result);
        assert_eq!(state.summary.as_ref().unwrap().count, 2);
        assert!(file.exists());
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.screen, Screen::Browse));

        std::fs::write(&file, "keep-me").unwrap();
        app.handle_key(key(KeyCode::Char('X')));
        type_path(&mut app, &file);
        app.handle_key(key(KeyCode::Enter));
        let Screen::Export(state) = &app.screen else {
            panic!("export confirm");
        };
        assert_eq!(state.phase, ExportPhase::ConfirmOverwrite);
        app.handle_key(key(KeyCode::Esc));
        let Screen::Export(state) = &app.screen else {
            panic!("export input");
        };
        assert_eq!(state.phase, ExportPhase::Input);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep-me");
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn settings_save_is_shared_and_invalid_counts_keep_the_draft() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('S')));
        let Screen::Settings(state) = &app.screen else {
            panic!("settings");
        };
        assert_eq!(state.config_dir, root);
        if let Screen::Settings(state) = &mut app.screen {
            state.draft.allow_relative_paths = true;
            state.draft.language = "en".into();
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        let loaded = aliasmgr_core::config::AppPaths { root: root.clone() }
            .load_config()
            .unwrap();
        assert!(loaded.runtime.allow_relative_paths);
        assert_eq!(loaded.runtime.language, "en");
        assert!(app.status.text.contains("settings saved"));
        assert!(matches!(app.screen, Screen::Settings(_)));

        let before = std::fs::read_to_string(root.join("config.toml")).unwrap();
        if let Screen::Settings(state) = &mut app.screen {
            state.draft.rc_keep = "not-a-number".into();
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(app.status.text.contains("must be a non-negative integer"));
        assert_eq!(
            std::fs::read_to_string(root.join("config.toml")).unwrap(),
            before
        );
        let Screen::Settings(state) = &app.screen else {
            panic!("settings stays open");
        };
        assert_eq!(state.draft.rc_keep, "not-a-number");
        assert!(state.error.is_some());
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_config_file_reports_an_error_instead_of_opening_settings() {
        let (root, mut app) = app_with_two();
        std::fs::write(root.join("config.toml"), "not = [valid").unwrap();
        app.handle_key(key(KeyCode::Char('S')));
        assert!(matches!(app.screen, Screen::Browse));
        assert!(app.status.text.contains("could not load config.toml"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn settings_cancel_dirty_and_save_failure_keep_the_draft() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('S')));
        if let Screen::Settings(state) = &mut app.screen {
            state.draft.language = "en".into();
        }
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.modal, Some(Modal::ConfirmDiscard)));
        app.handle_key(key(KeyCode::Char('n')));
        assert!(matches!(app.screen, Screen::Settings(_)));

        if let Screen::Settings(state) = &mut app.screen {
            state.draft.language = "en".into();
        }
        // Force a deterministic write failure by making the target a directory.
        let config_file = root.join("config.toml");
        if config_file.exists() {
            std::fs::remove_file(&config_file).unwrap();
        }
        std::fs::create_dir(&config_file).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(app.status.text.contains("settings were not saved"));
        let Screen::Settings(state) = &app.screen else {
            panic!("settings stays open");
        };
        assert_eq!(state.draft.language, "en");
        assert!(state.error.is_some());
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn esc_from_detail_focus_clears_applied_filters_in_one_press() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('/')));
        for character in "gs".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.visible.len(), 1);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Detail);
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.focus, Focus::List);
        assert!(app.filter.input.is_empty());
        assert_eq!(app.visible.len(), 2);
        assert!(app.status.text.contains("filters cleared"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn navigation_clamps_and_scrolls_without_panicking_on_empty_lists() {
        let root = temp_root("nav");
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::End));
        assert_eq!(app.selected, 0);
        let alias = seed(&app, "gs", "git");
        app.reload();
        assert_eq!(app.selected_alias().unwrap().id, alias.id);
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected, 0);
        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.selected, 0);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resize_marks_dirty_and_keeps_state() {
        let (root, mut app) = app_with_two();
        app.dirty = false;
        app.handle_key(key(KeyCode::Char('j')));
        let selected = app.selected;
        app.on_resize();
        assert!(app.dirty);
        assert_eq!(app.selected, selected);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn search_input_consumes_shortcuts_and_esc_restores_the_list() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('/')));
        assert!(app.filter.active);
        for character in "gs".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        // `g` and `s` were typed, not executed as shortcuts.
        assert_eq!(app.filter.input.value(), "gs");
        assert_eq!(app.visible.len(), 1);
        assert_eq!(app.selected_alias().unwrap().name, "gs");
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Enter));
        assert!(!app.filter.active);
        assert_eq!(app.visible.len(), 1);
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.filter.input.value(), "");
        assert_eq!(app.visible.len(), 2);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn q_quits_only_outside_text_input() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        assert!(matches!(app.screen, Screen::Editor(_)));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.should_quit);
        let Screen::Editor(editor) = &app.screen else {
            panic!("editor");
        };
        assert!(editor.input.value().contains('q'));
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(app.screen, Screen::Browse));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ctrl_c_asks_before_discarding_an_unsaved_draft() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(key(KeyCode::Char('!')));
        assert!(matches!(app.screen, Screen::Editor(_)));
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(app.modal, Some(Modal::ConfirmQuit)));
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Char('n')));
        assert!(!app.should_quit);
        assert!(matches!(app.screen, Screen::Editor(_)));
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        app.handle_key(key(KeyCode::Char('y')));
        assert!(app.should_quit);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dirty_editor_cancel_and_quit_require_confirmation() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.modal, Some(Modal::ConfirmDiscard)));
        app.handle_key(key(KeyCode::Char('n')));
        assert!(matches!(app.screen, Screen::Editor(_)));
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(app.screen, Screen::Browse));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn create_and_save_reports_sync_required_and_bumps_revision_once() {
        let root = temp_root("create");
        let mut app = App::new(root.clone()).unwrap();
        app.handle_key(key(KeyCode::Char('n')));
        let Screen::Editor(editor) = &mut app.screen else {
            panic!("editor");
        };
        editor.draft.name = "gs".into();
        editor.draft.executable = "git".into();
        editor.draft.fixed_args = vec!["status".into()];
        app.save_editor();
        assert!(matches!(app.screen, Screen::Browse));
        assert!(app.status.text.contains("press s to sync"));
        let saved = app.selected_alias_cloned().unwrap();
        assert_eq!(saved.name, "gs");
        assert_eq!(saved.revision, 1);
        // A second save call with no editor is a no-op (no duplicate submit).
        app.save_editor();
        assert_eq!(
            AliasService::new(&app.database)
                .get(saved.id)
                .unwrap()
                .unwrap()
                .revision,
            1
        );
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn double_submit_guard_refuses_a_second_save_while_saving() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.saving = true;
            editor.draft.description = "changed".into();
        }
        app.save_editor();
        assert!(app.status.text.contains("save already in progress"));
        if let Screen::Editor(editor) = &app.screen {
            assert!(editor.saving);
        }
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn external_modification_causes_a_conflict_and_is_never_overwritten_silently() {
        let (root, mut app) = app_with_two();
        let original = app
            .snapshot
            .aliases
            .iter()
            .find(|alias| alias.name == "gs")
            .cloned()
            .unwrap();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.draft.description = "local edit".into();
        }
        // Another front end writes the same record.
        let other = Database::open(root.join("aliases.db")).unwrap();
        let mut external = original.clone();
        external.description = "external edit".into();
        AliasService::new(&other).update(external).unwrap();
        app.save_editor();
        assert!(matches!(app.modal, Some(Modal::Conflict { .. })));
        let stored = AliasService::new(&other).get(original.id).unwrap().unwrap();
        assert_eq!(stored.description, "external edit");
        assert_eq!(stored.revision, original.revision + 1);
        // Ignoring the conflict leaves the record untouched.
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert_eq!(
            AliasService::new(&other)
                .get(original.id)
                .unwrap()
                .unwrap()
                .description,
            "external edit"
        );
        drop(other);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn conflict_overwrite_requires_an_explicit_choice_and_then_saves() {
        let (root, mut app) = app_with_two();
        let original = app
            .snapshot
            .aliases
            .iter()
            .find(|alias| alias.name == "gs")
            .cloned()
            .unwrap();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.draft.description = "local edit".into();
        }
        let other = Database::open(root.join("aliases.db")).unwrap();
        let mut external = original.clone();
        external.description = "external edit".into();
        AliasService::new(&other).update(external).unwrap();
        app.save_editor();
        assert!(matches!(app.modal, Some(Modal::Conflict { .. })));
        app.handle_key(key(KeyCode::Char('o')));
        assert!(matches!(app.screen, Screen::Browse));
        let stored = AliasService::new(&other).get(original.id).unwrap().unwrap();
        assert_eq!(stored.description, "local edit");
        assert_eq!(stored.revision, original.revision + 2);
        drop(other);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn conflict_reload_discards_the_draft_and_loads_the_latest_revision() {
        let (root, mut app) = app_with_two();
        let original = app
            .snapshot
            .aliases
            .iter()
            .find(|alias| alias.name == "gs")
            .cloned()
            .unwrap();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.draft.description = "local edit".into();
        }
        let other = Database::open(root.join("aliases.db")).unwrap();
        let mut external = original.clone();
        external.description = "external edit".into();
        AliasService::new(&other).update(external).unwrap();
        app.save_editor();
        app.handle_key(key(KeyCode::Char('r')));
        let Screen::Editor(editor) = &app.screen else {
            panic!("editor");
        };
        assert_eq!(editor.draft.description, "external edit");
        assert_eq!(editor.base_revision, original.revision + 1);
        assert!(!editor.is_dirty());
        drop(other);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn delete_requires_confirmation_and_retires_the_name() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('d')));
        assert!(matches!(app.modal, Some(Modal::ConfirmDelete { .. })));
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.snapshot.aliases.len(), 2);
        app.handle_key(key(KeyCode::Char('d')));
        app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(app.snapshot.aliases.len(), 1);
        assert!(app.status.text.contains("press s to sync"));
        let names = app.database.managed_name_set(&ShellKind::Bash).unwrap();
        assert_eq!(names.retired, vec!["gs".to_string()]);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rename_and_enable_disable_reuse_service_semantics() {
        let (root, mut app) = app_with_two();
        // Selected alias sorts first: gs.
        app.handle_key(key(KeyCode::Char('r')));
        let Some(Modal::Rename {
            alias, mut input, ..
        }) = app.modal.take()
        else {
            panic!("rename modal");
        };
        input.set_value("git-status");
        app.modal = Some(Modal::Rename {
            alias,
            input,
            error: None,
        });
        app.handle_key(key(KeyCode::Enter));
        assert!(app.snapshot.aliases.iter().any(|a| a.name == "git-status"));
        app.handle_key(key(KeyCode::Char(' ')));
        let alias = app
            .snapshot
            .aliases
            .iter()
            .find(|a| a.name == "git-status")
            .unwrap();
        assert!(!alias.enabled);
        app.handle_key(key(KeyCode::Char(' ')));
        let alias = app
            .snapshot
            .aliases
            .iter()
            .find(|a| a.name == "git-status")
            .unwrap();
        assert!(alias.enabled);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn editor_rename_retires_the_old_name() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.set_field(Field::Name);
            editor.input.set_value("git-status");
            editor.draft.name = "git-status".into();
        }
        app.save_editor();
        assert!(app.snapshot.aliases.iter().any(|a| a.name == "git-status"));
        let names = app.database.managed_name_set(&ShellKind::Bash).unwrap();
        assert_eq!(names.retired, vec!["gs".to_string()]);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn tag_filter_modal_applies_and_clears() {
        let (root, mut app) = app_with_two();
        let alias = app
            .snapshot
            .aliases
            .iter()
            .find(|alias| alias.name == "gs")
            .cloned()
            .unwrap();
        let mut tagged = alias.clone();
        tagged.tags = vec!["git".into()];
        AliasService::new(&app.database).update(tagged).unwrap();
        app.reload();
        app.handle_key(key(KeyCode::Char('t')));
        assert!(matches!(app.modal, Some(Modal::TagFilter { .. })));
        app.handle_key(key(KeyCode::Char(' ')));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.filter.tags, vec!["git".to_string()]);
        assert_eq!(app.visible.len(), 1);
        app.handle_key(key(KeyCode::Char('t')));
        app.handle_key(key(KeyCode::Char('c')));
        app.handle_key(key(KeyCode::Enter));
        assert!(app.filter.tags.is_empty());
        assert_eq!(app.visible.len(), 2);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sort_cycle_and_fuzzy_toggle_recompute_the_list() {
        let (root, mut app) = app_with_two();
        app.handle_key(key(KeyCode::Char('o')));
        assert_eq!(app.filter.sort, SortField::UpdatedAt);
        app.handle_key(key(KeyCode::Char('O')));
        assert_eq!(app.filter.sort, SortField::UpdatedAt);
        assert!(app.filter.descending);
        assert!(app.status.text.contains("descending"));
        app.handle_key(key(KeyCode::Char('F')));
        assert!(app.filter.fuzzy);
        assert!(app.status.text.contains("fuzzy"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preview_switches_shells_and_does_not_leak_secret_values() {
        let root = temp_root("preview-app");
        let mut app = App::new(root.clone()).unwrap();
        let mut alias = AliasRecord {
            name: "secret".into(),
            executable: "git".into(),
            shells: vec![ShellKind::Bash, ShellKind::Zsh],
            ..Default::default()
        };
        alias.environment.insert("TOKEN".into(), "hunter2".into());
        let saved = AliasService::new(&app.database).create(alias).unwrap();
        app.reload();
        app.handle_key(key(KeyCode::Char('p')));
        let Screen::Preview(state) = &app.screen else {
            panic!("preview");
        };
        assert!(state.body.contains("secret"));
        assert!(state.body.contains("***"));
        assert!(!state.body.contains("hunter2"));
        assert_eq!(state.shells.len(), 2);
        app.handle_key(key(KeyCode::Tab));
        let Screen::Preview(state) = &app.screen else {
            panic!("preview");
        };
        assert_eq!(state.index, 1);
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.screen, Screen::Browse));
        let _ = saved;
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn external_change_poll_refreshes_without_discarding_an_editor_draft() {
        let (root, mut app) = app_with_two();
        let original = app
            .snapshot
            .aliases
            .iter()
            .find(|alias| alias.name == "gs")
            .cloned()
            .unwrap();
        app.handle_key(key(KeyCode::Char('e')));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.draft.description = "local".into();
        }
        let other = Database::open(root.join("aliases.db")).unwrap();
        let mut external = original.clone();
        external.description = "external".into();
        AliasService::new(&other).update(external).unwrap();
        app.last_external_check = Instant::now() - Duration::from_secs(5);
        app.poll_external_changes();
        assert!(app.external_pending);
        assert!(app.status.text.contains("saving will ask"));
        app.reload();
        assert!(!app.external_pending);
        let Screen::Editor(editor) = &app.screen else {
            panic!("editor");
        };
        assert_eq!(editor.draft.description, "local");
        assert!(editor.is_dirty());
        drop(other);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_and_doctor_jobs_update_the_ui_without_blocking_key_handling() {
        let root = temp_root("jobs");
        let mut app = App::new(root.clone()).unwrap();
        seed(&app, "gs", "git");
        app.reload();
        app.start_sync();
        assert!(app.jobs.is_busy());
        // Keys still work while the job runs.
        app.handle_key(key(KeyCode::Char('j')));
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            app.poll_jobs();
            if !app.jobs.is_busy() {
                break;
            }
            assert!(Instant::now() < deadline, "sync job did not finish");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(matches!(app.screen, Screen::SyncReport(_)));
        assert!(app.status.text.contains("reload"));
        assert!(root.join("generated/bash.sh").exists());
        app.handle_key(key(KeyCode::Esc));
        app.start_doctor();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            app.poll_jobs();
            if !app.jobs.is_busy() {
                break;
            }
            assert!(Instant::now() < deadline, "doctor job did not finish");
            std::thread::sleep(Duration::from_millis(20));
        }
        let Screen::Doctor(state) = &app.screen else {
            panic!("doctor");
        };
        assert!(!state.busy);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn filter_summary_and_help_cover_semantics() {
        let filter = FilterState {
            input: TextInput::new("git"),
            tags: vec!["work".into()],
            sort: SortField::UpdatedAt,
            descending: true,
            fuzzy: true,
            ..Default::default()
        };
        let summary = filter_summary(&filter);
        assert!(summary.contains("search: git"));
        assert!(summary.contains("tags: work"));
        assert!(summary.contains("sort: updated_at desc"));
        let help = help_lines().join("\n");
        assert!(help.contains("python_script"));
        assert!(help.contains("classification only"));
        assert!(help.contains("current shell unknown"));
        assert!(help.contains("never runs source"));
    }

    #[test]
    fn help_uses_visible_fields_consistently() {
        assert_eq!(visible_fields(false), Field::BASIC.to_vec());
        assert_eq!(
            visible_fields(true).len(),
            Field::BASIC.len() + Field::ADVANCED.len()
        );
    }
}
