//! Interactive editor for the shared `config.toml`.
//!
//! The TUI mutates only the fields it shows, on top of the `AppConfig` loaded
//! from disk, so shell path overrides and unknown top-level keys survive a
//! save. Validation follows the shared `AppConfig::validate` rules.

use crate::input::{is_actionable, is_ctrl, TextInput};
use aliasmgr_core::{
    config::{AppConfig, AppPaths, MAX_RETAINED_FILES},
    error::AliasError,
};
use crossterm::event::{KeyCode, KeyEvent};
use std::path::{Path, PathBuf};

pub const SHELL_OPTIONS: [Option<&str>; 5] = [
    None,
    Some("bash"),
    Some("zsh"),
    Some("powershell5"),
    Some("powershell7"),
];
pub const LANGUAGE_OPTIONS: [&str; 2] = ["zh", "en"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    DefaultShell,
    Language,
    AllowRelativePaths,
    RcKeep,
    GeneratedKeep,
    DbKeep,
    LogKeep,
}

impl SettingsField {
    pub const ALL: [SettingsField; 7] = [
        SettingsField::DefaultShell,
        SettingsField::Language,
        SettingsField::AllowRelativePaths,
        SettingsField::RcKeep,
        SettingsField::GeneratedKeep,
        SettingsField::DbKeep,
        SettingsField::LogKeep,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsField::DefaultShell => "runtime.default_shell",
            SettingsField::Language => "runtime.language",
            SettingsField::AllowRelativePaths => "runtime.allow_relative_paths",
            SettingsField::RcKeep => "backups.rc_keep",
            SettingsField::GeneratedKeep => "backups.generated_keep",
            SettingsField::DbKeep => "backups.db_keep",
            SettingsField::LogKeep => "logs.keep_files",
        }
    }

    pub fn is_text(self) -> bool {
        matches!(
            self,
            SettingsField::RcKeep
                | SettingsField::GeneratedKeep
                | SettingsField::DbKeep
                | SettingsField::LogKeep
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsDraft {
    pub default_shell: Option<String>,
    pub language: String,
    pub allow_relative_paths: bool,
    pub rc_keep: String,
    pub generated_keep: String,
    pub db_keep: String,
    pub log_keep: String,
}

impl SettingsDraft {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            default_shell: config.runtime.default_shell.clone(),
            language: config.runtime.language.clone(),
            allow_relative_paths: config.runtime.allow_relative_paths,
            rc_keep: config.backups.rc_keep.to_string(),
            generated_keep: config.backups.generated_keep.to_string(),
            db_keep: config.backups.db_keep.to_string(),
            log_keep: config.logs.keep_files.to_string(),
        }
    }

    pub fn display(&self, field: SettingsField) -> String {
        match field {
            SettingsField::DefaultShell => self
                .default_shell
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "(none)".to_string()),
            SettingsField::Language => self.language.clone(),
            SettingsField::AllowRelativePaths => {
                if self.allow_relative_paths {
                    "[x] allowed".to_string()
                } else {
                    "[ ] denied".to_string()
                }
            }
            SettingsField::RcKeep => self.rc_keep.clone(),
            SettingsField::GeneratedKeep => self.generated_keep.clone(),
            SettingsField::DbKeep => self.db_keep.clone(),
            SettingsField::LogKeep => self.log_keep.clone(),
        }
    }

    pub fn text_mut(&mut self, field: SettingsField) -> Option<&mut String> {
        match field {
            SettingsField::RcKeep => Some(&mut self.rc_keep),
            SettingsField::GeneratedKeep => Some(&mut self.generated_keep),
            SettingsField::DbKeep => Some(&mut self.db_keep),
            SettingsField::LogKeep => Some(&mut self.log_keep),
            _ => None,
        }
    }

    pub fn text(&self, field: SettingsField) -> Option<&str> {
        match field {
            SettingsField::RcKeep => Some(&self.rc_keep),
            SettingsField::GeneratedKeep => Some(&self.generated_keep),
            SettingsField::DbKeep => Some(&self.db_keep),
            SettingsField::LogKeep => Some(&self.log_keep),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError {
    pub field: Option<SettingsField>,
    pub message: String,
}

impl SettingsError {
    pub fn new(field: Option<SettingsField>, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

fn count_field(message: &str) -> Option<SettingsField> {
    if message.contains("rc_keep") {
        Some(SettingsField::RcKeep)
    } else if message.contains("generated_keep") {
        Some(SettingsField::GeneratedKeep)
    } else if message.contains("db_keep") {
        Some(SettingsField::DbKeep)
    } else if message.contains("keep_files") {
        Some(SettingsField::LogKeep)
    } else {
        None
    }
}

fn validation_field(error: &AliasError) -> Option<SettingsField> {
    let message = error.to_string();
    if message.contains("default shell") {
        Some(SettingsField::DefaultShell)
    } else if message.contains("language") {
        Some(SettingsField::Language)
    } else {
        count_field(&message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsOutcome {
    None,
    Dirty,
    SaveRequested,
    CancelRequested,
}

#[derive(Debug, Clone)]
pub struct SettingsState {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub base: AppConfig,
    pub draft: SettingsDraft,
    pub original: SettingsDraft,
    pub field: SettingsField,
    pub input: TextInput,
    pub error: Option<SettingsError>,
}

impl SettingsState {
    pub fn new(root: &Path) -> Result<Self, AliasError> {
        let paths = AppPaths {
            root: root.to_path_buf(),
        };
        let base = paths.load_config()?;
        let draft = SettingsDraft::from_config(&base);
        let input = TextInput::new(draft.text(SettingsField::DefaultShell).unwrap_or_default());
        Ok(Self {
            config_dir: root.to_path_buf(),
            config_file: paths.config_file(),
            base,
            original: draft.clone(),
            draft,
            field: SettingsField::DefaultShell,
            input,
            error: None,
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.draft != self.original
    }

    pub fn set_field(&mut self, field: SettingsField) {
        self.commit_input();
        self.field = field;
        if let Some(value) = self.draft.text(field) {
            self.input = TextInput::new(value.to_string());
        }
    }

    fn commit_input(&mut self) {
        if self.field.is_text() {
            let value = self.input.value().to_string();
            if let Some(target) = self.draft.text_mut(self.field) {
                *target = value;
            }
        }
    }

    pub fn next_field(&mut self, backwards: bool) {
        self.commit_input();
        let fields = SettingsField::ALL;
        let index = fields
            .iter()
            .position(|field| *field == self.field)
            .unwrap_or(0);
        let next = if backwards {
            (index + fields.len() - 1) % fields.len()
        } else {
            (index + 1) % fields.len()
        };
        self.set_field(fields[next]);
    }

    pub fn cycle_default_shell(&mut self, backwards: bool) {
        let current = self.draft.default_shell.as_deref();
        let index = SHELL_OPTIONS
            .iter()
            .position(|option| *option == current)
            .unwrap_or(0);
        let next = if backwards {
            (index + SHELL_OPTIONS.len() - 1) % SHELL_OPTIONS.len()
        } else {
            (index + 1) % SHELL_OPTIONS.len()
        };
        self.draft.default_shell = SHELL_OPTIONS[next].map(str::to_string);
    }

    pub fn cycle_language(&mut self, backwards: bool) {
        let index = LANGUAGE_OPTIONS
            .iter()
            .position(|option| *option == self.draft.language.trim())
            .unwrap_or(0);
        let next = if backwards {
            (index + LANGUAGE_OPTIONS.len() - 1) % LANGUAGE_OPTIONS.len()
        } else {
            (index + 1) % LANGUAGE_OPTIONS.len()
        };
        self.draft.language = LANGUAGE_OPTIONS[next].to_string();
    }

    fn parse_count(&self, field: SettingsField, value: &str) -> Result<usize, SettingsError> {
        let parsed = value.trim().parse::<usize>().map_err(|_| {
            SettingsError::new(
                Some(field),
                format!("{} must be a non-negative integer", field.label()),
            )
        })?;
        if parsed > MAX_RETAINED_FILES {
            return Err(SettingsError::new(
                Some(field),
                format!("{} must be <= {MAX_RETAINED_FILES}", field.label()),
            ));
        }
        Ok(parsed)
    }

    /// Builds the full config on top of the loaded base, preserving unknown
    /// keys and shell path overrides the editor does not expose.
    pub fn to_config(&self) -> Result<AppConfig, SettingsError> {
        self.to_config_on(&self.base)
    }

    fn to_config_on(&self, base: &AppConfig) -> Result<AppConfig, SettingsError> {
        let mut config = base.clone();
        config.runtime.default_shell = match &self.draft.default_shell {
            Some(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
            _ => None,
        };
        config.runtime.language = self.draft.language.trim().to_string();
        config.runtime.allow_relative_paths = self.draft.allow_relative_paths;
        config.backups.rc_keep = self.parse_count(SettingsField::RcKeep, &self.draft.rc_keep)?;
        config.backups.generated_keep =
            self.parse_count(SettingsField::GeneratedKeep, &self.draft.generated_keep)?;
        config.backups.db_keep = self.parse_count(SettingsField::DbKeep, &self.draft.db_keep)?;
        config.logs.keep_files = self.parse_count(SettingsField::LogKeep, &self.draft.log_keep)?;
        config
            .validate()
            .map_err(|error| SettingsError::new(validation_field(&error), error.to_string()))?;
        Ok(config)
    }

    pub fn save(&mut self) -> Result<(), SettingsError> {
        let paths = AppPaths {
            root: self.config_dir.clone(),
        };
        // Re-read before writing so fields the TUI does not edit (shell path
        // overrides, unknown top-level keys, unrelated settings) are not
        // replaced with the snapshot loaded when this screen opened.
        let fresh = match paths.load_config() {
            Ok(config) => config,
            Err(error) => {
                let failure = SettingsError::new(
                    None,
                    format!(
                        "could not reload config.toml before saving: {error}; settings were not saved"
                    ),
                );
                self.error = Some(failure.clone());
                return Err(failure);
            }
        };
        let config = match self.to_config_on(&fresh) {
            Ok(config) => config,
            Err(error) => {
                self.error = Some(error.clone());
                return Err(error);
            }
        };
        if let Err(error) = config.save(&paths) {
            let message = friendly_save_error(&error, &self.config_file);
            let failure = SettingsError::new(None, message);
            self.error = Some(failure.clone());
            return Err(failure);
        }
        self.base = config;
        self.original = self.draft.clone();
        self.error = None;
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SettingsOutcome {
        if !is_actionable(&key) {
            return SettingsOutcome::None;
        }
        if is_ctrl(&key, 's') {
            self.commit_input();
            return SettingsOutcome::SaveRequested;
        }
        match key.code {
            KeyCode::Esc => return SettingsOutcome::CancelRequested,
            KeyCode::Tab => {
                self.next_field(false);
                return SettingsOutcome::Dirty;
            }
            KeyCode::BackTab => {
                self.next_field(true);
                return SettingsOutcome::Dirty;
            }
            _ => {}
        }
        if self.field.is_text() {
            if key.code == KeyCode::Enter {
                self.next_field(false);
                return SettingsOutcome::Dirty;
            }
            let changed = self.input.handle_key(key);
            if changed {
                self.commit_input();
                return SettingsOutcome::Dirty;
            }
            return SettingsOutcome::None;
        }
        match self.field {
            SettingsField::DefaultShell => match key.code {
                KeyCode::Left => {
                    self.cycle_default_shell(true);
                    SettingsOutcome::Dirty
                }
                KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.cycle_default_shell(false);
                    SettingsOutcome::Dirty
                }
                _ => SettingsOutcome::None,
            },
            SettingsField::Language => match key.code {
                KeyCode::Left => {
                    self.cycle_language(true);
                    SettingsOutcome::Dirty
                }
                KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.cycle_language(false);
                    SettingsOutcome::Dirty
                }
                _ => SettingsOutcome::None,
            },
            SettingsField::AllowRelativePaths => match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.draft.allow_relative_paths = !self.draft.allow_relative_paths;
                    SettingsOutcome::Dirty
                }
                _ => SettingsOutcome::None,
            },
            _ => SettingsOutcome::None,
        }
    }
}

fn friendly_save_error(error: &AliasError, path: &Path) -> String {
    match error {
        AliasError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => {
            format!(
                "permission denied writing {}; settings were not saved",
                path.display()
            )
        }
        AliasError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            format!(
                "configuration directory is missing for {}; settings were not saved",
                path.display()
            )
        }
        _ => format!(
            "could not save {}: {error}; settings were not saved",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aliasmgr_core::config::ShellPathOverrides;
    use crossterm::event::KeyModifiers;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aliasmgr-settings-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ))
    }

    #[test]
    fn missing_config_loads_defaults() {
        let root = temp_root("defaults");
        fs::create_dir_all(&root).unwrap();
        let state = SettingsState::new(&root).unwrap();
        assert_eq!(state.config_dir, root);
        assert!(!SettingsField::ALL
            .iter()
            .any(|field| field.label().contains("config_dir")));
        assert_eq!(state.draft.default_shell, None);
        assert_eq!(state.draft.language, "zh");
        assert!(!state.draft.allow_relative_paths);
        assert_eq!(state.draft.rc_keep, "10");
        assert_eq!(state.draft.generated_keep, "10");
        assert_eq!(state.draft.db_keep, "5");
        assert_eq!(state.draft.log_keep, "7");
        assert!(!state.is_dirty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_round_trips_values_and_preserves_shell_overrides_and_extra_keys() {
        let root = temp_root("roundtrip");
        fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let config = AppConfig {
            shells: ShellPathOverrides {
                bash_rc_path: Some(PathBuf::from("/tmp/custom-bashrc")),
                ..Default::default()
            },
            ..AppConfig::default()
        };
        config.save(&paths).unwrap();
        let saved = fs::read_to_string(paths.config_file()).unwrap();
        fs::write(
            paths.config_file(),
            format!("custom_key = \"keep\"\n{saved}"),
        )
        .unwrap();

        let mut state = SettingsState::new(&root).unwrap();
        state.draft.default_shell = Some("zsh".into());
        state.draft.language = "en".into();
        state.draft.allow_relative_paths = true;
        state.draft.rc_keep = "3".into();
        state.draft.generated_keep = "4".into();
        state.draft.db_keep = "2".into();
        state.draft.log_keep = "6".into();
        assert!(state.is_dirty());
        state.save().unwrap();
        assert!(!state.is_dirty());

        let loaded = AppPaths { root: root.clone() }.load_config().unwrap();
        assert_eq!(loaded.runtime.default_shell.as_deref(), Some("zsh"));
        assert_eq!(loaded.runtime.language, "en");
        assert!(loaded.runtime.allow_relative_paths);
        assert_eq!(loaded.backups.rc_keep, 3);
        assert_eq!(loaded.backups.generated_keep, 4);
        assert_eq!(loaded.backups.db_keep, 2);
        assert_eq!(loaded.logs.keep_files, 6);
        assert_eq!(
            loaded.shells.bash_rc_path.as_deref(),
            Some(Path::new("/tmp/custom-bashrc"))
        );
        assert_eq!(
            loaded
                .extra
                .get("custom_key")
                .and_then(|value| value.as_str()),
            Some("keep")
        );
        let raw = fs::read_to_string(paths.config_file()).unwrap();
        assert!(raw.contains("custom_key = \"keep\""), "{raw}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_reloads_external_config_changes_before_applying_edits() {
        let root = temp_root("concurrent");
        fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let initial = AppConfig::default();
        initial.save(&paths).unwrap();
        let mut state = SettingsState::new(&root).unwrap();

        // Another front end changes fields the TUI does not edit and adds an
        // unknown top-level key after the screen was opened.
        let mut external = paths.load_config().unwrap();
        external.shells.bash_rc_path = Some(PathBuf::from("/tmp/external-bashrc"));
        external.logs.max_file_bytes = 12345;
        external.save(&paths).unwrap();
        let saved = fs::read_to_string(paths.config_file()).unwrap();
        fs::write(
            paths.config_file(),
            format!("external_key = \"kept\"\n{saved}"),
        )
        .unwrap();

        state.draft.language = "en".into();
        state.save().unwrap();
        let loaded = paths.load_config().unwrap();
        assert_eq!(loaded.runtime.language, "en");
        assert_eq!(
            loaded.shells.bash_rc_path.as_deref(),
            Some(Path::new("/tmp/external-bashrc"))
        );
        assert_eq!(loaded.logs.max_file_bytes, 12345);
        assert_eq!(
            loaded
                .extra
                .get("external_key")
                .and_then(|value| value.as_str()),
            Some("kept")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_shell_language_and_counts_block_save_and_keep_the_file() {
        let root = temp_root("invalid");
        fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        AppConfig::default().save(&paths).unwrap();
        let before = fs::read_to_string(paths.config_file()).unwrap();
        let mut state = SettingsState::new(&root).unwrap();

        state.draft.default_shell = Some("fish".into());
        let error = state.save().unwrap_err();
        assert_eq!(error.field, Some(SettingsField::DefaultShell));
        state.draft.default_shell = Some("bash".into());

        state.draft.language = "fr".into();
        let error = state.save().unwrap_err();
        assert_eq!(error.field, Some(SettingsField::Language));
        state.draft.language = "zh".into();

        state.draft.rc_keep = "abc".into();
        let error = state.save().unwrap_err();
        assert_eq!(error.field, Some(SettingsField::RcKeep));
        state.draft.rc_keep = "10".into();

        state.draft.log_keep = (MAX_RETAINED_FILES + 1).to_string();
        let error = state.save().unwrap_err();
        assert_eq!(error.field, Some(SettingsField::LogKeep));

        assert_eq!(fs::read_to_string(paths.config_file()).unwrap(), before);
        assert!(state.is_dirty());
        assert!(state.error.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[allow(clippy::permissions_set_readonly_false)] // test cleanup only
    fn read_only_config_file_keeps_the_draft() {
        let root = temp_root("readonly");
        fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        AppConfig::default().save(&paths).unwrap();
        let mut state = SettingsState::new(&root).unwrap();
        state.draft.language = "en".into();
        let mut permissions = fs::metadata(paths.config_file()).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(paths.config_file(), permissions.clone()).unwrap();
        let result = state.save();
        assert!(
            result.is_err(),
            "read-only config file must not be overwritten"
        );
        assert!(state.is_dirty());
        assert_eq!(state.draft.language, "en");
        assert!(state.error.is_some());
        permissions.set_readonly(false);
        let _ = fs::set_permissions(paths.config_file(), permissions);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_failure_keeps_the_draft_and_reports_a_friendly_error() {
        let root = temp_root("save-failure");
        fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        AppConfig::default().save(&paths).unwrap();
        let mut state = SettingsState::new(&root).unwrap();
        state.draft.language = "en".into();
        // Replace the file with a directory so fs::write fails deterministically
        // on every platform.
        fs::remove_file(paths.config_file()).unwrap();
        fs::create_dir(paths.config_file()).unwrap();
        let error = state.save().unwrap_err();
        assert!(
            error.message.contains("settings were not saved"),
            "{error:?}"
        );
        assert!(state.is_dirty());
        assert_eq!(state.draft.language, "en");
        assert!(state.error.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn field_navigation_cycles_shell_language_and_boolean_values() {
        let root = temp_root("navigation");
        fs::create_dir_all(&root).unwrap();
        let mut state = SettingsState::new(&root).unwrap();
        state.set_field(SettingsField::DefaultShell);
        state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(state.draft.default_shell.as_deref(), Some("bash"));
        state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(state.draft.default_shell.as_deref(), Some("zsh"));
        state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(state.draft.default_shell.as_deref(), Some("bash"));
        state.set_field(SettingsField::Language);
        state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(state.draft.language, "en");
        state.set_field(SettingsField::AllowRelativePaths);
        state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(state.draft.allow_relative_paths);
        state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(state.field, SettingsField::RcKeep);
        assert!(state.is_dirty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn text_fields_consume_shortcut_letters() {
        let root = temp_root("text-input");
        fs::create_dir_all(&root).unwrap();
        let mut state = SettingsState::new(&root).unwrap();
        state.set_field(SettingsField::RcKeep);
        state.input.clear();
        for character in "12q3".chars() {
            state.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        assert_eq!(state.draft.rc_keep, "12q3");
        assert_eq!(
            state.to_config().unwrap_err().field,
            Some(SettingsField::RcKeep)
        );
        let _ = fs::remove_dir_all(root);
    }
}
