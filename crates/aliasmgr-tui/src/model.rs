//! Alias draft and the editing state machine.
//!
//! The draft is a UI-side projection only. Validation, checksums, tombstones
//! and persistence all stay in `aliasmgr-core`; [`AliasDraft::to_record`] builds
//! an `AliasRecord` and calls the same `validate_alias` the CLI and GUI use.

use crate::input::{is_actionable, is_ctrl, TextInput};
use aliasmgr_core::{
    error::AliasError,
    executor::render_argv,
    model::{
        normalize_restriction, record_checksum, AliasRecord, PathMode, PathRestriction,
        RestrictionMode, ShellKind, TargetType,
    },
    validation::validate_alias,
};
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::BTreeMap;

/// The four Shells the interactive front end manages. Records imported with
/// `fish`/`posix_sh` keep those shells as extra rows in the editor.
pub fn shell_choices() -> Vec<ShellKind> {
    vec![
        ShellKind::Bash,
        ShellKind::Zsh,
        ShellKind::PowerShell5,
        ShellKind::PowerShell7,
    ]
}

pub fn target_type_choices() -> Vec<TargetType> {
    vec![
        TargetType::NativeExecutable,
        TargetType::PythonScript,
        TargetType::PowerShellScript,
        TargetType::Batch,
        TargetType::ShellScript,
        TargetType::JavaJar,
        TargetType::Cmdlet,
        TargetType::ChangeDirectory,
    ]
}

pub fn target_type_label(target: &TargetType) -> &'static str {
    match target {
        TargetType::NativeExecutable => "native_executable",
        TargetType::PythonScript => "python_script",
        TargetType::PowerShellScript => "powershell_script",
        TargetType::Batch => "batch",
        TargetType::ShellScript => "shell_script",
        TargetType::JavaJar => "java_jar",
        TargetType::Cmdlet => "cmdlet",
        TargetType::ChangeDirectory => "change_directory",
        TargetType::RawShellCommand => "raw_shell_command",
    }
}

/// The runtime semantics that matter when choosing a target type. Only `batch`
/// adds validation and `change_directory` changes the command; the remaining
/// types are classification metadata and still run `executable` directly.
pub fn target_type_semantics(target: &TargetType) -> &'static str {
    match target {
        TargetType::NativeExecutable => "runs `executable` directly",
        TargetType::PythonScript => {
            "classification only; runs `executable` directly (use executable=python, args=script.py)"
        }
        TargetType::PowerShellScript => {
            "classification only; runs `executable` directly (no interpreter is added)"
        }
        TargetType::Batch => "runs through the batch executor; % ! & | ^ < > are rejected",
        TargetType::ShellScript => {
            "classification only; runs `executable` directly (no interpreter is added)"
        }
        TargetType::JavaJar => {
            "classification only; runs `executable` directly (use executable=java, args=-jar ...)"
        }
        TargetType::Cmdlet => "classification only; runs `executable` directly",
        TargetType::ChangeDirectory => "changes the invocation directory instead of running a tool",
        TargetType::RawShellCommand => "unsupported: raw shell commands are rejected by the model",
    }
}

pub fn shell_label(shell: &ShellKind) -> &'static str {
    shell.as_str()
}

/// Readiness of one shell as shown next to the editor's shell checkboxes. It
/// is a display projection of shared `shell_setup` state, never a save rule:
/// the strict preflight runs the same `shell_readiness_problems` check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellChoiceState {
    Enabled,
    Disabled,
    NotInstalled,
    LoaderInvalid,
    Wsl,
}

impl ShellChoiceState {
    pub fn label(self) -> &'static str {
        match self {
            ShellChoiceState::Enabled => "[enabled]",
            ShellChoiceState::Disabled => "[disabled]",
            ShellChoiceState::NotInstalled => "[not installed]",
            ShellChoiceState::LoaderInvalid => "[loader invalid]",
            ShellChoiceState::Wsl => "[wsl: integrate inside WSL]",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    TargetType,
    Executable,
    FixedArgs,
    PassArgs,
    Shells,
    Description,
    Tags,
    WorkingDirectory,
    Environment,
    PathMode,
    RestrictionMode,
    RestrictionPaths,
}

impl Field {
    pub const BASIC: [Field; 8] = [
        Field::Name,
        Field::TargetType,
        Field::Executable,
        Field::FixedArgs,
        Field::PassArgs,
        Field::Shells,
        Field::Description,
        Field::Tags,
    ];
    pub const ADVANCED: [Field; 5] = [
        Field::WorkingDirectory,
        Field::Environment,
        Field::PathMode,
        Field::RestrictionMode,
        Field::RestrictionPaths,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Field::Name => "Name",
            Field::TargetType => "Target type",
            Field::Executable => "Executable",
            Field::FixedArgs => "Fixed args",
            Field::PassArgs => "Pass user args",
            Field::Shells => "Shells",
            Field::Description => "Description",
            Field::Tags => "Tags",
            Field::WorkingDirectory => "Working dir",
            Field::Environment => "Environment",
            Field::PathMode => "Path mode",
            Field::RestrictionMode => "Restriction",
            Field::RestrictionPaths => "Restriction paths",
        }
    }

