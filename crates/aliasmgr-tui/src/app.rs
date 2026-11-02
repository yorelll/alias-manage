//! Application state and the keybinding state machine.
//!
//! All persistence goes through `AliasService`/`Database`; the TUI never writes
//! files, profiles, or the database directly except through the shared core.

use crate::{
    data::{self, JobKind, JobResult, Jobs, Snapshot},
    input::{is_actionable, is_ctrl, TextInput},
    model::{DraftError, Editor, EditorOutcome, Field},
    text,
    theme::Theme,
};
use aliasmgr_core::{
    config::AppPaths,
    doctor::DoctorFinding,
    error::AliasError,
    lock::FileLock,
    model::{AliasRecord, ShellKind},
    search::{search, SearchField, SearchQuery, SortField},
    service::AliasService,
    storage::Database,
    sync::{SyncCoordinator, SyncRun},
};
use crossterm::event::{KeyCode, KeyEvent};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewState {
    pub alias_name: String,
    pub shells: Vec<ShellKind>,
    pub index: usize,
    pub body: String,
    pub error: Option<String>,
    pub scroll: u16,
    pub redacted: bool,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    ConfirmDelete {
        alias: AliasRecord,
    },
    ConfirmQuit,
    ConfirmDiscard,
    Rename {
        input: TextInput,
    },
    TagFilter {
        entries: Vec<(String, bool)>,
        cursor: usize,
    },
    Conflict {
        current: AliasRecord,
    },
    Message {
        title: String,
        lines: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Browse,
    Editor(Box<Editor>),
    Preview(PreviewState),
    Doctor(DoctorState),
    Help(ScrollState),
    SyncReport(SyncReportState),
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
    last_external_check: Instant,
}

impl App {
    pub fn new(root: PathBuf) -> Result<Self, AliasError> {
        AppPaths { root: root.clone() }.ensure_directories()?;
        // A sync interrupted by a crash leaves a journal. Recover it through
        // the shared core helper under the same lock sync uses; never write a
        // second recovery implementation here.
        let journal = root.join("operation.journal");
        // A successful sync leaves a `state=completed` journal behind; only
        // prepared/committed entries represent work that needs recovery.
        let journal_pending = std::fs::read_to_string(&journal)
            .map(|content| {
                content
                    .lines()
                    .any(|line| line == "state=prepared" || line == "state=committed")
            })
            .unwrap_or(false);
        let recovery = if journal_pending {
            match FileLock::acquire(root.join("sync.lock"), Duration::from_secs(2)) {
                Ok(_lock) => Some(
                    SyncCoordinator::new(root.clone())
                        .recover_pending_operations()
                        .map_err(|error| error.to_string()),
                ),
                Err(error) => Some(Err(format!("sync lock unavailable: {error}"))),
            }
        } else {
            None
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

    pub fn reload(&mut self) {
        let selected_id = self.selected_alias().map(|alias| alias.id);
        match data::load_snapshot(&self.database, &self.root) {
            Ok(snapshot) => {
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
                self.dirty = true;
            }
            Err(error) => {
                self.set_status(StatusKind::Error, format!("refresh failed: {error}"));
            }
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
            Screen::Help(_) | Screen::SyncReport(_) => self.handle_scroll_key(key),
        }
    }

    fn request_quit(&mut self) {
        let dirty_editor = match &self.screen {
            Screen::Editor(editor) => editor.is_dirty(),
            _ => false,
        };
        if dirty_editor {
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
                    self.filter.input.clear();
                    self.recompute_visible();
                    self.dirty = true;
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
                    self.dirty = true;
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
            _ => {}
        }
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
        self.modal = Some(Modal::Rename {
            input: TextInput::new(alias.name),
        });
        self.dirty = true;
    }

    fn open_delete(&mut self) {
        let Some(alias) = self.selected_alias_cloned() else {
            self.set_status(StatusKind::Warning, "no alias selected");
            return;
        };
        self.modal = Some(Modal::ConfirmDelete { alias });
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

    fn open_preview(&mut self) {
        let Some(alias) = self.selected_alias_cloned() else {
            self.set_status(StatusKind::Warning, "no alias selected");
            return;
        };
        let shells = alias.shells.clone();
        let mut state = PreviewState {
            alias_name: alias.name.clone(),
            shells,
            index: 0,
            body: String::new(),
            error: None,
            scroll: 0,
            redacted: true,
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
        let draft = None;
        let aliases = data::with_draft(&self.snapshot.aliases, draft);
        match data::preview_shell(&self.root, &self.snapshot, &shell, &aliases) {
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
                    state.scroll = state.scroll.saturating_add(10);
                }
                KeyCode::PageUp => {
                    state.scroll = state.scroll.saturating_sub(10);
                }
                KeyCode::Home | KeyCode::Char('g') => state.scroll = 0,
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
                KeyCode::Char('y') | KeyCode::Char('Y') => self.perform_delete(alias),
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
            Modal::Rename { mut input } => match key.code {
                KeyCode::Enter => self.perform_rename(input.value().to_string()),
                KeyCode::Esc => {
                    self.set_status(StatusKind::Info, "rename cancelled");
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    self.modal = Some(Modal::Rename { input });
                }
                _ => {
                    input.handle_key(key);
                    self.modal = Some(Modal::Rename { input });
                }
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
            Modal::Conflict { current } => match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if let Screen::Editor(editor) = &mut self.screen {
                        editor.reload_from(&current);
                    }
                    self.set_status(
                        StatusKind::Info,
                        "loaded the latest revision; local changes were discarded",
                    );
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.screen = Screen::Browse;
                    self.set_status(StatusKind::Info, "local changes discarded");
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
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
                        Some((record, editor.is_new, editor.base_revision))
                    }
                    Err(error) => {
                        editor.error = Some(error);
                        None
                    }
                }
            }
            _ => None,
        };
        let Some((record, is_new, base_revision)) = prepared else {
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
                    self.modal = Some(Modal::Conflict { current });
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

    fn perform_rename(&mut self, new_name: String) {
        if self.writes_blocked() {
            return;
        }
        let Some(alias) = self.selected_alias_cloned() else {
            self.set_status(StatusKind::Warning, "no alias selected");
            return;
        };
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
        match outcome.result {
            JobResult::Sync(Ok(run)) => {
                let failed = run.failed();
                let count = run.results.len();
                self.reload();
                self.screen = Screen::SyncReport(SyncReportState { run, scroll: 0 });
                if failed {
                    self.set_status(
                        StatusKind::Warning,
                        "sync finished with failures; generated files may be stale; reload is still manual",
                    );
                } else {
                    self.set_status(
                        StatusKind::Success,
                        format!(
                            "synced {count} shell(s); run the shown reload command in this terminal"
                        ),
                    );
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
        "  j/k PgUp/PgDn scroll    Tab/[ ] switch preview shell".to_string(),
        "  Doctor: s sync, r rescan    Esc back".to_string(),
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
        "  Terminal smaller than 80x20 shows a clear message; resize to continue.".to_string(),
        "  No mouse, clipboard, Nerd Font or emoji dependency; ASCII fallback via ALIASMGR_TUI_ASCII=1.".to_string(),
        "  Ctrl+C quits (asks before discarding an unsaved editor draft).".to_string(),
    ]);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let Some(Modal::Rename { mut input }) = app.modal.take() else {
            panic!("rename modal");
        };
        input.set_value("git-status");
        app.modal = Some(Modal::Rename { input });
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
