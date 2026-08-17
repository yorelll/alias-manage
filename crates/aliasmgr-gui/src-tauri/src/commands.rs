use aliasmgr_core::{
    config::AppPaths,
    detection::{detect, DetectionContext},
    model::{AliasRecord, PathOrigin, PathMode, ShellKind, TargetType},
    validation::validate_alias,
    search::{SearchField, SearchQuery, SortField},
    service::AliasService,
    storage::Database,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Serializable status returned to the frontend on startup.
#[derive(Debug, Clone, Serialize)]
pub struct StartupStatus {
    /// Version string from aliasmgr-core (e.g. "0.1.0").
    pub version: String,
    /// The detected shell kind as a stable lowercase string, or null when none detected.
    pub detected_shell: Option<String>,
    /// Which detection source was used (e.g. "explicit", "parent", "shell_env",
    /// "login_shell", or "installed").
    pub detection_source: String,
    /// Resolved config directory path as a string.
    pub config_directory: String,
}

/// Convert a `ShellKind` to a stable, explicit string representation.
///
/// Uses an explicit match to guarantee stable, human-readable values that are
/// independent of any `Debug` or `Display` formatting and are stable across
/// renames or serde attribute changes.
fn shell_kind_to_str(kind: &ShellKind) -> &'static str {
    match kind {
        ShellKind::Bash => "bash",
        ShellKind::Zsh => "zsh",
        ShellKind::PowerShell5 => "powershell5",
        ShellKind::PowerShell7 => "powershell7",
        ShellKind::Fish => "fish",
        ShellKind::PosixSh => "posix_sh",
    }
}

