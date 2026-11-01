use aliasmgr_core::{
    config::AppPaths,
    detection::{detect, DetectionContext},
    model::{AliasRecord, ManagedNameSet, PathMode, PathOrigin, ShellKind, TargetType},
    search::{SearchField, SearchQuery, SortField},
    service::AliasService,
    shells::{bash, powershell, zsh},
    storage::Database,
    sync::{content_checksum, verify_file_checksum},
    transfer::{self, ConflictStrategy, ImportReport},
    validation::validate_alias,
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
    let detected_shell = detection
        .selected
        .as_ref()
        .map(shell_kind_to_str)
        .map(str::to_owned);
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
    #[serde(default)]
    pub path_mode: Option<String>,
    #[serde(default)]
    pub path_origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub fuzzy: bool,
    pub limit: usize,
    pub tag_filter: Vec<String>,
}

fn target_type_to_str(target: &TargetType) -> &'static str {
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

fn target_type_from_str(value: &str) -> Result<TargetType, String> {
    match value {
        "native_executable" => Ok(TargetType::NativeExecutable),
        "python_script" => Ok(TargetType::PythonScript),
        "powershell_script" | "power_shell_script" => Ok(TargetType::PowerShellScript),
        "batch" => Ok(TargetType::Batch),
        "shell_script" => Ok(TargetType::ShellScript),
        "java_jar" => Ok(TargetType::JavaJar),
        "cmdlet" => Ok(TargetType::Cmdlet),
        "change_directory" => Ok(TargetType::ChangeDirectory),
        "raw_shell_command" => Ok(TargetType::RawShellCommand),
        other => Err(format!("unknown target type: {other}")),
    }
}

fn shell_from_str(value: &str) -> Result<ShellKind, String> {
    match value {
        "bash" => Ok(ShellKind::Bash),
        "zsh" => Ok(ShellKind::Zsh),
        "powershell5" | "power_shell5" => Ok(ShellKind::PowerShell5),
        "powershell7" | "power_shell7" => Ok(ShellKind::PowerShell7),
        "fish" => Ok(ShellKind::Fish),
        "posix_sh" => Ok(ShellKind::PosixSh),
        other => Err(format!("unknown shell: {other}")),
    }
}

fn path_mode_to_str(mode: &PathMode) -> &'static str {
    match mode {
        PathMode::Absolute => "absolute",
        PathMode::RelativeToWorkingDirectory => "relative_to_working_directory",
    }
}

fn path_mode_from_str(value: &str) -> Result<PathMode, String> {
    match value {
        "absolute" => Ok(PathMode::Absolute),
        "relative_to_working_directory" => Ok(PathMode::RelativeToWorkingDirectory),
        other => Err(format!("unknown path mode: {other}")),
    }
}

fn path_origin_to_str(origin: &PathOrigin) -> &'static str {
    match origin {
        PathOrigin::GuiFilePicker => "gui_file_picker",
        PathOrigin::CliArgument => "cli_argument",
        PathOrigin::Import => "import",
    }
}

fn path_origin_from_str(value: &str) -> Result<PathOrigin, String> {
    match value {
        "gui_file_picker" => Ok(PathOrigin::GuiFilePicker),
        "cli_argument" => Ok(PathOrigin::CliArgument),
        "import" => Ok(PathOrigin::Import),
        other => Err(format!("unknown path origin: {other}")),
    }
}

fn alias_to_dto(alias: &AliasRecord) -> AliasDto {
    AliasDto {
        id: alias.id.to_string(),
        name: alias.name.clone(),
        description: alias.description.clone(),
        executable: alias.executable.clone(),
        target_type: target_type_to_str(&alias.target_type).to_owned(),
        fixed_args: alias.fixed_args.clone(),
        pass_args: alias.pass_args,
        working_directory: alias.working_directory.clone(),
        environment: alias.environment.clone(),
        shells: alias
            .shells
            .iter()
            .map(shell_kind_to_str)
            .map(str::to_owned)
            .collect(),
        enabled: alias.enabled,
        tags: alias.tags.clone(),
        revision: alias.revision,
        path_mode: Some(path_mode_to_str(&alias.path_mode).to_owned()),
        path_origin: Some(path_origin_to_str(&alias.path_origin).to_owned()),
    }
}

