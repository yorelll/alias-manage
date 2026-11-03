//! Interactive Shell integration screen for `aliasmgr doctor`.
//!
//! This is a CLI diagnostic screen that happens to live in the TUI crate so a
//! `--no-default-features` CLI does not link crossterm. It uses the shared
//! `shell_setup` API for detection, planning and apply; no loader or sync
//! logic is duplicated here.

use crate::terminal::{self, TerminalSession};
use aliasmgr_core::{
    config::AppPaths,
    detection::{self, ShellInstallation},
    error::AliasError,
    model::{ShellKind, SUPPORTED_SHELLS},
    shell_setup::{self, LoaderPreview, SetupAction, ShellSetupOptions, ShellSetupPlan},
    sync,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::IsTerminal,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};

/// Why the setup screen was opened. The TUI keeps the same state machine for
/// first-run, startup repair, the settings panel and the add/edit preflight so
/// loader/marker/sync policy is never reimplemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SetupOrigin {
    /// Interactive `aliasmgr doctor`; `q` exits without writing anything.
    Doctor,
    /// Brand-new configuration; `q`/skip persists an empty `shells.enabled`.
    FirstRun,
    /// Repair an enabled shell whose loader went missing.
    Repair,
    /// Blocked add/edit save; the caller restores the draft afterwards.
    Preflight,
    /// Shell integration panel opened from the settings screen.
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathFocus {
    Executable,
    Profile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviewEntry {
    kind: ShellKind,
    preview: LoaderPreview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    List,
    PathInput {
        kind: ShellKind,
        executable: String,
        profile: String,
        focus: PathFocus,
    },
    Confirm {
        previews: Vec<PreviewEntry>,
        scroll: u16,
    },
    NeedsPathChoice {
        kind: ShellKind,
    },
    Result,
}

#[derive(Debug)]
pub struct SetupApp {
    paths: AppPaths,
    config: aliasmgr_core::config::AppConfig,
    rows: Vec<ShellInstallation>,
    plans: Vec<Option<ShellSetupPlan>>,
    checked: BTreeSet<ShellKind>,
    selected: usize,
    mode: Mode,
    detecting: bool,
    receiver: Option<Receiver<ShellInstallation>>,
    pending_overrides: BTreeMap<ShellKind, (Option<PathBuf>, Option<PathBuf>)>,
    status: String,
    summary: Vec<String>,
    quit: bool,
    dirty: bool,
    origin: SetupOrigin,
}

impl SetupApp {
    fn new(paths: AppPaths) -> Result<Self, AliasError> {
        Self::new_with_origin(paths, SetupOrigin::Doctor, &[])
    }

    /// Builds the shared setup state and starts detection in the background.
    /// `checked` preselects shells for repair/preflight flows; first-run and
    /// settings still default to persisted intent or an already valid loader.
    pub(crate) fn new_with_origin(
        paths: AppPaths,
        origin: SetupOrigin,
        checked: &[ShellKind],
    ) -> Result<Self, AliasError> {
        let config = paths.load_config()?;
        let mut app = Self {
            paths,
            config,
            rows: Vec::new(),
            plans: Vec::new(),
            checked: checked.iter().cloned().collect(),
            selected: 0,
            mode: Mode::List,
            detecting: false,
            receiver: None,
            pending_overrides: BTreeMap::new(),
            status: "Detecting shells... version probes may take a moment.".into(),
            summary: Vec::new(),
            quit: false,
            dirty: true,
            origin,
        };
        app.start_detection(false);
        Ok(app)
    }

    pub(crate) fn origin(&self) -> SetupOrigin {
        self.origin
    }

    pub(crate) fn is_quit(&self) -> bool {
        self.quit
    }

    /// True once a confirmed apply produced at least one result line.
    pub(crate) fn has_changes(&self) -> bool {
        !self.summary.is_empty()
    }

    pub(crate) fn summary_text(&self) -> String {
        self.summary.join("\n")
    }

    /// Current installed rows and plans, used by the TUI to refresh shell
    /// selector labels after the panel closes.
    pub(crate) fn installations(&self) -> &[ShellInstallation] {
        &self.rows
    }

    pub(crate) fn setup_plans(&self) -> &[Option<ShellSetupPlan>] {
        &self.plans
    }

    /// Deterministic row injection for state/render tests: cancels the probe
    /// thread and rebuilds plans exactly as if detection had finished.
    #[cfg(test)]
    pub(crate) fn inject_rows_for_test(&mut self, rows: Vec<ShellInstallation>) {
        self.receiver = None;
        self.detecting = false;
        self.rows = rows;
        self.rebuild_plans();
        self.dirty = true;
    }

    fn start_detection(&mut self, reset_checked: bool) {
        if reset_checked {
            self.checked.clear();
        }
        self.detecting = true;
        self.rows.clear();
        self.plans.clear();
        self.pending_overrides.clear();
        self.status = "Detecting shells... version probes may take a moment.".into();
        let config = self.config.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            // Send each row as soon as its version probe finishes, so slow
            // probes show per-item progress instead of one long block.
            for kind in SUPPORTED_SHELLS {
                let row = detection::detect_shell(&config, &kind);
                if sender.send(row).is_err() {
                    break;
                }
            }
        });
        self.receiver = Some(receiver);
        self.dirty = true;
    }

    pub(crate) fn poll_detection(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let mut changed = false;
        loop {
            match receiver.try_recv() {
                Ok(row) => {
                    self.rows.push(row);
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.detecting = false;
                    changed = true;
                    break;
                }
            }
        }
        if changed {
            self.rebuild_plans();
            if self.detecting {
                self.status = format!(
                    "Detected {}/4 shells; version probes may still be running.",
                    self.rows.len()
                );
            } else if self.rows.len() == SUPPORTED_SHELLS.len() {
                self.status = "Space selects, Enter previews, r rescans, q quits.".into();
            } else {
                self.status = format!(
                    "Detection stopped after {} shells; press r to rescan.",
                    self.rows.len()
                );
            }
            self.dirty = true;
        }
    }

    fn rebuild_plans(&mut self) {
        self.plans = self
            .rows
            .iter()
            .map(|row| {
                let (executable, profile) = self
                    .pending_overrides
                    .get(&row.kind)
                    .cloned()
                    .unwrap_or((None, None));
                if executable.is_none() && profile.is_none() {
                    shell_setup::plan_shell_setup_with_installation(
                        &self.config,
                        &self.paths,
                        &row.kind,
                        row,
                        None,
                        None,
                    )
                    .ok()
                } else {
                    // Overrides (including the clear sentinel) change detection
                    // itself, so re-plan instead of reusing the probed row.
                    shell_setup::plan_shell_setup(
                        &self.config,
                        &self.paths,
                        &row.kind,
                        executable.as_deref(),
                        profile.as_deref(),
                    )
                    .ok()
                }
            })
            .collect();
        let enabled = shell_setup::enabled_shells(&self.config);
        let previously_checked = self.checked.clone();
        self.checked = self
            .rows
            .iter()
            .zip(self.plans.iter())
            .filter(|(row, plan)| {
                if row.wsl {
                    return false;
                }
                previously_checked.contains(&row.kind)
                    || enabled.contains(&row.kind)
                    || plan
                        .as_ref()
                        .map(|plan| plan.loader_status.is_installed())
                        .unwrap_or(false)
            })
            .map(|(row, _)| row.kind.clone())
            .collect();
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
    }

    fn current_row(&self) -> Option<&ShellInstallation> {
        self.rows.get(self.selected)
    }

    fn current_plan(&self) -> Option<&ShellSetupPlan> {
        self.plans.get(self.selected).and_then(|plan| plan.as_ref())
    }

    fn plan_for(&self, kind: &ShellKind) -> Option<&ShellSetupPlan> {
        self.rows
            .iter()
            .position(|row| &row.kind == kind)
            .and_then(|index| self.plans.get(index).and_then(|plan| plan.as_ref()))
    }

    fn apply_kinds(&self) -> Vec<ShellKind> {
        if self.checked.is_empty() {
            self.current_row()
                .map(|row| vec![row.kind.clone()])
                .unwrap_or_default()
        } else {
            self.checked.iter().cloned().collect()
        }
    }

    fn begin_path_input(&mut self) {
        let Some(row) = self.current_row() else {
            return;
        };
        let kind = row.kind.clone();
        let is_wsl = row.wsl;
        let executable = if is_wsl {
            // Never offer the WSL launcher as the integration executable.
            String::new()
        } else {
            self.current_plan()
                .and_then(|plan| plan.executable.clone())
                .filter(|path| path.is_file())
                .map(|path| path.display().to_string())
                .unwrap_or_default()
        };
        let profile = self
            .current_plan()
            .and_then(|plan| plan.loader_target.clone())
            .or_else(|| shell_setup::default_profile_path(&self.config, &self.paths, &kind))
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        self.mode = Mode::PathInput {
            kind,
            executable,
            profile,
            focus: PathFocus::Executable,
        };
        self.status = if is_wsl {
            "WSL launcher detected. Enter a native Windows executable (for example Git Bash) and an absolute profile/RC path, or Esc and run doctor inside the distribution.".into()
        } else {
            "Enter an absolute executable path, then a profile/RC path. Leading ~/ expands. Esc cancels.".into()
        };
        self.dirty = true;
    }

    fn confirm_ready_row(&mut self) {
        let kinds = self.apply_kinds();
        self.build_confirm(kinds);
    }

    /// Builds a preview for every shell that will be applied. Nothing is
    /// written until the user confirms this exact set; a shell that cannot be
    /// previewed blocks the action instead of being silently applied.
    fn build_confirm(&mut self, kinds: Vec<ShellKind>) {
        let mut previews = Vec::new();
        let mut seen = Vec::new();
        for kind in kinds {
            if seen.contains(&kind) {
                continue;
            }
            seen.push(kind.clone());
            let Some(plan) = self.plan_for(&kind) else {
                self.status = format!("{}: no setup plan", kind.label());
                self.mode = Mode::List;
                self.dirty = true;
                return;
            };
            match plan.action {
                SetupAction::WslManual if !self.pending_overrides.contains_key(&kind) => {
                    self.status = format!(
                        "{} is the WSL launcher; press Enter to enter a native Windows executable, or run doctor inside WSL.",
                        kind.label()
                    );
                    self.mode = Mode::List;
                    self.dirty = true;
                    return;
                }
                SetupAction::NeedsPath if !plan.detected => {
                    self.status = format!(
                        "{} still needs an executable that exists; press Enter on it and supply the paths.",
                        kind.label()
                    );
                    self.mode = Mode::List;
                    self.dirty = true;
                    return;
                }
                SetupAction::NeedsPath if !self.pending_overrides.contains_key(&kind) => {
                    self.status = format!(
                        "{} still needs an executable and profile path; press Enter on it first.",
                        kind.label()
                    );
                    self.mode = Mode::List;
                    self.dirty = true;
                    return;
                }
                SetupAction::WslManual
                | SetupAction::NeedsPath
                | SetupAction::Enable
                | SetupAction::InstallLoader => {}
            }
            let (executable, profile) = self
                .pending_overrides
                .get(&kind)
                .cloned()
                .unwrap_or((None, None));
            let preview = match shell_setup::preview_loader(
                &self.config,
                &self.paths,
                &kind,
                executable.as_deref(),
                profile.as_deref(),
            ) {
                Ok(Some(preview)) => preview,
                Ok(None) => {
                    self.status = format!(
                        "{} has no usable profile target; enter a path first.",
                        kind.label()
                    );
                    self.mode = Mode::List;
                    self.dirty = true;
                    return;
                }
                Err(error) => {
                    self.status = format!("{}: {error}", kind.label());
                    self.mode = Mode::List;
                    self.dirty = true;
                    return;
                }
            };
            previews.push(PreviewEntry {
                kind: kind.clone(),
                preview,
            });
        }
        if previews.is_empty() {
            self.status = "Select at least one shell first.".into();
            self.mode = Mode::List;
            self.dirty = true;
            return;
        }
        self.mode = Mode::Confirm {
            previews,
            scroll: 0,
        };
        self.status = "Review every target and block. y applies exactly these, n cancels.".into();
        self.dirty = true;
    }

    fn finish_path_input(&mut self) {
        let Mode::PathInput {
            kind,
            executable,
            profile,
            ..
        } = self.mode.clone()
        else {
            return;
        };
        let executable_path = aliasmgr_core::config::expand_home_path(Path::new(executable.trim()));
        if !executable_path.is_absolute() {
            self.status = format!(
                "Executable path must be absolute; use ~/... to expand the home directory (got {}).",
                executable_path.display()
            );
            self.dirty = true;
            return;
        }
        if !executable_path.is_file() {
            self.status = format!("Executable not found: {}", executable_path.display());
            self.dirty = true;
            return;
        }
        let profile_path = profile.trim();
        let profile_override = if profile_path.is_empty() {
            None
        } else {
            let path = aliasmgr_core::config::expand_home_path(Path::new(profile_path));
            if !path.is_absolute() {
                self.status = format!(
                    "Profile/RC path must be absolute; use ~/... to expand the home directory (got {}).",
                    path.display()
                );
                self.dirty = true;
                return;
            }
            Some(path)
        };
        let plan = match shell_setup::plan_shell_setup(
            &self.config,
            &self.paths,
            &kind,
            Some(&executable_path),
            profile_override.as_deref(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                self.status = format!("{}: {error}", kind.label());
                self.dirty = true;
                return;
            }
        };
        if !plan.detected {
            self.status = format!("{} executable was not detected.", kind.label());
            self.dirty = true;
            return;
        }
        self.pending_overrides
            .insert(kind.clone(), (Some(executable_path), profile_override));
        self.checked.insert(kind);
        let kinds = self.apply_kinds();
        self.build_confirm(kinds);
    }

    fn apply_confirmed(&mut self) {
        let Mode::Confirm { previews, .. } = self.mode.clone() else {
            return;
        };
        let mut summary = Vec::new();
        // Apply exactly the previewed set: build_confirm refuses to produce a
        // confirmation screen that omits any shell about to be written.
        for entry in previews {
            let kind = entry.kind;
            let (executable, profile) = self
                .pending_overrides
                .get(&kind)
                .cloned()
                .unwrap_or((None, None));
            let options = ShellSetupOptions {
                executable,
                profile,
                sync: true,
            };
            match shell_setup::apply_shell_setup(&self.paths, &kind, &options) {
                Ok(outcome) => {
                    let mut line = format!("{}: enabled", kind.label());
                    if outcome.loader_changed {
                        line.push_str(", loader installed");
                    } else if outcome.loader_installed {
                        line.push_str(", loader already present");
                    }
                    if let Some(error) = &outcome.loader_error {
                        line.push_str(&format!(", loader error: {error}"));
                    }
                    if let Some(error) = &outcome.sync_error {
                        line.push_str(&format!(", sync error: {error}"));
                    } else if let Some(run) = &outcome.sync {
                        if run.failed() {
                            line.push_str(&format!(
                                ", sync failed: {}",
                                run.failure_summary().join("; ")
                            ));
                        } else {
                            line.push_str(", sync ok");
                        }
                    }
                    summary.push(line);
                    if outcome.loader_installed {
                        summary.push(format!(
                            "Reload: {}",
                            sync::reload_command(&self.paths.root, &kind)
                        ));
                    }
                }
                Err(error) => summary.push(format!("{}: {error}", kind.label())),
            }
        }
        self.pending_overrides.clear();
        self.summary = summary.clone();
        self.status = if summary.is_empty() {
            "No changes.".into()
        } else {
            summary.join("\n")
        };
        self.mode = Mode::Result;
        self.config = self
            .paths
            .load_config()
            .unwrap_or_else(|_| self.config.clone());
        self.dirty = true;
    }

    /// Settings panel action: persist `enabled=false` for the current shell.
    /// The loader block is left alone (removing it is the advanced
    /// `shell uninstall` operation) and the executable is recorded in
    /// `shells.seen` by core so startup does not immediately re-offer it.
    fn disable_current(&mut self) {
        let Some(row) = self.current_row() else {
            return;
        };
        if row.wsl {
            self.status = "WSL launcher rows cannot be disabled from Windows.".into();
            self.dirty = true;
            return;
        }
        let kind = row.kind.clone();
        match shell_setup::set_shell_enabled(&self.paths, &kind, false) {
            Ok(config) => {
                self.config = config;
                self.checked.remove(&kind);
                self.pending_overrides.remove(&kind);
                self.rebuild_plans();
                self.status = format!(
                    "{} disabled; its loader block is left in place until removed with the CLI shell uninstall command.",
                    kind.label()
                );
            }
            Err(error) => self.status = format!("{}: {error}", kind.label()),
        }
        self.dirty = true;
    }

    /// Settings panel action: preview platform defaults for executable and
    /// profile. Nothing is persisted until the previewed set is confirmed;
    /// core recognises the sentinel and removes the stored overrides.
    fn clear_overrides_current(&mut self) {
        let Some(row) = self.current_row() else {
            return;
        };
        let kind = row.kind.clone();
        self.pending_overrides.insert(
            kind.clone(),
            (
                Some(PathBuf::from(shell_setup::CLEAR_OVERRIDE)),
                Some(PathBuf::from(shell_setup::CLEAR_OVERRIDE)),
            ),
        );
        self.checked.insert(kind.clone());
        self.rebuild_plans();
        self.status = format!(
            "{}: executable/profile overrides will be cleared and platform defaults used. Enter previews; nothing is written yet.",
            kind.label()
        );
        self.dirty = true;
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C'))
        {
            self.quit = true;
            return;
        }
        match self.mode.clone() {
            Mode::List => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected = self.selected.saturating_sub(1);
                    self.dirty = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.rows.is_empty() {
                        self.selected = (self.selected + 1).min(self.rows.len() - 1);
                    }
                    self.dirty = true;
                }
                KeyCode::Char(' ') => {
                    if let Some(row) = self.current_row() {
                        let kind = row.kind.clone();
                        let wsl = row.wsl;
                        if wsl {
                            self.status =
                                "WSL launcher: press Enter to enter a native Windows executable, or run doctor inside the distribution."
                                    .into();
                        } else if !self.checked.remove(&kind) {
                            self.checked.insert(kind);
                        }
                        self.dirty = true;
                    }
                }
                KeyCode::Enter => {
                    if self.detecting {
                        self.status = "Still detecting; wait for the rows to appear.".into();
                        self.dirty = true;
                    } else if let Some(action) = self.current_plan().map(|plan| plan.action) {
                        match action {
                            SetupAction::NeedsPath => {
                                if let Some(row) = self.current_row() {
                                    self.mode = Mode::NeedsPathChoice {
                                        kind: row.kind.clone(),
                                    };
                                    self.status =
                                        "Choose 1 (install and restart), 2 (enter paths), or 3 (cancel)."
                                            .into();
                                    self.dirty = true;
                                }
                            }
                            SetupAction::WslManual => {
                                self.begin_path_input();
                            }
                            SetupAction::Enable | SetupAction::InstallLoader => {
                                self.confirm_ready_row()
                            }
                        }
                    }
                }
                KeyCode::Char('r') => self.start_detection(false),
                KeyCode::Char('e') if self.origin != SetupOrigin::Doctor => {
                    self.begin_path_input();
                }
                KeyCode::Char('c') if self.origin != SetupOrigin::Doctor => {
                    self.clear_overrides_current();
                }
                KeyCode::Char('d') if self.origin == SetupOrigin::Settings => {
                    self.disable_current();
                }
                KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                _ => {}
            },
            Mode::PathInput {
                mut executable,
                mut profile,
                mut focus,
                kind,
            } => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::List;
                    self.dirty = true;
                }
                KeyCode::Tab | KeyCode::Up | KeyCode::Down => {
                    focus = match focus {
                        PathFocus::Executable => PathFocus::Profile,
                        PathFocus::Profile => PathFocus::Executable,
                    };
                    self.mode = Mode::PathInput {
                        kind,
                        executable,
                        profile,
                        focus,
                    };
                    self.dirty = true;
                }
                KeyCode::Char(character)
                    if !character.is_control()
                        && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    match focus {
                        PathFocus::Executable => executable.push(character),
                        PathFocus::Profile => profile.push(character),
                    }
                    self.mode = Mode::PathInput {
                        kind,
                        executable,
                        profile,
                        focus,
                    };
                    self.dirty = true;
                }
                KeyCode::Backspace => {
                    match focus {
                        PathFocus::Executable => {
                            executable.pop();
                        }
                        PathFocus::Profile => {
                            profile.pop();
                        }
                    }
                    self.mode = Mode::PathInput {
                        kind,
                        executable,
                        profile,
                        focus,
                    };
                    self.dirty = true;
                }
                KeyCode::Enter => {
                    if focus == PathFocus::Executable {
                        self.mode = Mode::PathInput {
                            kind,
                            executable,
                            profile,
                            focus: PathFocus::Profile,
                        };
                        self.dirty = true;
                    } else {
                        self.mode = Mode::PathInput {
                            kind,
                            executable,
                            profile,
                            focus,
                        };
                        self.finish_path_input();
                    }
                }
                _ => {}
            },
            Mode::NeedsPathChoice { kind } => match key.code {
                KeyCode::Char('1') => {
                    self.status = format!(
                        "Install {} and add it to PATH, then restart aliasmgr and run doctor again.",
                        kind.label()
                    );
                    self.mode = Mode::List;
                    self.dirty = true;
                }
                KeyCode::Char('2') => self.begin_path_input(),
                KeyCode::Char('3') | KeyCode::Esc => {
                    self.mode = Mode::List;
                    self.status = "Cancelled.".into();
                    self.dirty = true;
                }
                _ => {}
            },
            Mode::Confirm { mut scroll, .. } => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    scroll = scroll.saturating_sub(1);
                    if let Mode::Confirm {
                        scroll: current, ..
                    } = &mut self.mode
                    {
                        *current = scroll;
                    }
                    self.dirty = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    scroll = scroll.saturating_add(1);
                    if let Mode::Confirm {
                        scroll: current, ..
                    } = &mut self.mode
                    {
                        *current = scroll;
                    }
                    self.dirty = true;
                }
                KeyCode::Char('y' | 'Y') => self.apply_confirmed(),
                KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                    self.mode = Mode::List;
                    self.status = "Cancelled.".into();
                    self.dirty = true;
                }
                _ => {}
            },
            Mode::Result => {
                if self.origin == SetupOrigin::Doctor {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                        _ => {
                            self.mode = Mode::List;
                            self.start_detection(false);
                        }
                    }
                } else {
                    // TUI flows return to the suspended screen; only the
                    // standalone doctor screen rescans in place.
                    self.quit = true;
                }
            }
        }
    }
}

