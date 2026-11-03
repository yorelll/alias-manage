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
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::IsTerminal,
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathFocus {
    Executable,
    Profile,
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
        preview: LoaderPreview,
        apply_kinds: Vec<ShellKind>,
    },
    NeedsPathChoice {
        kind: ShellKind,
    },
    Result,
}

struct SetupApp {
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
}

impl SetupApp {
    fn new(paths: AppPaths) -> Result<Self, AliasError> {
        let config = paths.load_config()?;
        let mut app = Self {
            paths,
            config,
            rows: Vec::new(),
            plans: Vec::new(),
            checked: BTreeSet::new(),
            selected: 0,
            mode: Mode::List,
            detecting: false,
            receiver: None,
            pending_overrides: BTreeMap::new(),
            status: "Detecting shells... version probes may take a moment.".into(),
            summary: Vec::new(),
            quit: false,
            dirty: true,
        };
        app.start_detection(true);
        Ok(app)
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

    fn poll_detection(&mut self) {
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
                shell_setup::plan_shell_setup_with_installation(
                    &self.config,
                    &self.paths,
                    &row.kind,
                    row,
                    None,
                    None,
                )
                .ok()
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
        let executable = self
            .current_plan()
            .and_then(|plan| plan.executable.clone())
            .filter(|path| path.is_file())
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let profile = shell_setup::default_profile_path(&self.config, &self.paths, &kind)
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        self.mode = Mode::PathInput {
            kind,
            executable,
            profile,
            focus: PathFocus::Executable,
        };
        self.status = "Enter an executable path, then a profile/RC path. Esc cancels.".into();
        self.dirty = true;
    }

    fn confirm_ready_row(&mut self) {
        let Some(plan) = self.current_plan().cloned() else {
            return;
        };
        let kind = plan.kind.clone();
        let preview =
            match shell_setup::preview_loader(&self.config, &self.paths, &kind, None, None) {
                Ok(Some(preview)) => preview,
                Ok(None) => {
                    self.status = format!(
                        "{} has no profile path on this platform; enter a path first.",
                        kind.label()
                    );
                    self.dirty = true;
                    return;
                }
                Err(error) => {
                    self.status = format!("{}: {error}", kind.label());
                    self.dirty = true;
                    return;
                }
            };
        self.mode = Mode::Confirm {
            preview,
            apply_kinds: self.apply_kinds(),
        };
        self.status = "Review the target and block. y applies, n cancels.".into();
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
        let executable_path = PathBuf::from(executable.trim());
        if !executable_path.is_file() {
            self.status = format!("Executable not found: {}", executable_path.display());
            self.dirty = true;
            return;
        }
        let profile_path = profile.trim();
        let profile_override = (!profile_path.is_empty()).then(|| PathBuf::from(profile_path));
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
        let preview = match shell_setup::preview_loader(
            &self.config,
            &self.paths,
            &kind,
            Some(&executable_path),
            profile_override.as_deref(),
        ) {
            Ok(Some(preview)) => preview,
            Ok(None) => {
                self.status = format!("{} has no usable profile target.", kind.label());
                self.dirty = true;
                return;
            }
            Err(error) => {
                self.status = format!("{}: {error}", kind.label());
                self.dirty = true;
                return;
            }
        };
        self.pending_overrides
            .insert(kind.clone(), (Some(executable_path), profile_override));
        self.checked.insert(kind);
        self.mode = Mode::Confirm {
            preview,
            apply_kinds: self.apply_kinds(),
        };
        self.status = "Review the target and block. y applies, n cancels.".into();
        self.dirty = true;
    }

    fn apply_confirmed(&mut self) {
        let Mode::Confirm { apply_kinds, .. } = self.mode.clone() else {
            return;
        };
        let mut summary = Vec::new();
        for kind in apply_kinds {
            let action = self.plan_for(&kind).map(|plan| plan.action);
            let has_override = self.pending_overrides.contains_key(&kind);
            match action {
                Some(SetupAction::WslManual) => {
                    summary.push(format!(
                        "{}: WSL launcher; run doctor inside the WSL distribution",
                        kind.label()
                    ));
                    continue;
                }
                // The plan is read-only and does not include the paths just
                // typed in the prompt, so an override satisfies NeedsPath here.
                Some(SetupAction::NeedsPath) if !has_override => {
                    summary.push(format!("{}: a path is still required", kind.label()));
                    continue;
                }
                None => {
                    summary.push(format!("{}: no setup plan", kind.label()));
                    continue;
                }
                Some(SetupAction::Enable)
                | Some(SetupAction::InstallLoader)
                | Some(SetupAction::NeedsPath) => {}
            }
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

    fn handle_key(&mut self, key: KeyEvent) {
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
                                "WSL bash must be integrated inside the distribution.".into();
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
                                self.status =
                                    "WSL bash: run doctor inside the WSL distribution.".into();
                                self.dirty = true;
                            }
                            SetupAction::Enable | SetupAction::InstallLoader => {
                                self.confirm_ready_row()
                            }
                        }
                    }
                }
                KeyCode::Char('r') => self.start_detection(false),
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
            Mode::Confirm { .. } => match key.code {
                KeyCode::Char('y' | 'Y') => self.apply_confirmed(),
                KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                    self.mode = Mode::List;
                    self.status = "Cancelled.".into();
                    self.dirty = true;
                }
                _ => {}
            },
            Mode::Result => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                _ => {
                    self.mode = Mode::List;
                    self.start_detection(false);
                }
            },
        }
    }
}