/// Tauri command: return startup information to the frontend.
///
/// Delegates to aliasmgr-core for:
/// - version via `aliasmgr_core::version()`
/// - shell detection via `aliasmgr_core::detection::detect()`
/// - config directory via `aliasmgr_core::config::AppPaths::discover()`
#[tauri::command]
pub fn startup_status() -> StartupStatus {
    let version = aliasmgr_core::version().to_owned();

    let context = DetectionContext {
        explicit: None,
        parent_shell: None,
        shell_env: std::env::var("SHELL").ok(),
        login_shell: None,
    };
    let detection = detect(&context);
    let detected_shell = detection.selected.as_ref().map(shell_kind_to_str).map(str::to_owned);
    let detection_source = detection.source;

    let paths = AppPaths::discover(None);
    let config_directory = paths.root.to_string_lossy().into_owned();

    StartupStatus {
        version,
        detected_shell,
        detection_source,
        config_directory,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub executable: String,
    pub target_type: String,
    pub fixed_args: Vec<String>,
    pub pass_args: bool,
    pub working_directory: Option<String>,
    pub environment: BTreeMap<String, String>,
    pub shells: Vec<String>,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub fuzzy: bool,
    pub limit: usize,
    pub tag_filter: Vec<String>,
}

fn alias_to_dto(alias: &AliasRecord) -> AliasDto {
    AliasDto {
        id: alias.id.to_string(), name: alias.name.clone(), description: alias.description.clone(),
        executable: alias.executable.clone(), target_type: format!("{:?}", alias.target_type).to_lowercase(),
        fixed_args: alias.fixed_args.clone(), pass_args: alias.pass_args, working_directory: alias.working_directory.clone(),
        environment: alias.environment.clone(), shells: alias.shells.iter().map(|shell| format!("{shell:?}").to_lowercase()).collect(),
        enabled: alias.enabled, tags: alias.tags.clone(), revision: alias.revision,
    }
}

fn database() -> Result<Database, String> {
    let paths = AppPaths::discover(None);
    Database::open(paths.root.join("aliases.db")).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_aliases() -> Result<Vec<AliasDto>, String> {
    let database = database()?;
    AliasService::new(&database).list().map(|aliases| aliases.iter().map(alias_to_dto).collect()).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn search_aliases(request: SearchRequest) -> Result<Vec<AliasDto>, String> {
    let database = database()?;
    let query = SearchQuery { query: request.query, fuzzy: request.fuzzy, limit: request.limit, tag_filter: request.tag_filter, field: SearchField::All, sort: SortField::Name, descending: false };
    AliasService::new(&database).search(&query).map(|results| results.iter().map(|result| alias_to_dto(&result.alias)).collect()).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn tag_counts() -> Result<BTreeMap<String, usize>, String> {
    let database = database()?;
    AliasService::new(&database).tag_counts().map_err(|error| error.to_string())
}

fn dto_to_alias(dto: AliasDto) -> Result<AliasRecord, String> {
    let mut alias = AliasRecord::default();
    alias.id = uuid::Uuid::parse_str(&dto.id).map_err(|error| error.to_string())?;
    alias.name = dto.name;
    alias.description = dto.description;
    alias.executable = dto.executable;
    alias.fixed_args = dto.fixed_args;
    alias.pass_args = dto.pass_args;
    alias.working_directory = dto.working_directory;
    alias.environment = dto.environment;
    alias.enabled = dto.enabled;
    alias.tags = dto.tags;
    alias.revision = dto.revision;
    alias.target_type = serde_json::from_value(serde_json::Value::String(dto.target_type)).map_err(|error| error.to_string())?;
    alias.shells = dto.shells.into_iter().map(|shell| serde_json::from_value(serde_json::Value::String(shell)).map_err(|error| error.to_string())).collect::<Result<_, _>>()?;
    alias.path_mode = PathMode::Absolute;
    alias.path_origin = PathOrigin::GuiFilePicker;
    validate_alias(&alias).map_err(|error| error.to_string())?;
    Ok(alias)
}

#[tauri::command]
pub fn create_alias(alias: AliasDto) -> Result<AliasDto, String> {
    let database = database()?;
    AliasService::new(&database).create(dto_to_alias(alias)?).map(|saved| alias_to_dto(&saved)).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_alias(alias: AliasDto) -> Result<AliasDto, String> {
    let database = database()?;
    AliasService::new(&database).update(dto_to_alias(alias)?).map(|saved| alias_to_dto(&saved)).map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorFindingDto { pub shell: Option<String>, pub severity: String, pub code: String, pub message: String, pub action: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewDto { pub imported: Vec<String>, pub skipped: Vec<String>, pub warnings: Vec<String>, pub unsupported: Vec<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsDto { pub config_directory: String, pub default_shell: String, pub backup_keep: usize, pub log_keep: usize, pub allow_relative_paths: bool }

#[tauri::command]
pub fn doctor_status() -> Result<Vec<DoctorFindingDto>, String> { Ok(Vec::new()) }

#[tauri::command]
pub fn generated_preview(_shell: String) -> Result<String, String> { Ok(String::new()) }

#[tauri::command]
pub fn reload_command(shell: String) -> Result<String, String> { Ok(match shell.as_str() { "zsh" => "source generated/zsh.sh", "powershell5" => ". generated/powershell5.ps1", "powershell7" => ". generated/powershell7.ps1", _ => ". generated/bash.sh" }.into()) }

#[tauri::command]
pub fn import_preview(_file: String) -> Result<ImportPreviewDto, String> { Ok(ImportPreviewDto { imported: vec![], skipped: vec![], warnings: vec![], unsupported: vec![] }) }

#[tauri::command]
pub fn import_confirm(file: String) -> Result<ImportPreviewDto, String> { import_preview(file) }

#[tauri::command]
pub fn config_get() -> Result<SettingsDto, String> { let paths = AppPaths::discover(None); Ok(SettingsDto { config_directory: paths.root.to_string_lossy().into_owned(), default_shell: "bash".into(), backup_keep: 10, log_keep: 7, allow_relative_paths: false }) }

#[tauri::command]
pub fn config_save(settings: SettingsDto) -> Result<SettingsDto, String> { Ok(settings) }

#[tauri::command]
pub fn uninstall_preview(_purge: bool) -> Result<Vec<String>, String> { Ok(vec!["Referenced target files are protected".into()]) }

#[tauri::command]
pub fn uninstall_confirm(_purge: bool) -> Result<Vec<String>, String> { Ok(vec![]) }

#[tauri::command]
pub fn overridden_definitions() -> Result<Vec<serde_json::Value>, String> { Ok(vec![]) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_status_serializes_to_json() {
        let status = StartupStatus {
            version: "0.1.0".to_owned(),
            detected_shell: Some("bash".to_owned()),
            detection_source: "shell_env".to_owned(),
            config_directory: "/home/user/.config/alias-manager".to_owned(),
        };
        let json = serde_json::to_string(&status).expect("serialization failed");
        assert!(json.contains("\"version\":\"0.1.0\""));
        assert!(json.contains("\"detected_shell\":\"bash\""));
        assert!(json.contains("\"detection_source\":\"shell_env\""));
        assert!(json.contains("\"config_directory\""));
    }

    #[test]
    fn startup_status_null_shell_serializes() {
        let status = StartupStatus {
            version: "0.1.0".to_owned(),
            detected_shell: None,
            detection_source: "installed".to_owned(),
            config_directory: "/tmp/test".to_owned(),
        };
        let json = serde_json::to_string(&status).expect("serialization failed");
        assert!(json.contains("\"detected_shell\":null"));
    }

    #[test]
    fn shell_kind_to_str_returns_stable_values() {
        assert_eq!(shell_kind_to_str(&ShellKind::Bash), "bash");
        assert_eq!(shell_kind_to_str(&ShellKind::Zsh), "zsh");
        assert_eq!(shell_kind_to_str(&ShellKind::PowerShell5), "powershell5");
        assert_eq!(shell_kind_to_str(&ShellKind::PowerShell7), "powershell7");
        assert_eq!(shell_kind_to_str(&ShellKind::Fish), "fish");
        assert_eq!(shell_kind_to_str(&ShellKind::PosixSh), "posix_sh");
    }

    #[test]
    fn version_is_non_empty() {
        let v = aliasmgr_core::version();
        assert!(!v.is_empty(), "core version must not be empty");
    }
}