fn render(frame: &mut Frame, app: &SetupApp) {
    render_in(frame, frame.area(), app);
}

/// Renders the setup state into an arbitrary area so the TUI can embed it as a
/// screen and the standalone `doctor` flow can use the whole terminal.
pub(crate) fn render_in(frame: &mut Frame, area: Rect, app: &SetupApp) {
    let list_mode = matches!(app.mode, Mode::List);
    let chunks = if list_mode {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(6),
                Constraint::Length(3),
                Constraint::Length(4),
            ])
            .split(area)
    } else {
        // Give previews/path prompts the full body so every applied target and
        // block is reachable (the confirm screen scrolls).
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(6),
                Constraint::Length(4),
            ])
            .split(area)
    };

    let title = match app.origin {
        SetupOrigin::FirstRun => "Alias Manager — Shell integration (first run)",
        SetupOrigin::Repair => "Alias Manager — Shell integration (repair)",
        SetupOrigin::Preflight => "Alias Manager — Shell integration (required by this alias)",
        SetupOrigin::Settings => "Alias Manager — Shell integration settings",
        SetupOrigin::Doctor => {
            if app.detecting {
                "Alias Manager — Shell integration (detecting...)"
            } else {
                "Alias Manager — Shell integration"
            }
        }
    };
    frame.render_widget(
        Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD)),
        chunks[0],
    );

    if list_mode {
        let items = app
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let checked = app.checked.contains(&row.kind);
                let marker = if row.wsl {
                    "-"
                } else if checked {
                    "x"
                } else {
                    " "
                };
                let plan = app.plans.get(index).and_then(|plan| plan.as_ref());
                let loader = plan
                    .map(|plan| plan.loader_status.as_str())
                    .unwrap_or("...");
                let detail = if row.wsl {
                    "WSL launcher (Enter: enter a native Windows path)".to_string()
                } else if row.detected {
                    format!(
                        "{} [{}]{}",
                        row.executable
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "<unknown>".into()),
                        loader,
                        row.version
                            .as_ref()
                            .map(|version| format!(" v{version}"))
                            .unwrap_or_default()
                    )
                } else {
                    format!("not found [{loader}]")
                };
                ListItem::new(Line::from(format!(
                    "[{marker}] {:<16} {detail}",
                    row.kind.label()
                )))
            })
            .collect::<Vec<_>>();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Shells"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default();
        if !app.rows.is_empty() {
            state.select(Some(app.selected.min(app.rows.len() - 1)));
        }
        frame.render_stateful_widget(list, chunks[1], &mut state);
        let mut keys = "↑/↓ or j/k move   Space select   Enter preview/apply   e paths   c defaults   r rescan"
            .to_string();
        if app.origin == SetupOrigin::Settings {
            keys.push_str("   d disable");
        }
        keys.push_str(if app.origin == SetupOrigin::Doctor {
            "   q quit"
        } else {
            "   q skip"
        });
        frame.render_widget(
            Paragraph::new(keys)
                .block(Block::default().borders(Borders::ALL).title("Keys"))
                .wrap(Wrap { trim: true }),
            chunks[2],
        );
        frame.render_widget(status_widget(app), chunks[3]);
        return;
    }

    let panel = match &app.mode {
        Mode::List => Paragraph::new(""),
        Mode::PathInput {
            kind,
            executable,
            profile,
            focus,
        } => {
            let marker = |is_focus: bool| if is_focus { ">" } else { " " };
            Paragraph::new(format!(
                "{} is not ready. Enter an absolute executable and profile/RC path.\n\
                 {} Executable: {}\n\
                 {} Profile/RC: {}\n\n\
                 Leading ~/ (or ~\\) expands to the home directory. Tab switches fields, \
                 Enter accepts, Esc cancels.",
                kind.label(),
                marker(*focus == PathFocus::Executable),
                executable,
                marker(*focus == PathFocus::Profile),
                profile
            ))
            .block(Block::default().borders(Borders::ALL).title("Paths"))
            .wrap(Wrap { trim: false })
        }
        Mode::NeedsPathChoice { kind } => Paragraph::new(format!(
            "{} was not detected.\n\n\
             1) Install {} and add it to PATH, then restart aliasmgr and run doctor again.\n\
             2) It is installed in a non-standard location: enter the executable and profile/RC paths.\n\
             3) Cancel without enabling this shell.",
            kind.label(),
            kind.label()
        ))
        .block(Block::default().borders(Borders::ALL).title("Options"))
        .wrap(Wrap { trim: false }),
        Mode::Confirm { previews, scroll } => {
            let mut text = String::new();
            for entry in previews {
                text.push_str(&format!(
                    "{} — {}\n{}\n\n",
                    entry.kind.label(),
                    entry.preview.target.display(),
                    entry.preview.block.trim_end()
                ));
            }
            text.push_str("↑/↓ scroll   y = apply this exact set   n = cancel");
            Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Preview"))
                .wrap(Wrap { trim: false })
                .scroll((*scroll, 0))
        }
        Mode::Result => Paragraph::new(app.status.clone())
            .block(Block::default().borders(Borders::ALL).title("Result"))
            .wrap(Wrap { trim: false }),
    };
    frame.render_widget(panel, chunks[1]);
    frame.render_widget(status_widget(app), chunks[2]);
}

