//! Import/export flow state for the TUI.
//!
//! All file parsing, conflict resolution, sensitive-environment filtering and
//! atomic persistence stays in `aliasmgr_core::transfer`; this module only
//! models the screens, path input, conflict strategy choice and preview/result
//! phases.

use crate::{
    data::{transfer_format, ExportSummary, ImportReportSummary},
    input::{is_actionable, TextInput},
};
use aliasmgr_core::transfer::ConflictStrategy;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

pub fn strategy_label(strategy: ConflictStrategy) -> &'static str {
    match strategy {
        ConflictStrategy::Ask => "Ask (conflicts are skipped after preview)",
        ConflictStrategy::Skip => "Skip",
        ConflictStrategy::Overwrite => "Overwrite",
        ConflictStrategy::Rename => "Rename",
    }
}

fn next_strategy(strategy: ConflictStrategy) -> ConflictStrategy {
    match strategy {
        ConflictStrategy::Ask => ConflictStrategy::Skip,
        ConflictStrategy::Skip => ConflictStrategy::Overwrite,
        ConflictStrategy::Overwrite => ConflictStrategy::Rename,
        ConflictStrategy::Rename => ConflictStrategy::Ask,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPhase {
    Input,
    Preview,
    Confirming,
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportAction {
    None,
    Preview {
        path: PathBuf,
        strategy: ConflictStrategy,
    },
    Confirm {
        path: PathBuf,
        strategy: ConflictStrategy,
    },
    Back,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportState {
    pub path_input: TextInput,
    pub path: Option<PathBuf>,
    pub strategy: ConflictStrategy,
    pub phase: ImportPhase,
    pub report: Option<ImportReportSummary>,
    pub error: Option<String>,
    pub busy: bool,
    pub scroll: u16,
}

impl Default for ImportState {
    fn default() -> Self {
        Self {
            path_input: TextInput::default(),
            path: None,
            strategy: ConflictStrategy::Ask,
            phase: ImportPhase::Input,
            report: None,
            error: None,
            busy: false,
            scroll: 0,
        }
    }
}

impl ImportState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn format(&self) -> &'static str {
        self.path
            .as_deref()
            .map(transfer_format)
            .unwrap_or("unknown")
    }

    pub fn begin_preview(&mut self) {
        self.busy = true;
        self.phase = ImportPhase::Preview;
        self.report = None;
        self.error = None;
        self.scroll = 0;
    }

    pub fn begin_confirm(&mut self) {
        self.busy = true;
        self.phase = ImportPhase::Confirming;
        self.error = None;
    }

    pub fn on_preview(&mut self, report: ImportReportSummary) {
        self.busy = false;
        self.phase = ImportPhase::Preview;
        self.report = Some(report);
        self.error = None;
        self.scroll = 0;
    }

    pub fn on_confirmed(&mut self, report: ImportReportSummary) {
        self.busy = false;
        self.phase = ImportPhase::Result;
        self.report = Some(report);
        self.error = None;
        self.scroll = 0;
    }

    pub fn on_error(&mut self, message: String) {
        self.busy = false;
        self.phase = ImportPhase::Input;
        self.report = None;
        self.error = Some(message);
        self.scroll = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ImportAction {
        if !is_actionable(&key) || self.busy {
            return ImportAction::None;
        }
        match self.phase {
            ImportPhase::Input => match key.code {
                KeyCode::Esc => ImportAction::Back,
                KeyCode::Enter => {
                    let value = self.path_input.value().trim().to_string();
                    if value.is_empty() {
                        self.error = Some("enter a file path to import".to_string());
                        return ImportAction::None;
                    }
                    let path = PathBuf::from(value);
                    self.path = Some(path.clone());
                    self.error = None;
                    // Stay in Input until the caller actually starts the
                    // preview worker; a refused start must not leave an empty
                    // Preview screen that can be confirmed.
                    ImportAction::Preview {
                        path,
                        strategy: self.strategy,
                    }
                }
                _ => {
                    if self.path_input.handle_key(key) {
                        self.error = None;
                    }
                    ImportAction::None
                }
            },
            ImportPhase::Preview => match key.code {
                KeyCode::Esc => ImportAction::Back,
                KeyCode::Char('c') => {
                    self.strategy = next_strategy(self.strategy);
                    match &self.path {
                        Some(path) => ImportAction::Preview {
                            path: path.clone(),
                            strategy: self.strategy,
                        },
                        None => ImportAction::None,
                    }
                }
                KeyCode::Char('y') | KeyCode::Enter => match &self.path {
                    Some(path) => ImportAction::Confirm {
                        path: path.clone(),
                        strategy: self.strategy,
                    },
                    None => ImportAction::None,
                },
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll = self.scroll.saturating_add(1);
                    ImportAction::None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll = self.scroll.saturating_sub(1);
                    ImportAction::None
                }
                _ => ImportAction::None,
            },
            ImportPhase::Confirming | ImportPhase::Result => match key.code {
                KeyCode::Esc | KeyCode::Enter => ImportAction::Back,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll = self.scroll.saturating_add(1);
                    ImportAction::None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll = self.scroll.saturating_sub(1);
                    ImportAction::None
                }
                _ => ImportAction::None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPhase {
    Input,
    ConfirmOverwrite,
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportAction {
    None,
    Start { path: PathBuf, overwrite: bool },
    Back,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportState {
    pub path_input: TextInput,
    pub path: Option<PathBuf>,
    pub phase: ExportPhase,
    pub summary: Option<ExportSummary>,
    pub error: Option<String>,
    pub busy: bool,
    pub scroll: u16,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            path_input: TextInput::default(),
            path: None,
            phase: ExportPhase::Input,
            summary: None,
            error: None,
            busy: false,
            scroll: 0,
        }
    }
}

impl ExportState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn format(&self) -> &'static str {
        self.path
            .as_deref()
            .map(transfer_format)
            .unwrap_or("unknown")
    }

    pub fn on_result(&mut self, summary: ExportSummary) {
        self.busy = false;
        self.phase = ExportPhase::Result;
        self.summary = Some(summary);
        self.error = None;
        self.scroll = 0;
    }

    pub fn on_error(&mut self, message: String) {
        self.busy = false;
        self.error = Some(message);
        self.scroll = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ExportAction {
        if !is_actionable(&key) || self.busy {
            return ExportAction::None;
        }
        match self.phase {
            ExportPhase::Input => match key.code {
                KeyCode::Esc => ExportAction::Back,
                KeyCode::Enter => {
                    let value = self.path_input.value().trim().to_string();
                    if value.is_empty() {
                        self.error = Some("enter a file path to export".to_string());
                        return ExportAction::None;
                    }
                    let path = PathBuf::from(value);
                    self.path = Some(path.clone());
                    if path.is_dir() {
                        self.error = Some(format!("target is a directory: {}", path.display()));
                        self.phase = ExportPhase::Input;
                        return ExportAction::None;
                    }
                    self.error = None;
                    if path.exists() {
                        self.phase = ExportPhase::ConfirmOverwrite;
                        ExportAction::None
                    } else {
                        ExportAction::Start {
                            path,
                            overwrite: false,
                        }
                    }
                }
                _ => {
                    if self.path_input.handle_key(key) {
                        self.error = None;
                    }
                    ExportAction::None
                }
            },
            ExportPhase::ConfirmOverwrite => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => match &self.path {
                    Some(path) => {
                        let path = path.clone();
                        ExportAction::Start {
                            path,
                            overwrite: true,
                        }
                    }
                    None => ExportAction::None,
                },
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.phase = ExportPhase::Input;
                    ExportAction::None
                }
                _ => ExportAction::None,
            },
            ExportPhase::Result => match key.code {
                KeyCode::Esc | KeyCode::Enter => ExportAction::Back,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll = self.scroll.saturating_add(1);
                    ExportAction::None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll = self.scroll.saturating_sub(1);
                    ExportAction::None
                }
                _ => ExportAction::None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs;
    use std::path::Path;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn temp_path(label: &str, extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aliasmgr-transfer-ui-{label}-{}-{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default(),
            extension
        ))
    }

    #[test]
    fn import_path_input_previews_and_cycles_strategy() {
        let mut state = ImportState::new();
        for character in "in.toml".chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }
        let action = state.handle_key(key(KeyCode::Enter));
        match action {
            ImportAction::Preview { path, strategy } => {
                assert_eq!(path, PathBuf::from("in.toml"));
                assert_eq!(strategy, ConflictStrategy::Ask);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(state.phase, ImportPhase::Input);
        state.begin_preview();
        state.busy = false; // simulated worker completion
        assert_eq!(state.phase, ImportPhase::Preview);
        assert_eq!(state.format(), "TOML");
        let action = state.handle_key(key(KeyCode::Char('c')));
        assert!(matches!(
            action,
            ImportAction::Preview {
                strategy: ConflictStrategy::Skip,
                ..
            }
        ));
        assert_eq!(
            state.handle_key(key(KeyCode::Char('y'))),
            ImportAction::Confirm {
                path: PathBuf::from("in.toml"),
                strategy: ConflictStrategy::Skip
            }
        );
    }

    #[test]
    fn import_preview_is_not_persisted_by_the_state_machine() {
        let state = ImportState::new();
        assert!(state.report.is_none());
        assert!(!Path::new("in.json").exists());
    }

    #[test]
    fn export_requires_overwrite_confirmation_for_existing_files() {
        let path = temp_path("existing", "json");
        fs::write(&path, "x").unwrap();
        let mut state = ExportState::new();
        for character in path.to_string_lossy().chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }
        let action = state.handle_key(key(KeyCode::Enter));
        assert_eq!(action, ExportAction::None);
        assert_eq!(state.phase, ExportPhase::ConfirmOverwrite);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('y'))),
            ExportAction::Start {
                path: path.clone(),
                overwrite: true
            }
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn export_rejects_a_directory_without_an_overwrite_prompt() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-transfer-ui-dir-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        let mut state = ExportState::new();
        for character in root.to_string_lossy().chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }
        assert_eq!(state.handle_key(key(KeyCode::Enter)), ExportAction::None);
        assert_eq!(state.phase, ExportPhase::Input);
        assert!(state
            .error
            .as_deref()
            .unwrap()
            .contains("target is a directory"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_cancelled_overwrite_does_not_start() {
        let path = temp_path("cancel", "toml");
        fs::write(&path, "x").unwrap();
        let mut state = ExportState::new();
        for character in path.to_string_lossy().chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }
        assert_eq!(state.handle_key(key(KeyCode::Enter)), ExportAction::None);
        assert_eq!(state.handle_key(key(KeyCode::Esc)), ExportAction::None);
        assert_eq!(state.phase, ExportPhase::Input);
        assert_eq!(state.format(), "TOML");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn preview_advances_only_when_the_worker_starts() {
        let mut state = ImportState::new();
        for character in "in.json".chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }
        let action = state.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, ImportAction::Preview { .. }));
        assert_eq!(state.phase, ImportPhase::Input);
        state.begin_preview();
        assert_eq!(state.phase, ImportPhase::Preview);
        assert!(state.busy);
        assert!(state
            .handle_key(key(KeyCode::Char('y')))
            .eq(&ImportAction::None));
        state.on_error("worker refused".into());
        assert_eq!(state.phase, ImportPhase::Input);
        assert!(!state.busy);
        assert!(state.error.as_deref().unwrap().contains("worker refused"));
    }

    #[test]
    fn confirming_phase_is_reported_until_the_result_arrives() {
        let mut state = ImportState::new();
        state.path = Some(PathBuf::from("in.json"));
        state.begin_confirm();
        assert_eq!(state.phase, ImportPhase::Confirming);
        assert!(state.busy);
        state.on_confirmed(ImportReportSummary {
            imported: vec!["gs".into()],
            skipped: Vec::new(),
            unsupported: Vec::new(),
            warnings: Vec::new(),
            strategy: ConflictStrategy::Ask,
            path: PathBuf::from("in.json"),
            format: "JSON",
            overwrite_existing: Default::default(),
            file_fingerprint: Default::default(),
        });
        assert_eq!(state.phase, ImportPhase::Result);
        assert!(!state.busy);
    }
}