    pub fn is_advanced(self) -> bool {
        Self::ADVANCED.contains(&self)
    }

    pub fn is_text(self) -> bool {
        matches!(
            self,
            Field::Name | Field::Executable | Field::Description | Field::WorkingDirectory
        )
    }
}

pub fn visible_fields(advanced: bool) -> Vec<Field> {
    let mut fields = Field::BASIC.to_vec();
    if advanced {
        fields.extend(Field::ADVANCED);
    }
    fields
}

/// A validation failure with the editor field it most likely belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftError {
    pub field: Option<Field>,
    pub message: String,
}

impl DraftError {
    pub fn new(field: Option<Field>, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

fn field_for_error(error: &AliasError) -> Option<Field> {
    match error {
        AliasError::InvalidAliasName
        | AliasError::NameReserved
        | AliasError::ExactNameConflict(_)
        | AliasError::CaseFoldConflict(_)
        | AliasError::AliasConflict(_) => Some(Field::Name),
        AliasError::InvalidArgTemplate => Some(Field::FixedArgs),
        AliasError::AdvancedModeUnsupported => Some(Field::TargetType),
        AliasError::InvalidAliasRecord(message) => {
            if message.contains("executable") {
                Some(Field::Executable)
            } else if message.contains("shell") {
                Some(Field::Shells)
            } else if message.contains("restriction") {
                Some(Field::RestrictionPaths)
            } else {
                None
            }
        }
        _ => None,
    }
}

impl From<AliasError> for DraftError {
    fn from(error: AliasError) -> Self {
        Self {
            field: field_for_error(&error),
            message: error.to_string(),
        }
    }
}

/// Everything the editor can change. Derived equality drives the dirty check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasDraft {
    pub name: String,
    pub description: String,
    pub target_type: TargetType,
    pub executable: String,
    pub fixed_args: Vec<String>,
    pub pass_args: bool,
    pub working_directory: String,
    pub environment: Vec<(String, String)>,
    pub shells: Vec<ShellKind>,
    pub tags: Vec<String>,
    pub path_mode: PathMode,
    pub restriction_mode: Option<RestrictionMode>,
    pub restriction_paths: Vec<String>,
    pub enabled: bool,
}

impl Default for AliasDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            target_type: TargetType::NativeExecutable,
            executable: String::new(),
            fixed_args: Vec::new(),
            pass_args: true,
            working_directory: String::new(),
            environment: Vec::new(),
            shells: vec![ShellKind::Bash],
            tags: Vec::new(),
            path_mode: PathMode::Absolute,
            restriction_mode: None,
            restriction_paths: Vec::new(),
            enabled: true,
        }
    }
}

impl AliasDraft {
    pub fn from_record(record: &AliasRecord) -> Self {
        Self {
            name: record.name.clone(),
            description: record.description.clone(),
            target_type: record.target_type.clone(),
            executable: record.executable.clone(),
            fixed_args: record.fixed_args.clone(),
            pass_args: record.pass_args,
            working_directory: record.working_directory.clone().unwrap_or_default(),
            environment: record
                .environment
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            shells: record.shells.clone(),
            tags: record.tags.clone(),
            path_mode: record.path_mode.clone(),
            restriction_mode: record.restriction.as_ref().map(|value| value.mode),
            restriction_paths: record
                .restriction
                .as_ref()
                .map(|value| value.paths.clone())
                .unwrap_or_default(),
            enabled: record.enabled,
        }
    }

    /// Builds a validated `AliasRecord`. `base` supplies the identity and
    /// immutable metadata (id, created_at, path origin, advanced mode).
    pub fn to_record(&self, base: &AliasRecord) -> Result<AliasRecord, DraftError> {
        let record = self.build_record(base)?;
        validate_alias(&record).map_err(DraftError::from)?;
        Ok(record)
    }

    /// Builds the record shape without whole-record validation, so the argv
    /// preview still shows argument boundaries while the user is typing a new
    /// alias name. Saving always goes through [`Self::to_record`].
    fn build_record(&self, base: &AliasRecord) -> Result<AliasRecord, DraftError> {
        for (key, _value) in &self.environment {
            if key.is_empty() {
                return Err(DraftError::new(
                    Some(Field::Environment),
                    "environment key must not be empty",
                ));
            }
            if key.contains('=') {
                return Err(DraftError::new(
                    Some(Field::Environment),
                    "environment key must not contain '='",
                ));
            }
        }
        let paths: Vec<String> = self
            .restriction_paths
            .iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect();
        let restriction = match (self.restriction_mode, paths.is_empty()) {
            (None, true) => None,
            (None, false) => {
                return Err(DraftError::new(
                    Some(Field::RestrictionMode),
                    "choose whitelist or blacklist for the restriction paths",
                ))
            }
            (Some(mode), _) => Some(PathRestriction { mode, paths }),
        };
        let mut environment = BTreeMap::new();
        for (key, value) in &self.environment {
            environment.insert(key.clone(), value.clone());
        }
        let tags = trimmed_unique(&self.tags);
        let shells = canonical_shells(&self.shells);
        let mut record = base.clone();
        record.name = self.name.clone();
        record.description = self.description.clone();
        record.target_type = self.target_type.clone();
        record.executable = self.executable.clone();
        record.fixed_args = self.fixed_args.clone();
        record.pass_args = self.pass_args;
        record.working_directory = if self.working_directory.trim().is_empty() {
            None
        } else {
            Some(self.working_directory.clone())
        };
        record.environment = environment;
        record.shells = shells;
        record.tags = tags;
        record.path_mode = self.path_mode.clone();
        record.restriction = restriction;
        record.enabled = self.enabled;
        normalize_restriction(&mut record.restriction);
        record.record_checksum = record_checksum(&record).map_err(|error| {
            DraftError::new(None, format!("could not build record checksum: {error}"))
        })?;
        Ok(record)
    }