fn status_widget(app: &SetupApp) -> Paragraph<'_> {
    let status = if app.mode == Mode::Result {
        if app.origin == SetupOrigin::Doctor {
            "Press any key to rescan, q to quit.".to_string()
        } else {
            "Press any key to return; the result above is kept.".to_string()
        }
    } else {
        app.status.clone()
    };
    Paragraph::new(status)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .wrap(Wrap { trim: true })
}

fn event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut SetupApp,
) -> Result<String, AliasError> {
    loop {
        if app.dirty {
            terminal.draw(|frame| render(frame, app))?;
            app.dirty = false;
        }
        if app.quit {
            break;
        }
        if event::poll(Duration::from_millis(150))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Resize(_, _) => app.dirty = true,
                _ => {}
            }
        }
        app.poll_detection();
    }
    if app.summary.is_empty() {
        Ok("doctor: no changes".into())
    } else {
        Ok(app.summary.join("\n"))
    }
}

/// Runs the interactive Shell integration screen and returns a plain summary
/// (including reload commands) for the CLI to print after the alternate screen
/// is restored.
pub fn run_shell_setup(config_dir: Option<PathBuf>) -> Result<String, AliasError> {
    let paths = AppPaths::discover(config_dir.as_deref());
    terminal::check_interactive(
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
    )?;
    let mut app = SetupApp::new(paths)?;
    terminal::install_panic_hook();
    let mut session = match TerminalSession::enter() {
        Ok(session) => session,
        Err(error) => {
            terminal::restore_panic_hook();
            return Err(AliasError::Io(error));
        }
    };
    let result = event_loop(session.terminal_mut(), &mut app);
    drop(session);
    terminal::restore_panic_hook();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_kinds_uses_selection_then_current_row() {
        // The UI behavior is stateful; this test documents the fallback rule
        // without opening a terminal.
        let app = SetupApp {
            paths: AppPaths {
                root: PathBuf::from("config"),
            },
            config: aliasmgr_core::config::AppConfig::default(),
            rows: vec![ShellInstallation {
                kind: ShellKind::Bash,
                executable: None,
                detected: false,
                source: detection::ShellDetectionSource::NotFound,
                version: None,
                wsl: false,
            }],
            plans: vec![None],
            checked: BTreeSet::new(),
            selected: 0,
            mode: Mode::List,
            detecting: false,
            receiver: None,
            pending_overrides: BTreeMap::new(),
            status: String::new(),
            summary: Vec::new(),
            quit: false,
            dirty: false,
            origin: SetupOrigin::Doctor,
        };
        assert_eq!(app.apply_kinds(), vec![ShellKind::Bash]);
    }

    #[test]
    fn confirmed_apply_uses_pending_paths_for_a_needs_path_plan() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-shell-setup-ui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let config = aliasmgr_core::config::AppConfig::default();
        config.save(&paths).unwrap();
        let executable = root.join(if cfg!(windows) { "bash.exe" } else { "bash" });
        std::fs::write(&executable, b"").unwrap();
        let profile = root.join("bashrc");
        let plan = ShellSetupPlan {
            kind: ShellKind::Bash,
            detected: false,
            executable: None,
            source: detection::ShellDetectionSource::NotFound,
            wsl: false,
            enabled: false,
            loader_target: None,
            loader_status: shell_setup::LoaderStatus::Unavailable,
            action: SetupAction::NeedsPath,
        };
        let mut pending = BTreeMap::new();
        pending.insert(ShellKind::Bash, (Some(executable), Some(profile.clone())));
        let mut app = SetupApp {
            paths,
            config,
            rows: vec![ShellInstallation {
                kind: ShellKind::Bash,
                executable: None,
                detected: false,
                source: detection::ShellDetectionSource::NotFound,
                version: None,
                wsl: false,
            }],
            plans: vec![Some(plan)],
            checked: [ShellKind::Bash].into_iter().collect(),
            selected: 0,
            mode: Mode::Confirm {
                previews: vec![PreviewEntry {
                    kind: ShellKind::Bash,
                    preview: LoaderPreview {
                        target: profile.clone(),
                        powershell: false,
                        block: String::new(),
                    },
                }],
                scroll: 0,
            },
            detecting: false,
            receiver: None,
            pending_overrides: pending,
            status: String::new(),
            summary: Vec::new(),
            quit: false,
            dirty: false,
            origin: SetupOrigin::Doctor,
        };
        app.apply_confirmed();
        let summary = app.summary.join("\n");
        assert!(summary.contains("Bash: enabled"), "{summary}");
        assert!(!summary.contains("path is still required"), "{summary}");
        let config = app.paths.load_config().unwrap();
        assert_eq!(config.shells.enabled, vec!["bash"]);
        assert!(profile.exists(), "loader profile must be written");
        assert!(std::fs::read_to_string(&profile)
            .unwrap()
            .contains(aliasmgr_core::shells::common::START_MARKER));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn confirm_previews_exactly_the_checked_set_not_only_the_cursor_row() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-shell-setup-ui-set-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let ps5_profile = root.join("ps5-profile.ps1");
        let ps7_profile = root.join("ps7-profile.ps1");
        let bash_profile = root.join("bashrc");
        let ps5_exe = root.join(if cfg!(windows) {
            "powershell.exe"
        } else {
            "powershell"
        });
        let ps7_exe = root.join(if cfg!(windows) { "pwsh.exe" } else { "pwsh" });
        std::fs::write(&ps5_exe, b"").unwrap();
        std::fs::write(&ps7_exe, b"").unwrap();
        let mut config = aliasmgr_core::config::AppConfig::default();
        config.shells.powershell5_profile_path = Some(ps5_profile.clone());
        config.shells.powershell7_profile_path = Some(ps7_profile.clone());
        config.shells.bash_rc_path = Some(bash_profile.clone());
        config
            .shells
            .executables
            .insert("powershell5".into(), ps5_exe.clone());
        config
            .shells
            .executables
            .insert("powershell7".into(), ps7_exe.clone());
        config.save(&paths).unwrap();

        let make_plan = |kind: ShellKind, executable: PathBuf, profile: PathBuf| ShellSetupPlan {
            kind,
            detected: true,
            executable: Some(executable),
            source: detection::ShellDetectionSource::Manual,
            wsl: false,
            enabled: false,
            loader_target: Some(profile),
            loader_status: shell_setup::LoaderStatus::Missing,
            action: SetupAction::InstallLoader,
        };
        let rows = vec![
            ShellInstallation {
                kind: ShellKind::PowerShell5,
                executable: Some(ps5_exe.clone()),
                detected: true,
                source: detection::ShellDetectionSource::Manual,
                version: None,
                wsl: false,
            },
            ShellInstallation {
                kind: ShellKind::PowerShell7,
                executable: Some(ps7_exe.clone()),
                detected: true,
                source: detection::ShellDetectionSource::Manual,
                version: None,
                wsl: false,
            },
            ShellInstallation {
                kind: ShellKind::Bash,
                executable: None,
                detected: false,
                source: detection::ShellDetectionSource::NotFound,
                version: None,
                wsl: false,
            },
        ];
        let plans = vec![
            Some(make_plan(
                ShellKind::PowerShell5,
                ps5_exe,
                ps5_profile.clone(),
            )),
            Some(make_plan(
                ShellKind::PowerShell7,
                ps7_exe,
                ps7_profile.clone(),
            )),
            Some(make_plan(
                ShellKind::Bash,
                root.join("missing-bash"),
                bash_profile.clone(),
            )),
        ];
        let mut app = SetupApp {
            paths,
            config,
            rows,
            plans,
            checked: [ShellKind::PowerShell5, ShellKind::PowerShell7]
                .into_iter()
                .collect(),
            // Cursor is on the unchecked Bash row; Enter must still preview and
            // apply only the checked PowerShell shells.
            selected: 2,
            mode: Mode::List,
            detecting: false,
            receiver: None,
            pending_overrides: BTreeMap::new(),
            status: String::new(),
            summary: Vec::new(),
            quit: false,
            dirty: false,
            origin: SetupOrigin::Doctor,
        };
        app.confirm_ready_row();
        let preview_kinds = match &app.mode {
            Mode::Confirm { previews, .. } => previews
                .iter()
                .map(|entry| entry.kind.clone())
                .collect::<Vec<_>>(),
            other => panic!("expected confirm, got {other:?}"),
        };
        assert_eq!(
            preview_kinds,
            vec![ShellKind::PowerShell5, ShellKind::PowerShell7]
        );
        app.apply_confirmed();
        let summary = app.summary.join("\n");
        assert!(summary.contains("PowerShell 5.1: enabled"), "{summary}");
        assert!(summary.contains("PowerShell 7: enabled"), "{summary}");
        assert!(!summary.contains("Bash: enabled"), "{summary}");
        let config = app.paths.load_config().unwrap();
        assert_eq!(config.shells.enabled, vec!["powershell5", "powershell7"]);
        assert!(ps5_profile.exists());
        assert!(ps7_profile.exists());
        assert!(!bash_profile.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn wsl_row_enter_opens_manual_path_input_without_prefilling_the_launcher() {
        let plan = ShellSetupPlan {
            kind: ShellKind::Bash,
            detected: true,
            executable: Some(PathBuf::from(r"C:\Windows\System32\bash.exe")),
            source: detection::ShellDetectionSource::Wsl,
            wsl: true,
            enabled: false,
            loader_target: None,
            loader_status: shell_setup::LoaderStatus::Unavailable,
            action: SetupAction::WslManual,
        };
        let mut app = SetupApp {
            paths: AppPaths {
                root: PathBuf::from("config"),
            },
            config: aliasmgr_core::config::AppConfig::default(),
            rows: vec![ShellInstallation {
                kind: ShellKind::Bash,
                executable: Some(PathBuf::from(r"C:\Windows\System32\bash.exe")),
                detected: true,
                source: detection::ShellDetectionSource::Wsl,
                version: None,
                wsl: true,
            }],
            plans: vec![Some(plan)],
            checked: BTreeSet::new(),
            selected: 0,
            mode: Mode::List,
            detecting: false,
            receiver: None,
            pending_overrides: BTreeMap::new(),
            status: String::new(),
            summary: Vec::new(),
            quit: false,
            dirty: false,
            origin: SetupOrigin::Doctor,
        };
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match app.mode {
            Mode::PathInput {
                executable, kind, ..
            } => {
                assert_eq!(kind, ShellKind::Bash);
                assert!(
                    executable.is_empty(),
                    "the WSL launcher must not be prefilled as the executable"
                );
            }
            other => panic!("expected path input, got {other:?}"),
        }
        assert!(app.status.contains("WSL launcher"), "{}", app.status);
    }

    #[test]
    fn confirm_refuses_a_checked_shell_that_still_needs_a_path() {
        let mut app = SetupApp {
            paths: AppPaths {
                root: PathBuf::from("config"),
            },
            config: aliasmgr_core::config::AppConfig::default(),
            rows: vec![ShellInstallation {
                kind: ShellKind::Bash,
                executable: None,
                detected: false,
                source: detection::ShellDetectionSource::NotFound,
                version: None,
                wsl: false,
            }],
            plans: vec![Some(ShellSetupPlan {
                kind: ShellKind::Bash,
                detected: false,
                executable: None,
                source: detection::ShellDetectionSource::NotFound,
                wsl: false,
                enabled: false,
                loader_target: None,
                loader_status: shell_setup::LoaderStatus::Unavailable,
                action: SetupAction::NeedsPath,
            })],
            checked: [ShellKind::Bash].into_iter().collect(),
            selected: 0,
            mode: Mode::List,
            detecting: false,
            receiver: None,
            pending_overrides: BTreeMap::new(),
            status: String::new(),
            summary: Vec::new(),
            quit: false,
            dirty: false,
            origin: SetupOrigin::Doctor,
        };
        app.confirm_ready_row();
        assert!(matches!(app.mode, Mode::List));
        assert!(app.status.contains("needs an executable"), "{}", app.status);
    }

    fn ui_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-shell-setup-ui-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn row(kind: ShellKind, executable: Option<PathBuf>) -> ShellInstallation {
        ShellInstallation {
            kind,
            detected: executable.is_some(),
            source: if executable.is_some() {
                detection::ShellDetectionSource::Manual
            } else {
                detection::ShellDetectionSource::NotFound
            },
            executable,
            version: None,
            wsl: false,
        }
    }

    fn bash_app(root: &Path, origin: SetupOrigin) -> SetupApp {
        let paths = AppPaths {
            root: root.to_path_buf(),
        };
        let executable = root.join(if cfg!(windows) { "bash.exe" } else { "bash" });
        std::fs::write(&executable, b"").unwrap();
        let mut config = aliasmgr_core::config::AppConfig::default();
        config.shells.bash_rc_path = Some(root.join("bashrc"));
        config
            .shells
            .executables
            .insert("bash".into(), executable.clone());
        config.save(&paths).unwrap();
        let mut app = SetupApp::new_with_origin(paths, origin, &[]).unwrap();
        app.inject_rows_for_test(vec![
            row(ShellKind::Bash, Some(executable)),
            row(ShellKind::Zsh, None),
            row(ShellKind::PowerShell5, None),
            row(ShellKind::PowerShell7, None),
        ]);
        app
    }

    #[test]
    fn first_run_selects_previews_applies_and_persists_enablement_and_loader() {
        let root = ui_root("first-run");
        let mut app = bash_app(&root, SetupOrigin::FirstRun);
        assert!(app.checked.is_empty(), "detection alone must not pre-check");
        assert!(app.current_plan().unwrap().loader_status == shell_setup::LoaderStatus::Missing);
        app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(app.checked.contains(&ShellKind::Bash));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(app.mode, Mode::Confirm { .. }));
        app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(matches!(app.mode, Mode::Result));
        assert!(app.has_changes());
        assert!(app.summary_text().contains("Bash: enabled"));
        let config = app.paths.load_config().unwrap();
        assert_eq!(config.shells.enabled, vec!["bash"]);
        let profile = root.join("bashrc");
        assert!(profile.exists(), "apply must install the loader");
        assert!(std::fs::read_to_string(&profile)
            .unwrap()
            .contains(aliasmgr_core::shells::common::START_MARKER));
        // Isolated root only: the test profile is the temp file above.
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn first_run_skip_quits_without_changes_or_writes() {
        let root = ui_root("skip");
        let mut app = bash_app(&root, SetupOrigin::FirstRun);
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.is_quit());
        assert!(!app.has_changes());
        assert!(app.paths.load_config().unwrap().shells.enabled.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn not_detected_enter_offers_three_options() {
        let root = ui_root("three-options");
        let paths = AppPaths { root: root.clone() };
        aliasmgr_core::config::AppConfig::default()
            .save(&paths)
            .unwrap();
        let mut app = SetupApp::new_with_origin(paths, SetupOrigin::FirstRun, &[]).unwrap();
        app.inject_rows_for_test(vec![
            row(ShellKind::Bash, None),
            row(ShellKind::Zsh, None),
            row(ShellKind::PowerShell5, None),
            row(ShellKind::PowerShell7, None),
        ]);
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.mode,
            Mode::NeedsPathChoice {
                kind: ShellKind::Bash
            }
        ));
        app.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert!(app.status.contains("Install Bash"), "{}", app.status);
        assert!(matches!(app.mode, Mode::List));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert!(app.status.contains("Cancelled"));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert!(matches!(app.mode, Mode::PathInput { .. }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn manual_paths_reject_relative_and_missing_values_and_expand_tilde() {
        let root = ui_root("paths");
        let mut app = bash_app(&root, SetupOrigin::FirstRun);
        let executable = root.join(if cfg!(windows) { "bash.exe" } else { "bash" });

        app.mode = Mode::PathInput {
            kind: ShellKind::Bash,
            executable: "relative/bash".into(),
            profile: String::new(),
            focus: PathFocus::Executable,
        };
        app.finish_path_input();
        assert!(
            app.status.contains("Executable path must be absolute"),
            "{}",
            app.status
        );

        app.mode = Mode::PathInput {
            kind: ShellKind::Bash,
            executable: "~/definitely-missing-aliasmgr-bash".into(),
            profile: String::new(),
            focus: PathFocus::Executable,
        };
        app.finish_path_input();
        assert!(
            app.status.contains("Executable not found"),
            "{}",
            app.status
        );
        assert!(
            !app.status.contains("must be absolute"),
            "tilde must expand before validation: {}",
            app.status
        );

        app.mode = Mode::PathInput {
            kind: ShellKind::Bash,
            executable: executable.display().to_string(),
            profile: "relative/profile".into(),
            focus: PathFocus::Profile,
        };
        app.finish_path_input();
        assert!(
            app.status.contains("Profile/RC path must be absolute"),
            "{}",
            app.status
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn checked_wsl_row_blocks_confirm_until_a_native_path_is_supplied() {
        let root = ui_root("wsl-confirm");
        let paths = AppPaths { root: root.clone() };
        aliasmgr_core::config::AppConfig::default()
            .save(&paths)
            .unwrap();
        let mut app = SetupApp::new_with_origin(paths, SetupOrigin::FirstRun, &[]).unwrap();
        app.inject_rows_for_test(vec![ShellInstallation {
            kind: ShellKind::Bash,
            executable: Some(PathBuf::from(r"C:\Windows\System32\bash.exe")),
            detected: true,
            source: detection::ShellDetectionSource::Wsl,
            version: None,
            wsl: true,
        }]);
        app.checked.insert(ShellKind::Bash);
        app.confirm_ready_row();
        assert!(matches!(app.mode, Mode::List));
        assert!(app.status.contains("WSL launcher"), "{}", app.status);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn settings_origin_disable_persists_and_clear_previews_defaults_without_writing() {
        let root = ui_root("settings-origin");
        let mut app = bash_app(&root, SetupOrigin::Settings);
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert!(app.paths.load_config().unwrap().shells.enabled.is_empty());
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        let (executable, profile) = app.pending_overrides.get(&ShellKind::Bash).unwrap();
        assert_eq!(
            executable.as_deref(),
            Some(Path::new(shell_setup::CLEAR_OVERRIDE))
        );
        assert_eq!(
            profile.as_deref(),
            Some(Path::new(shell_setup::CLEAR_OVERRIDE))
        );
        assert!(app.status.contains("platform defaults"), "{}", app.status);
        // Nothing is persisted until the previewed set is confirmed.
        assert!(app
            .paths
            .load_config()
            .unwrap()
            .shells
            .executables
            .contains_key("bash"));
        let _ = std::fs::remove_dir_all(root);
    }
}