fn render(frame: &mut Frame, app: &SetupApp) {
    let panel_height = match app.mode {
        Mode::List => 3,
        _ => 14,
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(6),
            Constraint::Length(panel_height),
            Constraint::Length(4),
        ])
        .split(frame.area());

    let title = if app.detecting {
        "Alias Manager — Shell integration (detecting...)"
    } else {
        "Alias Manager — Shell integration"
    };
    frame.render_widget(
        Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD)),
        chunks[0],
    );

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
                "WSL launcher (integrate inside the distribution)".to_string()
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

    let panel = match &app.mode {
        Mode::List => Paragraph::new(
            "↑/↓ or j/k move   Space enable   Enter preview/apply   r rescan   q quit",
        )
        .block(Block::default().borders(Borders::ALL).title("Keys"))
        .wrap(Wrap { trim: true }),
        Mode::PathInput {
            kind,
            executable,
            profile,
            focus,
        } => {
            let marker = |is_focus: bool| if is_focus { ">" } else { " " };
            Paragraph::new(format!(
                "{} is not ready. Enter executable and profile/RC paths.\n\
                 {} Executable: {}\n\
                 {} Profile/RC: {}\n\n\
                 Tab switches fields, Enter accepts, Esc cancels.",
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
        Mode::Confirm { preview, .. } => Paragraph::new(format!(
            "Target: {}\n\n{}\n\ny = apply, n = cancel",
            preview.target.display(),
            preview.block.trim_end()
        ))
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: false }),
        Mode::Result => Paragraph::new(app.status.clone())
            .block(Block::default().borders(Borders::ALL).title("Result"))
            .wrap(Wrap { trim: false }),
    };
    frame.render_widget(panel, chunks[2]);

    let status = if app.mode == Mode::Result {
        "Press any key to rescan, q to quit.".to_string()
    } else {
        app.status.clone()
    };
    frame.render_widget(
        Paragraph::new(status)
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .wrap(Wrap { trim: true }),
        chunks[3],
    );
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
                preview: LoaderPreview {
                    target: profile.clone(),
                    powershell: false,
                    block: String::new(),
                },
                apply_kinds: vec![ShellKind::Bash],
            },
            detecting: false,
            receiver: None,
            pending_overrides: pending,
            status: String::new(),
            summary: Vec::new(),
            quit: false,
            dirty: false,
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
}