    /// The argv the alias would execute, using two placeholder user arguments
    /// so the `{{args}}` insertion point is visible. This is the same expansion
    /// the generated wrappers use; nothing is split or re-joined.
    pub fn argv_preview(&self, base: &AliasRecord) -> Result<Vec<String>, DraftError> {
        let record = self.build_record(base)?;
        render_argv(&record, &["<arg1>".to_string(), "<arg2>".to_string()])
            .map_err(|error| DraftError::new(field_for_error(&error), error.to_string()))
    }
}

fn trimmed_unique(values: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || result.iter().any(|existing| existing == trimmed) {
            continue;
        }
        result.push(trimmed.to_string());
    }
    result
}

fn canonical_shells(selected: &[ShellKind]) -> Vec<ShellKind> {
    let mut result: Vec<ShellKind> = Vec::new();
    for shell in shell_choices() {
        if selected.contains(&shell) && !result.contains(&shell) {
            result.push(shell);
        }
    }
    // Preserve imported shells the four built-in toggles do not cover.
    for shell in selected {
        if !result.contains(shell) {
            result.push(shell.clone());
        }
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListInputStage {
    Value,
    EnvKey,
    EnvValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorOutcome {
    None,
    Dirty,
    SaveRequested,
    CancelRequested,
}

/// Editing state for one alias. `base_revision` is the revision the draft was
/// loaded from and is sent to the revision-checked update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    pub draft: AliasDraft,
    pub original: AliasDraft,
    pub base: AliasRecord,
    pub is_new: bool,
    pub base_revision: i64,
    pub field: Field,
    pub advanced: bool,
    pub input: TextInput,
    pub list_input: Option<TextInput>,
    pub list_input_stage: ListInputStage,
    pub cursor: usize,
    pub shell_cursor: usize,
    /// Detection/enablement labels for the four built-in shells, filled by the
    /// App when the editor opens (empty for imported-only shells).
    pub shell_choice_states: BTreeMap<ShellKind, ShellChoiceState>,
    pub error: Option<DraftError>,
    pub saving: bool,
    pub scroll: u16,
    /// Set when a save was refused because the stored revision moved on.
    pub conflict_revision: Option<i64>,
}

impl Editor {
    pub fn new(record: Option<&AliasRecord>) -> Self {
        let base = record.cloned().unwrap_or_default();
        let draft = if record.is_some() {
            AliasDraft::from_record(&base)
        } else {
            AliasDraft::default()
        };
        let input = TextInput::new(draft.name.clone());
        Self {
            original: draft.clone(),
            draft,
            base_revision: record.map(|value| value.revision).unwrap_or(0),
            is_new: record.is_none(),
            base,
            field: Field::Name,
            advanced: false,
            input,
            list_input: None,
            list_input_stage: ListInputStage::Value,
            cursor: 0,
            shell_cursor: 0,
            shell_choice_states: BTreeMap::new(),
            error: None,
            saving: false,
            scroll: 0,
            conflict_revision: None,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.draft != self.original
    }

    pub fn title(&self) -> String {
        if self.is_new {
            "New alias".to_string()
        } else {
            format!("Edit {}", self.base.name)
        }
    }

    pub fn shell_editor_choices(&self) -> Vec<ShellKind> {
        let mut choices = shell_choices();
        for shell in &self.draft.shells {
            if !choices.contains(shell) {
                choices.push(shell.clone());
            }
        }
        choices
    }

    pub fn env_display(&self) -> Vec<String> {
        self.draft
            .environment
            .iter()
            .map(|(key, value)| {
                format!(
                    "{}={}",
                    if key.is_empty() { "(empty key)" } else { key },
                    crate::text::mask_value(value)
                )
            })
            .collect()
    }

    pub fn validate_now(&self) -> Result<AliasRecord, DraftError> {
        self.draft.to_record(&self.base)
    }

    fn commit_text(&mut self) {
        match self.field {
            Field::Name => self.draft.name = self.input.value().to_string(),
            Field::Executable => self.draft.executable = self.input.value().to_string(),
            Field::Description => self.draft.description = self.input.value().to_string(),
            Field::WorkingDirectory => {
                self.draft.working_directory = self.input.value().to_string()
            }
            _ => {}
        }
    }

    fn load_text(&mut self) {
        let value = match self.field {
            Field::Name => self.draft.name.clone(),
            Field::Executable => self.draft.executable.clone(),
            Field::Description => self.draft.description.clone(),
            Field::WorkingDirectory => self.draft.working_directory.clone(),
            _ => String::new(),
        };
        self.input = TextInput::new(value);
    }

    pub fn set_field(&mut self, field: Field) {
        if self.list_input.is_some() {
            self.commit_list_input(false);
        }
        self.commit_text();
        self.field = field;
        self.cursor = 0;
        self.shell_cursor = 0;
        if field.is_text() {
            self.load_text();
        }
    }

    pub fn next_field(&mut self, backwards: bool) {
        if self.list_input.is_some() {
            self.commit_list_input(false);
        }
        self.commit_text();
        let fields = visible_fields(self.advanced);
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

    pub fn toggle_advanced(&mut self) {
        self.advanced = !self.advanced;
        if !self.advanced && self.field.is_advanced() {
            self.set_field(Field::Name);
        }
    }

    fn list_len(&self) -> usize {
        match self.field {
            Field::FixedArgs => self.draft.fixed_args.len(),
            Field::Tags => self.draft.tags.len(),
            Field::Environment => self.draft.environment.len(),
            Field::RestrictionPaths => self.draft.restriction_paths.len(),
            _ => 0,
        }
    }

    fn list_get(&self, index: usize) -> String {
        match self.field {
            Field::FixedArgs => self
                .draft
                .fixed_args
                .get(index)
                .cloned()
                .unwrap_or_default(),
            Field::Tags => self.draft.tags.get(index).cloned().unwrap_or_default(),
            Field::Environment => self
                .draft
                .environment
                .get(index)
                .map(|(key, value)| match self.list_input_stage {
                    ListInputStage::EnvKey => key.clone(),
                    _ => value.clone(),
                })
                .unwrap_or_default(),
            Field::RestrictionPaths => self
                .draft
                .restriction_paths
                .get(index)
                .cloned()
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn list_set(&mut self, index: usize, value: String) {
        match self.field {
            Field::FixedArgs => {
                if let Some(item) = self.draft.fixed_args.get_mut(index) {
                    *item = value;
                }
            }
            Field::Tags => {
                if let Some(item) = self.draft.tags.get_mut(index) {
                    *item = value;
                }
            }
            Field::RestrictionPaths => {
                if let Some(item) = self.draft.restriction_paths.get_mut(index) {
                    *item = value;
                }
            }
            Field::Environment => {
                if let Some((key, item)) = self.draft.environment.get_mut(index) {
                    match self.list_input_stage {
                        ListInputStage::EnvKey => *key = value,
                        _ => *item = value,
                    }
                }
            }
            _ => {}
        }
    }

    fn list_push_empty(&mut self) {
        match self.field {
            Field::FixedArgs => self.draft.fixed_args.push(String::new()),
            Field::Tags => self.draft.tags.push(String::new()),
            Field::RestrictionPaths => self.draft.restriction_paths.push(String::new()),
            Field::Environment => self.draft.environment.push((String::new(), String::new())),
            _ => {}
        }
    }

    fn list_remove(&mut self, index: usize) {
        match self.field {
            Field::FixedArgs if index < self.draft.fixed_args.len() => {
                self.draft.fixed_args.remove(index);
            }
            Field::Tags if index < self.draft.tags.len() => {
                self.draft.tags.remove(index);
            }
            Field::RestrictionPaths if index < self.draft.restriction_paths.len() => {
                self.draft.restriction_paths.remove(index);
            }
            Field::Environment if index < self.draft.environment.len() => {
                self.draft.environment.remove(index);
            }
            _ => {}
        }
    }

    fn list_swap(&mut self, left: usize, right: usize) {
        match self.field {
            Field::FixedArgs => self.draft.fixed_args.swap(left, right),
            Field::Tags => self.draft.tags.swap(left, right),
            Field::RestrictionPaths => self.draft.restriction_paths.swap(left, right),
            Field::Environment => self.draft.environment.swap(left, right),
            _ => {}
        }
    }

    fn start_list_edit(&mut self, new_entry: bool) {
        if new_entry {
            self.list_push_empty();
            self.cursor = self.list_len().saturating_sub(1);
        }
        if self.list_len() == 0 {
            return;
        }
        self.cursor = self.cursor.min(self.list_len() - 1);
        self.list_input_stage = if self.field == Field::Environment {
            if new_entry {
                ListInputStage::EnvKey
            } else {
                ListInputStage::EnvValue
            }
        } else {
            ListInputStage::Value
        };
        let value = self.list_get(self.cursor);
        self.list_input = Some(TextInput::new(value));
    }

    /// Commits the active sub-editor. When `advance_env` is true, Enter on an
    /// environment key moves on to its value; otherwise the sub-editor closes.
    fn commit_list_input(&mut self, advance_env: bool) {
        let Some(input) = self.list_input.take() else {
            return;
        };
        let value = input.value().to_string();
        self.list_set(self.cursor, value);
        if advance_env
            && self.field == Field::Environment
            && self.list_input_stage == ListInputStage::EnvKey
        {
            self.list_input_stage = ListInputStage::EnvValue;
            let value = self.list_get(self.cursor);
            self.list_input = Some(TextInput::new(value));
        }
    }

    fn cancel_list_input(&mut self) {
        self.list_input = None;
    }

    fn move_cursor(&mut self, delta: i64) {
        let len = self.list_len();
        if len == 0 {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as i64 + delta;
        self.cursor = next.clamp(0, len as i64 - 1) as usize;
    }

    fn toggle_shell(&mut self) {
        let choices = self.shell_editor_choices();
        let Some(shell) = choices.get(self.shell_cursor).cloned() else {
            return;
        };
        if let Some(index) = self.draft.shells.iter().position(|item| *item == shell) {
            self.draft.shells.remove(index);
        } else {
            self.draft.shells.push(shell);
        }
    }

    fn cycle_target_type(&mut self, delta: i64) {
        let choices = target_type_choices();
        let index = choices
            .iter()
            .position(|choice| *choice == self.draft.target_type)
            .unwrap_or(0) as i64;
        let next = (index + delta).rem_euclid(choices.len() as i64) as usize;
        self.draft.target_type = choices[next].clone();
    }

    fn cycle_restriction_mode(&mut self, delta: i64) {
        let modes = [
            None,
            Some(RestrictionMode::Whitelist),
            Some(RestrictionMode::Blacklist),
        ];
        let index = modes
            .iter()
            .position(|mode| *mode == self.draft.restriction_mode)
            .unwrap_or(0) as i64;
        let next = (index + delta).rem_euclid(modes.len() as i64) as usize;
        self.draft.restriction_mode = modes[next];
    }

    /// Handles a key while the editor is the active screen. Text fields consume
    /// every printable character, so `s`, `q`, `d`, `/` never reach globals.
    pub fn handle_key(&mut self, key: KeyEvent) -> EditorOutcome {
        if !is_actionable(&key) {
            return EditorOutcome::None;
        }
        if is_ctrl(&key, 's') {
            // A list sub-editor keeps its text outside the draft until Enter.
            // Commit it before saving so the typed value is not silently lost.
            self.commit_list_input(false);
            self.commit_text();
            return EditorOutcome::SaveRequested;
        }
        if let Some(input) = self.list_input.as_mut() {
            match key.code {
                KeyCode::Enter => {
                    self.commit_list_input(true);
                    return EditorOutcome::Dirty;
                }
                KeyCode::Esc => {
                    self.cancel_list_input();
                    return EditorOutcome::Dirty;
                }
                KeyCode::Tab => {
                    self.commit_list_input(false);
                    self.next_field(false);
                    return EditorOutcome::Dirty;
                }
                KeyCode::BackTab => {
                    self.commit_list_input(false);
                    self.next_field(true);
                    return EditorOutcome::Dirty;
                }
                _ => {
                    let changed = input.handle_key(key);
                    return if changed {
                        EditorOutcome::Dirty
                    } else {
                        EditorOutcome::None
                    };
                }
            }
        }
        if key.code == KeyCode::F(3) {
            self.toggle_advanced();
            return EditorOutcome::Dirty;
        }
        match key.code {
            KeyCode::Tab => {
                self.next_field(false);
                return EditorOutcome::Dirty;
            }
            KeyCode::BackTab => {
                self.next_field(true);
                return EditorOutcome::Dirty;
            }
            KeyCode::Esc => return EditorOutcome::CancelRequested,
            _ => {}
        }
        if self.field.is_text() {
            if key.code == KeyCode::Enter {
                self.commit_text();
                self.next_field(false);
                return EditorOutcome::Dirty;
            }
            let changed = self.input.handle_key(key);
            if changed {
                self.commit_text();
                return EditorOutcome::Dirty;
            }
            return EditorOutcome::None;
        }
        match self.field {
            Field::TargetType => match key.code {
                KeyCode::Left => {
                    self.cycle_target_type(-1);
                    EditorOutcome::Dirty
                }
                KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.cycle_target_type(1);
                    EditorOutcome::Dirty
                }
                _ => EditorOutcome::None,
            },
            Field::PassArgs => match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.draft.pass_args = !self.draft.pass_args;
                    EditorOutcome::Dirty
                }
                _ => EditorOutcome::None,
            },
            Field::PathMode => match key.code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                    self.draft.path_mode = match &self.draft.path_mode {
                        PathMode::Absolute => PathMode::RelativeToWorkingDirectory,
                        PathMode::RelativeToWorkingDirectory => PathMode::Absolute,
                    };
                    EditorOutcome::Dirty
                }
                _ => EditorOutcome::None,
            },
            Field::RestrictionMode => match key.code {
                KeyCode::Left => {
                    self.cycle_restriction_mode(-1);
                    EditorOutcome::Dirty
                }
                KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.cycle_restriction_mode(1);
                    EditorOutcome::Dirty
                }
                _ => EditorOutcome::None,
            },
            Field::Shells => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.shell_cursor = self.shell_cursor.saturating_sub(1);
                    EditorOutcome::Dirty
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.shell_editor_choices().len().saturating_sub(1);
                    self.shell_cursor = (self.shell_cursor + 1).min(max);
                    EditorOutcome::Dirty
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.toggle_shell();
                    EditorOutcome::Dirty
                }
                _ => EditorOutcome::None,
            },
            Field::FixedArgs | Field::Tags | Field::Environment | Field::RestrictionPaths => {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.move_cursor(-1);
                        EditorOutcome::Dirty
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.move_cursor(1);
                        EditorOutcome::Dirty
                    }
                    KeyCode::Char('K') => {
                        if self.cursor > 0 {
                            self.list_swap(self.cursor, self.cursor - 1);
                            self.cursor -= 1;
                        }
                        EditorOutcome::Dirty
                    }
                    KeyCode::Char('J') => {
                        if self.cursor + 1 < self.list_len() {
                            self.list_swap(self.cursor, self.cursor + 1);
                            self.cursor += 1;
                        }
                        EditorOutcome::Dirty
                    }
                    KeyCode::Char('a') => {
                        self.start_list_edit(true);
                        EditorOutcome::Dirty
                    }
                    KeyCode::Char('d') => {
                        self.list_remove(self.cursor);
                        self.cursor = self.cursor.min(self.list_len().saturating_sub(1));
                        EditorOutcome::Dirty
                    }
                    KeyCode::Enter => {
                        self.start_list_edit(false);
                        EditorOutcome::Dirty
                    }
                    _ => EditorOutcome::None,
                }
            }
            _ => EditorOutcome::None,
        }
    }

    /// Replaces the draft with the current stored record (conflict reload).
    pub fn reload_from(&mut self, record: &AliasRecord) {
        self.base = record.clone();
        self.base_revision = record.revision;
        self.draft = AliasDraft::from_record(record);
        self.original = self.draft.clone();
        self.field = Field::Name;
        self.list_input = None;
        self.error = None;
        self.conflict_revision = None;
        self.load_text();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::is_actionable;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typing(text: &str) -> Vec<KeyEvent> {
        text.chars().map(|c| key(KeyCode::Char(c))).collect()
    }

    fn sample_record() -> AliasRecord {
        let mut environment = BTreeMap::new();
        environment.insert("TOKEN".to_string(), "hunter2".to_string());
        AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            fixed_args: vec!["status".into(), "with space".into()],
            environment,
            shells: vec![ShellKind::Bash, ShellKind::PowerShell7],
            description: "git status".into(),
            tags: vec!["git".into()],
            ..Default::default()
        }
    }

    #[test]
    fn draft_round_trips_every_editable_field() {
        let record = sample_record();
        let draft = AliasDraft::from_record(&record);
        let rebuilt = draft.to_record(&record).unwrap();
        assert_eq!(rebuilt.name, record.name);
        assert_eq!(rebuilt.fixed_args, record.fixed_args);
        assert_eq!(rebuilt.environment, record.environment);
        assert_eq!(rebuilt.shells, record.shells);
        assert_eq!(rebuilt.tags, record.tags);
        assert_eq!(rebuilt.description, record.description);
        assert_eq!(rebuilt.record_checksum, record_checksum(&rebuilt).unwrap());
    }

    #[test]
    fn structured_args_preserve_spaces_quotes_empty_and_cjk() {
        let mut editor = Editor::new(None);
        editor.set_field(Field::FixedArgs);
        editor.draft.name = "cm".into();
        editor.draft.executable = "python3".into();
        // a -> edit new empty argument
        assert_eq!(
            editor.handle_key(key(KeyCode::Char('a'))),
            EditorOutcome::Dirty
        );
        for event in typing("two words") {
            editor.handle_key(event);
        }
        editor.handle_key(key(KeyCode::Enter));
        editor.handle_key(key(KeyCode::Char('a')));
        for event in typing("say \"hi\"") {
            editor.handle_key(event);
        }
        editor.handle_key(key(KeyCode::Enter));
        editor.handle_key(key(KeyCode::Char('a')));
        for event in typing("中文 参数") {
            editor.handle_key(event);
        }
        editor.handle_key(key(KeyCode::Enter));
        editor.handle_key(key(KeyCode::Char('a')));
        editor.handle_key(key(KeyCode::Enter)); // commit empty string
        editor.handle_key(key(KeyCode::Char('a')));
        for event in typing("{{args}}") {
            editor.handle_key(event);
        }
        editor.handle_key(key(KeyCode::Enter));
        assert_eq!(
            editor.draft.fixed_args,
            vec![
                "two words".to_string(),
                "say \"hi\"".to_string(),
                "中文 参数".to_string(),
                String::new(),
                "{{args}}".to_string()
            ]
        );
        let record = editor.draft.to_record(&editor.base).unwrap();
        assert_eq!(record.fixed_args, editor.draft.fixed_args);
        // Expansion keeps argv boundaries; the empty string remains an element.
        let argv = render_argv(&record, &["u1".into()]).unwrap();
        assert_eq!(
            argv,
            vec![
                "python3".to_string(),
                "two words".to_string(),
                "say \"hi\"".to_string(),
                "中文 参数".to_string(),
                String::new(),
                "u1".to_string()
            ]
        );
    }

    #[test]
    fn argv_preview_works_while_a_new_alias_name_is_still_empty() {
        let editor = Editor::new(None);
        let mut draft = editor.draft.clone();
        draft.executable = "python3".into();
        draft.fixed_args = vec!["script.py".into(), "two words".into()];
        assert_eq!(
            draft.argv_preview(&editor.base).unwrap(),
            vec![
                "python3".to_string(),
                "script.py".to_string(),
                "two words".to_string(),
                "<arg1>".to_string(),
                "<arg2>".to_string()
            ]
        );
        // Saving still requires the core name validation.
        assert_eq!(
            draft.to_record(&editor.base).unwrap_err().field,
            Some(Field::Name)
        );
    }

    #[test]
    fn args_can_be_reordered_and_deleted_without_rejoining() {
        let mut editor = Editor::new(Some(&sample_record()));
        editor.set_field(Field::FixedArgs);
        assert_eq!(editor.draft.fixed_args, vec!["status", "with space"]);
        editor.handle_key(key(KeyCode::Char('J')));
        assert_eq!(editor.draft.fixed_args, vec!["with space", "status"]);
        assert_eq!(editor.cursor, 1);
        editor.handle_key(key(KeyCode::Char('K')));
        assert_eq!(editor.draft.fixed_args, vec!["status", "with space"]);
        editor.handle_key(key(KeyCode::Char('d')));
        assert_eq!(editor.draft.fixed_args, vec!["with space"]);
    }

    #[test]
    fn editing_a_row_writes_only_that_argv_element() {
        let mut editor = Editor::new(Some(&sample_record()));
        editor.set_field(Field::FixedArgs);
        editor.handle_key(key(KeyCode::Char('j'))); // select "with space"
        editor.handle_key(key(KeyCode::Enter));
        for _ in 0.."with space".len() {
            editor.handle_key(key(KeyCode::Backspace));
        }
        for event in typing("--format=oneline") {
            editor.handle_key(event);
        }
        editor.handle_key(key(KeyCode::Enter));
        assert_eq!(
            editor.draft.fixed_args,
            vec!["status".to_string(), "--format=oneline".to_string()]
        );
    }

    #[test]
    fn text_fields_consume_shortcut_letters_and_ctrl_s_requests_save() {
        let mut editor = Editor::new(None);
        editor.set_field(Field::Name);
        for event in typing("qsd/中文") {
            assert_ne!(
                editor.handle_key(event),
                EditorOutcome::SaveRequested,
                "letters must never trigger global actions while typing"
            );
        }
        assert_eq!(editor.input.value(), "qsd/中文");
        assert_eq!(editor.draft.name, "qsd/中文");
        let save = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(editor.handle_key(save), EditorOutcome::SaveRequested);
        assert_eq!(editor.input.value(), "qsd/中文");
    }

    #[test]
    fn ctrl_s_commits_a_pending_list_edit_before_saving() {
        let mut editor = Editor::new(Some(&sample_record()));
        editor.set_field(Field::FixedArgs);
        editor.handle_key(key(KeyCode::Enter));
        editor
            .list_input
            .as_mut()
            .unwrap()
            .set_value("committed-on-save");
        let save = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(editor.handle_key(save), EditorOutcome::SaveRequested);
        assert_eq!(
            editor.draft.fixed_args,
            vec!["committed-on-save".to_string(), "with space".to_string()]
        );
    }

    #[test]
    fn validation_errors_map_to_fields() {
        let mut editor = Editor::new(None);
        editor.draft.name = "bad name".into();
        editor.draft.executable = "git".into();
        let error = editor.validate_now().unwrap_err();
        assert_eq!(error.field, Some(Field::Name));

        editor.draft.name = "gs".into();
        editor.draft.fixed_args = vec!["{{args}}".into()];
        editor.draft.pass_args = false;
        let error = editor.validate_now().unwrap_err();
        assert_eq!(error.field, Some(Field::FixedArgs));

        editor.draft.fixed_args.clear();
        editor.draft.pass_args = true;
        editor.draft.shells.clear();
        let error = editor.validate_now().unwrap_err();
        assert_eq!(error.field, Some(Field::Shells));

        editor.draft.shells = vec![ShellKind::Bash];
        editor.draft.executable = "   ".into();
        let error = editor.validate_now().unwrap_err();
        assert_eq!(error.field, Some(Field::Executable));
    }

    #[test]
    fn restriction_requires_a_mode_and_absolute_paths() {
        let absolute = std::env::temp_dir().to_string_lossy().into_owned();
        let mut editor = Editor::new(None);
        editor.draft.name = "rmwork".into();
        editor.draft.executable = "rm".into();
        editor.draft.restriction_paths = vec![format!(" {absolute} "), String::new()];
        let error = editor.validate_now().unwrap_err();
        assert_eq!(error.field, Some(Field::RestrictionMode));

        editor.draft.restriction_mode = Some(RestrictionMode::Whitelist);
        let record = editor.validate_now().unwrap();
        let restriction = record.restriction.unwrap();
        assert_eq!(restriction.mode, RestrictionMode::Whitelist);
        assert_eq!(restriction.paths, vec![absolute.clone()]);

        editor.draft.restriction_paths = vec!["relative".into()];
        let error = editor.validate_now().unwrap_err();
        assert_eq!(error.field, Some(Field::RestrictionPaths));
    }

    #[test]
    fn dirty_state_tracks_edits_and_reload_clears_it() {
        let record = sample_record();
        let mut editor = Editor::new(Some(&record));
        assert!(!editor.is_dirty());
        editor.set_field(Field::Description);
        editor.handle_key(key(KeyCode::Char('!')));
        assert!(editor.is_dirty());
        editor.reload_from(&record);
        assert!(!editor.is_dirty());
        assert_eq!(editor.base_revision, record.revision);
    }

    #[test]
    fn tab_order_respects_advanced_toggle() {
        let mut editor = Editor::new(None);
        let mut seen = Vec::new();
        for _ in 0..Field::BASIC.len() {
            seen.push(editor.field);
            editor.handle_key(key(KeyCode::Tab));
        }
        assert_eq!(seen, Field::BASIC.to_vec());
        assert_eq!(editor.field, Field::Name);
        editor.set_field(Field::Tags);
        editor.toggle_advanced();
        editor.next_field(false);
        assert_eq!(editor.field, Field::WorkingDirectory);
    }

    #[test]
    fn target_type_cycles_all_supported_types() {
        let mut editor = Editor::new(None);
        editor.set_field(Field::TargetType);
        let mut seen = Vec::new();
        for _ in 0..target_type_choices().len() {
            seen.push(editor.draft.target_type.clone());
            editor.handle_key(key(KeyCode::Right));
        }
        assert_eq!(seen, target_type_choices());
    }

    #[test]
    fn shell_toggle_adds_and_removes_shells() {
        let mut editor = Editor::new(None);
        editor.set_field(Field::Shells);
        assert_eq!(editor.draft.shells, vec![ShellKind::Bash]);
        editor.handle_key(key(KeyCode::Down));
        editor.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(editor.draft.shells, vec![ShellKind::Bash, ShellKind::Zsh]);
        editor.handle_key(key(KeyCode::Up));
        editor.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(editor.draft.shells, vec![ShellKind::Zsh]);
    }

    #[test]
    fn environment_values_are_masked_in_the_editor_list() {
        let editor = Editor::new(Some(&sample_record()));
        let lines = editor.env_display();
        assert_eq!(lines, vec!["TOKEN=***".to_string()]);
        assert!(!lines.join("\n").contains("hunter2"));
    }

    #[test]
    fn env_value_is_only_shown_while_explicitly_editing_it() {
        let mut editor = Editor::new(Some(&sample_record()));
        editor.set_field(Field::Environment);
        assert_eq!(editor.env_display(), vec!["TOKEN=***"]);
        editor.handle_key(key(KeyCode::Enter));
        // The active value input contains the real value for the user to edit;
        // the list renderer still masks it.
        assert_eq!(editor.list_input.as_ref().unwrap().value(), "hunter2");
        assert_eq!(editor.env_display(), vec!["TOKEN=***"]);
    }

    #[test]
    fn new_environment_entry_edits_key_then_value() {
        let mut editor = Editor::new(None);
        editor.set_field(Field::Environment);
        editor.handle_key(key(KeyCode::Char('a')));
        for event in typing("MODE") {
            editor.handle_key(event);
        }
        editor.handle_key(key(KeyCode::Enter));
        assert_eq!(editor.list_input_stage, ListInputStage::EnvValue);
        for event in typing("fast") {
            editor.handle_key(event);
        }
        editor.handle_key(key(KeyCode::Enter));
        assert_eq!(
            editor.draft.environment,
            vec![("MODE".to_string(), "fast".to_string())]
        );
    }

    #[test]
    fn empty_working_directory_becomes_none() {
        let mut editor = Editor::new(None);
        editor.draft.name = "gs".into();
        editor.draft.executable = "git".into();
        editor.draft.working_directory = "   ".into();
        assert!(editor.validate_now().unwrap().working_directory.is_none());
        editor.draft.working_directory = "/tmp/work".into();
        assert_eq!(
            editor.validate_now().unwrap().working_directory.as_deref(),
            Some("/tmp/work")
        );
    }

    #[test]
    fn actionability_ignores_key_release() {
        let mut release = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        release.kind = crossterm::event::KeyEventKind::Release;
        assert!(!is_actionable(&release));
    }
}