fn database() -> Result<Database, String> {
    let paths = AppPaths::discover(None);
    Database::open(paths.root.join("aliases.db")).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_aliases() -> Result<Vec<AliasDto>, String> {
    let database = database()?;
    AliasService::new(&database)
        .list()
        .map(|aliases| aliases.iter().map(alias_to_dto).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn search_aliases(request: SearchRequest) -> Result<Vec<AliasDto>, String> {
    let database = database()?;
    let query = SearchQuery {
        query: request.query,
        fuzzy: request.fuzzy,
        limit: request.limit,
        tag_filter: request.tag_filter,
        field: SearchField::All,
        sort: SortField::Name,
        descending: false,
    };
    AliasService::new(&database)
        .search(&query)
        .map(|results| {
            results
                .iter()
                .map(|result| alias_to_dto(&result.alias))
                .collect()
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn tag_counts() -> Result<BTreeMap<String, usize>, String> {
    let database = database()?;
    AliasService::new(&database)
        .tag_counts()
        .map_err(|error| error.to_string())
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
    alias.target_type = target_type_from_str(&dto.target_type)?;
    alias.shells = dto
        .shells
        .iter()
        .map(|shell| shell_from_str(shell))
        .collect::<Result<_, _>>()?;
    alias.path_mode = match dto.path_mode.as_deref() {
        Some(value) => path_mode_from_str(value)?,
        None => PathMode::Absolute,
    };
    alias.path_origin = match dto.path_origin.as_deref() {
        Some(value) => path_origin_from_str(value)?,
        None => PathOrigin::GuiFilePicker,
    };
    validate_alias(&alias).map_err(|error| error.to_string())?;
    Ok(alias)
}

/// Rendered shell definition for the wizard preview. `error` is populated per
/// shell so one unsupported shell cannot hide the others.
#[derive(Debug, Clone, Serialize)]
pub struct ShellPreviewDto {
    pub shell: String,
    pub content: String,
    pub error: Option<String>,
}

fn render_shell_preview(alias: &AliasRecord, shell: &ShellKind) -> Result<String, String> {
    // Preview never has tombstones to clean up: the alias is not persisted yet.
    let names = ManagedNameSet {
        current: Vec::new(),
        retired: Vec::new(),
    };
    match shell {
        ShellKind::Bash => bash::render(alias, &names),
        ShellKind::Zsh => zsh::render(alias, &names),
        ShellKind::PowerShell5 | ShellKind::PowerShell7 => {
            powershell::render(alias, shell.clone(), &names)
        }
        other => return Err(format!("unsupported shell: {}", shell_kind_to_str(other))),
    }
    .map_err(|error| error.to_string())
}

/// Tauri command: render an unsaved alias for each requested shell.
///
/// `dto_to_alias` applies the same validation as create/update, but nothing is
/// inserted and no generated file or database is touched.
#[tauri::command]
pub fn preview_alias(alias: AliasDto) -> Result<Vec<ShellPreviewDto>, String> {
    let record = dto_to_alias(alias)?;
    Ok(record
        .shells
        .iter()
        .map(|shell| match render_shell_preview(&record, shell) {
            Ok(content) => ShellPreviewDto {
                shell: shell_kind_to_str(shell).to_owned(),
                content,
                error: None,
            },
            Err(error) => ShellPreviewDto {
                shell: shell_kind_to_str(shell).to_owned(),
                content: String::new(),
                error: Some(error),
            },
        })
        .collect())
}

#[tauri::command]
pub fn create_alias(mut alias: AliasDto) -> Result<AliasDto, String> {
    let database = database()?;
    alias.id = uuid::Uuid::new_v4().to_string();
    AliasService::new(&database)
        .create(dto_to_alias(alias)?)
        .map(|saved| alias_to_dto(&saved))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_alias(alias: AliasDto) -> Result<AliasDto, String> {
    let database = database()?;
    AliasService::new(&database)
        .update(dto_to_alias(alias)?)
        .map(|saved| alias_to_dto(&saved))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_alias(id: String) -> Result<bool, String> {
    let parsed = uuid::Uuid::parse_str(&id).map_err(|error| error.to_string())?;
    let database = database()?;
    let Some(alias) = database
        .get_alias(parsed)
        .map_err(|error| error.to_string())?
    else {
        return Ok(false);
    };
    for shell in &alias.shells {
        database
            .retire_name(&alias.name, shell, alias.revision)
            .map_err(|error| error.to_string())?;
    }
    database
        .delete_alias(parsed)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorFindingDto {
    pub shell: Option<String>,
    pub severity: String,
    pub code: String,
    pub message: String,
    pub action: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewDto {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
    pub unsupported: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsDto {
    pub config_directory: String,
    pub default_shell: String,
    pub backup_keep: usize,
    pub log_keep: usize,
    pub allow_relative_paths: bool,
}

#[tauri::command]
pub fn doctor_status() -> Result<Vec<DoctorFindingDto>, String> {
    let paths = AppPaths::discover(None);
    let mut findings = Vec::new();
    // Check existence before opening: `Database::open` would create the file.
    if !paths.root.join("aliases.db").exists() {
        findings.push(DoctorFindingDto {
            shell: None,
            severity: "error".into(),
            code: "database_missing".into(),
            message: "数据库文件缺失".into(),
            action: Some("创建或选择配置目录".into()),
            detail: None,
        });
    }
    let database = database()?;
    for shell in ["bash", "zsh", "powershell5", "powershell7"] {
        let path = paths.generated_path(shell);
        if !path.exists() {
            findings.push(DoctorFindingDto {
                shell: Some(shell.into()),
                severity: "warning".into(),
                code: "generated_missing".into(),
                message: format!("生成文件缺失: {shell}"),
                action: Some("执行同步".into()),
                detail: None,
            });
            continue;
        }
        // Generated files carry the checksum of their own body; a mismatch
        // means the file was edited outside Alias Manager.
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let expected = content
            .lines()
            .find_map(|line| line.strip_prefix("# file_checksum: "));
        let actual = content_checksum(&content);
        match expected {
            Some(expected) if verify_file_checksum(&content, expected) => {}
            Some(expected) => findings.push(DoctorFindingDto {
                shell: Some(shell.into()),
                severity: "warning".into(),
                code: "generated_modified".into(),
                message: format!("生成文件已被手动修改: {shell}"),
                action: Some("重新同步以覆盖手动修改".into()),
                detail: Some(format!("expected {expected}, actual {actual}")),
            }),
            None => findings.push(DoctorFindingDto {
                shell: Some(shell.into()),
                severity: "warning".into(),
                code: "generated_modified".into(),
                message: format!("生成文件缺少校验和标记: {shell}"),
                action: Some("重新同步以修复生成文件".into()),
                detail: Some(format!("expected <missing>, actual {actual}")),
            }),
        }
    }
    let max_alias_revision = database
        .list_aliases()
        .map_err(|error| error.to_string())?
        .iter()
        .map(|alias| alias.revision)
        .max()
        .unwrap_or(0);
    let max_retired_revision = database
        .max_retired_revision()
        .map_err(|error| error.to_string())?;
    let current_revision = max_alias_revision.max(max_retired_revision);
    for state in database.shell_states().map_err(|error| error.to_string())? {
        let shell = shell_kind_to_str(&state.shell).to_owned();
        if let Some(error) = &state.last_error {
            findings.push(DoctorFindingDto {
                shell: Some(shell.clone()),
                severity: "error".into(),
                code: "sync_failed".into(),
                message: format!("同步失败: {shell}"),
                action: Some("查看错误详情后重新同步".into()),
                detail: Some(error.clone()),
            });
        }
        if !state.loader_installed {
            findings.push(DoctorFindingDto {
                shell: Some(shell.clone()),
                severity: "warning".into(),
                code: "loader_missing".into(),
                message: format!("加载器未安装: {shell}"),
                action: Some("重新安装 Shell 集成".into()),
                detail: None,
            });
        }
        if state.applied_revision < current_revision {
            findings.push(DoctorFindingDto {
                shell: Some(shell.clone()),
                severity: "warning".into(),
                code: "stale".into(),
                message: format!("生成文件落后于数据库: {shell}"),
                action: Some("执行同步".into()),
                detail: Some(format!(
                    "applied revision {}, expected {current_revision}",
                    state.applied_revision
                )),
            });
        }
    }
    if let Some(error) = database.reliability_error() {
        findings.push(DoctorFindingDto {
            shell: None,
            severity: "warning".into(),
            code: "unreliable_filesystem".into(),
            message: error.to_string(),
            action: Some("改用本地配置目录".into()),
            detail: None,
        });
    }
    Ok(findings)
}

#[tauri::command]
pub fn generated_preview(shell: String) -> Result<String, String> {
    let paths = AppPaths::discover(None);
    std::fs::read_to_string(paths.generated_path(&shell)).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reload_command(shell: String) -> Result<String, String> {
    let paths = AppPaths::discover(None);
    let path = paths.generated_path(&shell);
    Ok(match shell.as_str() {
        "zsh" => format!("source '{}'", path.display()),
        "powershell5" | "powershell7" => format!(". '{}'", path.display()),
        _ => format!(". '{}'", path.display()),
    })
}

fn existing_alias_map(database: &Database) -> Result<BTreeMap<String, AliasRecord>, String> {
    database
        .list_aliases()
        .map_err(|error| error.to_string())
        .map(|aliases| {
            aliases
                .into_iter()
                .map(|alias| (alias.name.clone(), alias))
                .collect()
        })
}

fn import_path(path: &std::path::Path) -> Result<ImportReport, String> {
    let database = database()?;
    let existing = existing_alias_map(&database)?;
    let aliases = if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
        transfer::read_toml(path)
    } else {
        transfer::read_json(path)
    }
    .map_err(|error| error.to_string())?;
    transfer::import_records(aliases, &existing, ConflictStrategy::Ask)
        .map_err(|error| error.to_string())
}

/// Resolve the in-memory export format from the caller's filename. Any
/// non-`.toml` name is treated as JSON, matching the file-based importer.
fn content_import_records(content: &str, filename: &str) -> Result<Vec<AliasRecord>, String> {
    let is_toml = std::path::Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"));
    let records = if is_toml {
        transfer::read_toml_str(content)
    } else {
        transfer::read_json_str(content)
    };
    records.map_err(|error| error.to_string())
}

fn evaluate_content_import(
    content: &str,
    filename: &str,
    existing: &BTreeMap<String, AliasRecord>,
) -> Result<ImportReport, String> {
    transfer::import_records(
        content_import_records(content, filename)?,
        existing,
        ConflictStrategy::Ask,
    )
    .map_err(|error| error.to_string())
}

/// Tauri command: validate pasted/dropped import content without persisting it.
///
/// The existing-alias set is read through a read-only connection and a missing
/// database is treated as empty, so previewing never creates or writes files.
#[tauri::command]
pub fn import_preview_content(
    content: String,
    filename: String,
) -> Result<ImportPreviewDto, String> {
    let paths = AppPaths::discover(None);
    let database_path = paths.root.join("aliases.db");
    let existing = if database_path.exists() {
        let database =
            Database::open_read_only(&database_path).map_err(|error| error.to_string())?;
        existing_alias_map(&database)?
    } else {
        BTreeMap::new()
    };
    evaluate_content_import(&content, &filename, &existing)
        .map(|report| import_dto(report.into_public()))
}

/// Tauri command: evaluate and atomically persist pasted/dropped import content.
#[tauri::command]
pub fn import_confirm_content(
    content: String,
    filename: String,
) -> Result<ImportPreviewDto, String> {
    let database = database()?;
    let existing = existing_alias_map(&database)?;
    let report = evaluate_content_import(&content, &filename, &existing)?;
    database
        .insert_aliases_atomic(&report.accepted_records)
        .map_err(|error| error.to_string())?;
    Ok(import_dto(report.into_public()))
}

fn import_dto(report: aliasmgr_core::transfer::ImportReport) -> ImportPreviewDto {
    ImportPreviewDto {
        imported: report.imported,
        skipped: report.skipped,
        warnings: report.warnings,
        unsupported: report.unsupported,
    }
}

#[tauri::command]
pub fn import_preview(file: String) -> Result<ImportPreviewDto, String> {
    import_path(std::path::Path::new(&file)).map(|report| import_dto(report.into_public()))
}

#[tauri::command]
pub fn import_confirm(file: String) -> Result<ImportPreviewDto, String> {
    let report = import_path(std::path::Path::new(&file))?;
    let database = database()?;
    database
        .insert_aliases_atomic(&report.accepted_records)
        .map_err(|error| error.to_string())?;
    Ok(import_dto(report.into_public()))
}

#[tauri::command]
pub fn config_get() -> Result<SettingsDto, String> {
    let paths = AppPaths::discover(None);
    let config = paths.load_config().map_err(|error| error.to_string())?;
    Ok(SettingsDto {
        config_directory: paths.root.to_string_lossy().into_owned(),
        default_shell: config
            .runtime
            .default_shell
            .clone()
            .unwrap_or_else(|| "bash".into()),
        backup_keep: config.backups.rc_keep,
        log_keep: config.logs.keep_files,
        allow_relative_paths: config.runtime.allow_relative_paths,
    })
}

#[tauri::command]
pub fn config_save(settings: SettingsDto) -> Result<SettingsDto, String> {
    let paths = AppPaths {
        root: std::path::PathBuf::from(&settings.config_directory),
    };
    let mut config = paths.load_config().map_err(|error| error.to_string())?;
    config.backups.rc_keep = settings.backup_keep;
    config.logs.keep_files = settings.log_keep;
    config.runtime.default_shell = Some(settings.default_shell.clone());
    config.runtime.allow_relative_paths = settings.allow_relative_paths;
    config.save(&paths).map_err(|error| error.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn uninstall_preview(purge: bool) -> Result<Vec<String>, String> {
    Ok(if purge {
        vec!["将删除 Alias Manager 生成文件、数据库和标记加载块；引用目标文件保持不变".into()]
    } else {
        vec!["将移除加载块并保留生成别名；引用目标文件保持不变".into()]
    })
}

#[tauri::command]
pub fn uninstall_confirm(purge: bool) -> Result<Vec<String>, String> {
    use aliasmgr_core::shells::common::{remove_loader, write_profile_content};

    let paths = AppPaths::discover(None);
    let config = paths.load_config().unwrap_or_default();
    let mut loaders: Vec<std::path::PathBuf> = [
        config.shells.bash_rc_path,
        config.shells.zsh_rc_path,
        config.shells.powershell5_profile_path,
        config.shells.powershell7_profile_path,
    ]
    .into_iter()
    .flatten()
    .collect();
    loaders.push(paths.root.join(".bashrc"));
    loaders.push(paths.root.join(".zshrc"));
    loaders.sort();
    loaders.dedup();

    // Remove loader blocks before any destructive cleanup so a loader error
    // aborts while the generated files and database are still intact.
    for loader in loaders.iter().filter(|path| path.exists()) {
        let content = std::fs::read_to_string(loader).map_err(|error| error.to_string())?;
        let cleaned = remove_loader(&content).map_err(|error| error.to_string())?;
        if cleaned != content {
            write_profile_content(loader, &cleaned).map_err(|error| error.to_string())?;
        }
    }

    let mode = if purge {
        aliasmgr_core::uninstall::UninstallMode::PurgeAliases
    } else {
        aliasmgr_core::uninstall::UninstallMode::RetainAliases
    };
    aliasmgr_core::uninstall::uninstall(&paths.root, mode, None)
        .map_err(|error| error.to_string())?;
    Ok(vec!["卸载清理已完成；引用目标文件未处理".into()])
}

#[tauri::command]
pub fn overridden_definitions() -> Result<Vec<serde_json::Value>, String> {
    database()?
        .overridden_definitions()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that mutate the process-wide `ALIASMGR_CONFIG_DIR`.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Owns a temporary config directory and the process-wide
    /// `ALIASMGR_CONFIG_DIR` override for the duration of a test.
    struct TempConfig {
        root: std::path::PathBuf,
        previous: Option<std::ffi::OsString>,
    }

    impl TempConfig {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "aliasmgr-gui-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_nanos())
                    .unwrap_or_default()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let previous = std::env::var_os("ALIASMGR_CONFIG_DIR");
            std::env::set_var("ALIASMGR_CONFIG_DIR", &root);
            Self { root, previous }
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var("ALIASMGR_CONFIG_DIR", value),
                None => std::env::remove_var("ALIASMGR_CONFIG_DIR"),
            }
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn export_content(aliases: &[AliasRecord]) -> String {
        serde_json::to_string(&aliasmgr_core::transfer::ExportFile {
            format_version: aliasmgr_core::transfer::FORMAT_VERSION,
            exported_at: "2026-11-01T00:00:00Z".into(),
            exported_by_version: "0.1.0".into(),
            aliases: aliases.to_vec(),
        })
        .unwrap()
    }

    fn preview_dto() -> AliasDto {
        AliasDto {
            id: uuid::Uuid::new_v4().to_string(),
            name: "gsp".into(),
            description: String::new(),
            executable: "git".into(),
            target_type: "native_executable".into(),
            fixed_args: vec!["status".into()],
            pass_args: true,
            working_directory: None,
            environment: BTreeMap::new(),
            shells: vec!["bash".into(), "zsh".into(), "powershell7".into()],
            enabled: true,
            tags: Vec::new(),
            revision: 1,
            path_mode: Some("absolute".into()),
            path_origin: Some("gui_file_picker".into()),
        }
    }

    #[test]
    fn preview_alias_renders_each_shell_and_writes_nothing() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("preview");

        let previews = preview_alias(preview_dto()).unwrap();
        assert_eq!(previews.len(), 3);
        assert_eq!(
            previews
                .iter()
                .map(|preview| preview.shell.as_str())
                .collect::<Vec<_>>(),
            vec!["bash", "zsh", "powershell7"]
        );
        assert!(previews.iter().all(|preview| preview.error.is_none()));
        assert!(previews
            .iter()
            .all(|preview| preview.content.contains("gsp")));

        let mut invalid = preview_dto();
        invalid.name = "invalid name".into();
        assert!(preview_alias(invalid).is_err());

        // Preview is pure: no database and no generated directory appear.
        assert!(!config.root.join("aliases.db").exists());
        assert!(!config.root.join("generated").exists());
        assert_eq!(std::fs::read_dir(&config.root).unwrap().count(), 0);
    }

    #[test]
    fn content_import_preview_does_not_persist_or_create_the_database() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("import-preview");
        let content = export_content(&[AliasRecord {
            name: "gui-import".into(),
            executable: "Write-Output imported".into(),
            shells: vec![ShellKind::PowerShell5],
            ..Default::default()
        }]);

        let preview = import_preview_content(content.clone(), "gui-import.json".into()).unwrap();
        assert_eq!(preview.imported, vec!["gui-import"]);
        assert!(!config.root.join("aliases.db").exists());

        let database = Database::open(config.root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "existing".into(),
                executable: "tool".into(),
                ..Default::default()
            })
            .unwrap();
        drop(database);

        let preview = import_preview_content(content, "gui-import.json".into()).unwrap();
        assert_eq!(preview.imported, vec!["gui-import"]);
        let database = Database::open(config.root.join("aliases.db")).unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 1);
    }

    #[test]
    fn content_import_confirm_persists_valid_records_and_skips_invalid() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("import-confirm");
        let content = export_content(&[
            AliasRecord {
                name: "gui-import".into(),
                executable: "Write-Output imported".into(),
                shells: vec![ShellKind::PowerShell5],
                ..Default::default()
            },
            AliasRecord {
                name: "INVALID NAME WITH SPACES".into(),
                executable: "bad".into(),
                shells: vec![ShellKind::PowerShell5],
                ..Default::default()
            },
        ]);

        let report = import_confirm_content(content, "gui-import.json".into()).unwrap();
        assert_eq!(report.imported, vec!["gui-import"]);
        assert_eq!(report.skipped, vec!["INVALID NAME WITH SPACES"]);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("invalid alias")));

        let database = Database::open(config.root.join("aliases.db")).unwrap();
        let names: Vec<_> = database
            .list_aliases()
            .unwrap()
            .into_iter()
            .map(|alias| alias.name)
            .collect();
        assert_eq!(names, vec!["gui-import"]);
    }

    #[test]
    fn doctor_detects_tampered_generated_file_and_stale_state() {
        use aliasmgr_core::model::{ShellState, ShellStatus};

        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("doctor");
        let database = Database::open(config.root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                revision: 3,
                ..Default::default()
            })
            .unwrap();
        database
            .upsert_shell_state(&ShellState {
                shell: ShellKind::Bash,
                applied_revision: 1,
                file_checksum: String::new(),
                loader_installed: true,
                status: ShellStatus::Ok,
                last_error: None,
            })
            .unwrap();
        drop(database);

        let body = "# Alias Manager\n# revision: 1\nalias gs='git'\n";
        let expected = aliasmgr_core::sync::content_checksum(body);
        let tampered = body.replace("alias gs='git'", "alias gs='rm'");
        let generated = config.root.join("generated/bash.sh");
        std::fs::create_dir_all(generated.parent().unwrap()).unwrap();
        std::fs::write(
            &generated,
            format!("{tampered}# file_checksum: {expected}\n"),
        )
        .unwrap();

        let findings = doctor_status().unwrap();
        let modified = findings
            .iter()
            .find(|finding| {
                finding.code == "generated_modified" && finding.shell.as_deref() == Some("bash")
            })
            .expect("tampered generated file must be reported");
        assert_eq!(modified.severity, "warning");
        let detail = modified.detail.as_deref().unwrap();
        assert!(detail.contains(&expected));
        assert!(detail.contains(&aliasmgr_core::sync::content_checksum(&tampered)));

        let stale = findings
            .iter()
            .find(|finding| finding.code == "stale" && finding.shell.as_deref() == Some("bash"))
            .expect("older applied revision must be reported as stale");
        assert_eq!(stale.severity, "warning");
        assert!(stale
            .detail
            .as_deref()
            .unwrap()
            .contains("applied revision 1"));
    }

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

    #[test]
    fn target_type_names_round_trip_for_all_variants() {
        for target in [
            TargetType::NativeExecutable,
            TargetType::PythonScript,
            TargetType::PowerShellScript,
            TargetType::Batch,
            TargetType::ShellScript,
            TargetType::JavaJar,
            TargetType::Cmdlet,
            TargetType::ChangeDirectory,
            TargetType::RawShellCommand,
        ] {
            assert_eq!(
                target_type_from_str(target_type_to_str(&target)).unwrap(),
                target
            );
        }
        assert_eq!(
            target_type_to_str(&TargetType::PowerShellScript),
            "powershell_script"
        );
    }

    #[test]
    fn shell_names_round_trip_for_all_variants() {
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::PowerShell5,
            ShellKind::PowerShell7,
            ShellKind::Fish,
            ShellKind::PosixSh,
        ] {
            assert_eq!(shell_from_str(shell_kind_to_str(&shell)).unwrap(), shell);
        }
        assert_eq!(shell_kind_to_str(&ShellKind::PowerShell5), "powershell5");
        assert_eq!(shell_kind_to_str(&ShellKind::PowerShell7), "powershell7");
    }

    #[test]
    fn alias_dto_round_trip_preserves_target_type_and_shells() {
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            target_type: TargetType::PowerShellScript,
            shells: vec![
                ShellKind::Bash,
                ShellKind::Zsh,
                ShellKind::PowerShell5,
                ShellKind::PowerShell7,
                ShellKind::Fish,
                ShellKind::PosixSh,
            ],
            ..Default::default()
        };
        let dto = alias_to_dto(&alias);
        assert_eq!(dto.target_type, "powershell_script");
        assert_eq!(
            dto.shells,
            vec![
                "bash",
                "zsh",
                "powershell5",
                "powershell7",
                "fish",
                "posix_sh"
            ]
        );
        let restored = dto_to_alias(dto).unwrap();
        assert_eq!(restored.target_type, alias.target_type);
        assert_eq!(restored.shells, alias.shells);
    }

    #[test]
    fn legacy_serde_spellings_are_accepted() {
        let mut dto = alias_to_dto(&AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        });
        dto.target_type = "power_shell_script".into();
        dto.shells = vec!["power_shell5".into(), "power_shell7".into()];
        let alias = dto_to_alias(dto).unwrap();
        assert_eq!(alias.target_type, TargetType::PowerShellScript);
        assert_eq!(
            alias.shells,
            vec![ShellKind::PowerShell5, ShellKind::PowerShell7]
        );
    }

    #[test]
    fn unknown_enum_names_are_rejected() {
        assert!(target_type_from_str("nope").is_err());
        assert!(shell_from_str("nope").is_err());
    }

    #[test]
    fn alias_dto_round_trip_preserves_path_mode_and_origin() {
        let alias = AliasRecord {
            name: "rel".into(),
            executable: "./tool".into(),
            path_mode: PathMode::RelativeToWorkingDirectory,
            path_origin: PathOrigin::Import,
            ..Default::default()
        };
        let dto = alias_to_dto(&alias);
        assert_eq!(
            dto.path_mode.as_deref(),
            Some("relative_to_working_directory")
        );
        assert_eq!(dto.path_origin.as_deref(), Some("import"));
        let restored = dto_to_alias(dto).unwrap();
        assert_eq!(restored.path_mode, PathMode::RelativeToWorkingDirectory);
        assert_eq!(restored.path_origin, PathOrigin::Import);
    }

    #[test]
    fn missing_path_fields_default_to_absolute_gui_picker() {
        let mut dto = alias_to_dto(&AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        });
        dto.path_mode = None;
        dto.path_origin = None;
        let restored = dto_to_alias(dto).unwrap();
        assert_eq!(restored.path_mode, PathMode::Absolute);
        assert_eq!(restored.path_origin, PathOrigin::GuiFilePicker);
    }

    #[test]
    fn unknown_path_values_are_rejected() {
        let mut dto = alias_to_dto(&AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        });
        dto.path_mode = Some("sideways".into());
        assert!(dto_to_alias(dto.clone()).is_err());
        dto.path_mode = Some("absolute".into());
        dto.path_origin = Some("magic".into());
        assert!(dto_to_alias(dto).is_err());
        assert!(path_mode_from_str("nope").is_err());
        assert!(path_origin_from_str("nope").is_err());
    }

    #[test]
    fn delete_alias_retires_name_before_removing_the_record() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = std::env::temp_dir().join(format!(
            "aliasmgr-gui-delete-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let previous = std::env::var_os("ALIASMGR_CONFIG_DIR");
        std::env::set_var("ALIASMGR_CONFIG_DIR", &temp);

        let database = database().unwrap();
        let alias = AliasService::new(&database)
            .create(AliasRecord {
                name: "gsdel".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap();
        drop(database);
        let id = alias.id.to_string();

        let deleted = delete_alias(id.clone()).unwrap();
        let database = Database::open(temp.join("aliases.db")).unwrap();
        let remaining = database
            .get_alias(uuid::Uuid::parse_str(&id).unwrap())
            .unwrap();
        let bash_names = database.managed_name_set(&ShellKind::Bash).unwrap();
        let zsh_names = database.managed_name_set(&ShellKind::Zsh).unwrap();
        drop(database);

        // Restore the process-wide environment and temp directory before asserting.
        match previous {
            Some(value) => std::env::set_var("ALIASMGR_CONFIG_DIR", value),
            None => std::env::remove_var("ALIASMGR_CONFIG_DIR"),
        }
        let _ = std::fs::remove_dir_all(&temp);

        assert!(deleted);
        assert!(remaining.is_none());
        assert!(bash_names.retired.contains(&"gsdel".to_string()));
        assert!(zsh_names.retired.contains(&"gsdel".to_string()));
    }

    #[test]
    fn config_save_then_config_get_round_trips_runtime_settings() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = std::env::temp_dir().join(format!(
            "aliasmgr-gui-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let previous = std::env::var_os("ALIASMGR_CONFIG_DIR");
        std::env::set_var("ALIASMGR_CONFIG_DIR", &temp);
        let saved = config_save(SettingsDto {
            config_directory: temp.to_string_lossy().into_owned(),
            default_shell: "zsh".into(),
            backup_keep: 3,
            log_keep: 4,
            allow_relative_paths: true,
        });
        let loaded = config_get();
        match previous {
            Some(value) => std::env::set_var("ALIASMGR_CONFIG_DIR", value),
            None => std::env::remove_var("ALIASMGR_CONFIG_DIR"),
        }
        let _ = std::fs::remove_dir_all(&temp);
        let saved = saved.unwrap();
        let loaded = loaded.unwrap();
        assert_eq!(saved.default_shell, "zsh");
        assert_eq!(loaded.default_shell, "zsh");
        assert!(loaded.allow_relative_paths);
        assert_eq!(loaded.backup_keep, 3);
        assert_eq!(loaded.log_keep, 4);
    }
}
