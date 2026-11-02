use aliasmgr_core::{
    config::AppPaths,
    detection::{detect, find_definition_source, DetectionContext},
    model::{
        AliasRecord, ManagedNameSet, PathMode, PathOrigin, PathRestriction, ShellKind, TargetType,
    },
    search::{SearchField, SearchQuery, SortField},
    service::AliasService,
    shells::{bash, powershell, zsh},
    storage::Database,
    sync::{content_checksum, verify_file_checksum, SyncCoordinator},
    transfer::{self, ConflictStrategy, ImportReport},
    validation::validate_alias,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Serializable status returned to the frontend on startup.
#[derive(Debug, Clone, Serialize)]
pub struct StartupStatus {
    /// Version string from aliasmgr-core (e.g. "0.1.0").
    pub version: String,
    /// The detected shell kind as a stable lowercase string, or null when none detected.
    pub detected_shell: Option<String>,
    /// Shells found on PATH, as stable lowercase strings, in detection order.
    pub installed_shells: Vec<String>,
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
    let installed_shells = detection
        .installed
        .iter()
        .map(shell_kind_to_str)
        .map(str::to_owned)
        .collect();
    let detection_source = detection.source;

    let paths = AppPaths::discover(None);
    let config_directory = paths.root.to_string_lossy().into_owned();

    StartupStatus {
        version,
        detected_shell,
        installed_shells,
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
    #[serde(default)]
    pub restriction: Option<PathRestriction>,
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
        restriction: alias.restriction.clone(),
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
    let alias = AliasRecord {
        id: uuid::Uuid::parse_str(&dto.id).map_err(|error| error.to_string())?,
        name: dto.name,
        description: dto.description,
        executable: dto.executable,
        fixed_args: dto.fixed_args,
        pass_args: dto.pass_args,
        working_directory: dto.working_directory,
        environment: dto.environment,
        enabled: dto.enabled,
        tags: dto.tags,
        restriction: dto.restriction,
        revision: dto.revision,
        target_type: target_type_from_str(&dto.target_type)?,
        shells: dto
            .shells
            .iter()
            .map(|shell| shell_from_str(shell))
            .collect::<Result<_, _>>()?,
        path_mode: match dto.path_mode.as_deref() {
            Some(value) => path_mode_from_str(value)?,
            None => PathMode::Absolute,
        },
        path_origin: match dto.path_origin.as_deref() {
            Some(value) => path_origin_from_str(value)?,
            None => PathOrigin::GuiFilePicker,
        },
        ..AliasRecord::default()
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
        // Match the CLI: the tombstone must outrank the deleted revision so a
        // later sync renders the cleanup block for this name.
        database
            .retire_name(&alias.name, shell, alias.revision.saturating_add(1))
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

fn default_language() -> String {
    "zh".to_owned()
}

/// Normalize a persisted language value, falling back to the Chinese default.
fn normalized_language(value: &str) -> String {
    match value.trim() {
        "en" => "en".to_owned(),
        _ => "zh".to_owned(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsDto {
    pub config_directory: String,
    pub default_shell: String,
    pub backup_keep: usize,
    pub log_keep: usize,
    pub allow_relative_paths: bool,
    #[serde(default = "default_language")]
    pub language: String,
}

#[tauri::command]
pub fn doctor_status() -> Result<Vec<DoctorFindingDto>, String> {
    const KNOWN_SHELLS: [&str; 4] = ["bash", "zsh", "powershell5", "powershell7"];

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
    let aliases = database.list_aliases().map_err(|error| error.to_string())?;
    let max_alias_revision = aliases
        .iter()
        .map(|alias| alias.revision)
        .max()
        .unwrap_or(0);
    let max_retired_revision = database
        .max_retired_revision()
        .map_err(|error| error.to_string())?;
    let current_revision = max_alias_revision.max(max_retired_revision);

    // Only shells an alias asks for (or that still have tombstones) should get
    // a generated file; an untouched install must not look broken.
    let mut referenced_shells: BTreeSet<String> = BTreeSet::new();
    for alias in &aliases {
        for shell in &alias.shells {
            referenced_shells.insert(shell_kind_to_str(shell).to_owned());
        }
    }
    let mut managed_names = BTreeMap::new();
    for name in KNOWN_SHELLS {
        let Ok(kind) = shell_from_str(name) else {
            continue;
        };
        let names = database
            .managed_name_set(&kind)
            .map_err(|error| error.to_string())?;
        if !names.retired.is_empty() {
            referenced_shells.insert(name.to_owned());
        }
        managed_names.insert(name.to_owned(), names);
    }

    let coordinator = SyncCoordinator::new(&paths.root);
    let mut stale_details: BTreeMap<String, String> = BTreeMap::new();
    for name in KNOWN_SHELLS {
        if !referenced_shells.contains(name) {
            continue;
        }
        let path = paths.generated_path(name);
        if !path.exists() {
            findings.push(DoctorFindingDto {
                shell: Some(name.into()),
                severity: "warning".into(),
                code: "generated_missing".into(),
                message: format!("生成文件缺失: {name}"),
                action: Some("执行同步".into()),
                detail: None,
            });
            continue;
        }
        let content = match std::fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(content) => content,
                Err(error) => {
                    findings.push(DoctorFindingDto {
                        shell: Some(name.into()),
                        severity: "warning".into(),
                        code: "generated_unreadable".into(),
                        message: format!("生成文件编码无效: {name}"),
                        action: Some("重新同步以修复生成文件".into()),
                        detail: Some(error.to_string()),
                    });
                    continue;
                }
            },
            Err(error) => {
                findings.push(DoctorFindingDto {
                    shell: Some(name.into()),
                    severity: "warning".into(),
                    code: "generated_unreadable".into(),
                    message: format!("生成文件无法读取: {name}"),
                    action: Some("重新同步以修复生成文件".into()),
                    detail: Some(error.to_string()),
                });
                continue;
            }
        };
        // Generated files carry the checksum of their own body; a mismatch
        // means the file was edited outside Alias Manager.
        let expected = content
            .lines()
            .find_map(|line| line.strip_prefix("# file_checksum: "));
        let actual = content_checksum(&content);
        let mut modified = false;
        match expected {
            Some(expected) if verify_file_checksum(&content, expected) => {}
            Some(expected) => {
                modified = true;
                findings.push(DoctorFindingDto {
                    shell: Some(name.into()),
                    severity: "warning".into(),
                    code: "generated_modified".into(),
                    message: format!("生成文件已被手动修改: {name}"),
                    action: Some("重新同步以覆盖手动修改".into()),
                    detail: Some(format!("expected {expected}, actual {actual}")),
                });
            }
            None => {
                modified = true;
                findings.push(DoctorFindingDto {
                    shell: Some(name.into()),
                    severity: "warning".into(),
                    code: "generated_modified".into(),
                    message: format!("生成文件缺少校验和标记: {name}"),
                    action: Some("重新同步以修复生成文件".into()),
                    detail: Some(format!("expected <missing>, actual {actual}")),
                });
            }
        }
        if modified {
            continue;
        }
        // A checksum line can be self-consistent while the body is stale. The
        // fresh render (no writes) tells us what the current DB should produce.
        let Ok(kind) = shell_from_str(name) else {
            continue;
        };
        let Some(names) = managed_names.get(name) else {
            continue;
        };
        // Sync renders one shell at a time: aliases that do not target this
        // shell must be excluded, or a healthy multi-shell install looks stale.
        let shell_aliases: Vec<AliasRecord> = aliases
            .iter()
            .filter(|alias| alias.shells.contains(&kind))
            .cloned()
            .collect();
        match coordinator.render_shell(&shell_aliases, &kind, current_revision, names) {
            Ok(body) => {
                let expected_checksum = content_checksum(&body);
                if expected_checksum != actual {
                    stale_details.insert(
                        name.to_owned(),
                        format!("expected checksum {expected_checksum}, actual {actual}"),
                    );
                }
            }
            Err(error) => findings.push(DoctorFindingDto {
                shell: Some(name.into()),
                severity: "warning".into(),
                code: "render_failed".into(),
                message: format!("无法渲染预期内容: {name}"),
                action: Some("检查别名目标后重新同步".into()),
                detail: Some(error.to_string()),
            }),
        }
    }

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
            stale_details.entry(shell).or_insert_with(|| {
                format!(
                    "applied revision {}, expected {current_revision}",
                    state.applied_revision
                )
            });
        }
    }
    for (shell, detail) in stale_details {
        findings.push(DoctorFindingDto {
            shell: Some(shell.clone()),
            severity: "warning".into(),
            code: "stale".into(),
            message: format!("生成文件落后于数据库: {shell}"),
            action: Some("执行同步".into()),
            detail: Some(detail),
        });
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
    let is_toml = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"));
    let aliases = if is_toml {
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
        language: normalized_language(&config.runtime.language),
    })
}

#[tauri::command]
pub fn config_save(settings: SettingsDto) -> Result<SettingsDto, String> {
    // The config directory is resolved at startup and is read-only in the UI:
    // never create a different directory or report a save that did not happen.
    let paths = AppPaths::discover(None);
    let requested = PathBuf::from(&settings.config_directory);
    if requested != paths.root {
        return Err(format!("配置目录为只读，无法更改: {}", requested.display()));
    }
    let language = settings.language.trim();
    if language != "zh" && language != "en" {
        return Err(format!(
            "不支持的界面语言 / unsupported language: {} (expected \"zh\" or \"en\")",
            settings.language
        ));
    }
    let mut config = paths.load_config().map_err(|error| error.to_string())?;
    config.backups.rc_keep = settings.backup_keep;
    config.logs.keep_files = settings.log_keep;
    config.runtime.default_shell = Some(settings.default_shell.clone());
    config.runtime.allow_relative_paths = settings.allow_relative_paths;
    config.runtime.language = language.to_owned();
    config.save(&paths).map_err(|error| error.to_string())?;
    Ok(SettingsDto {
        config_directory: paths.root.to_string_lossy().into_owned(),
        language: language.to_owned(),
        ..settings
    })
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

/// Default PowerShell profile path used when no override is configured.
fn default_powershell_profile_path(shell: &ShellKind) -> Option<PathBuf> {
    if cfg!(windows) {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from)?;
        let folder = match shell {
            ShellKind::PowerShell5 => "WindowsPowerShell",
            ShellKind::PowerShell7 => "PowerShell",
            _ => return None,
        };
        Some(
            home.join("Documents")
                .join(folder)
                .join("Microsoft.PowerShell_profile.ps1"),
        )
    } else {
        match shell {
            // Windows PowerShell 5.1 does not exist outside Windows.
            ShellKind::PowerShell7 => std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".config").join("powershell").join("profile.ps1")),
            _ => None,
        }
    }
}

/// Candidate rc/profile files for a shell: configured path first, then the
/// platform default. Only paths are collected; nothing is read or written here.
fn override_scan_paths(
    config: &aliasmgr_core::config::AppConfig,
    root: &Path,
    shell: &ShellKind,
) -> Vec<PathBuf> {
    let candidates = match shell {
        ShellKind::Bash => vec![
            config.shells.bash_rc_path.clone(),
            Some(root.join(".bashrc")),
        ],
        ShellKind::Zsh => vec![config.shells.zsh_rc_path.clone(), Some(root.join(".zshrc"))],
        ShellKind::PowerShell5 => vec![
            config.shells.powershell5_profile_path.clone(),
            default_powershell_profile_path(shell),
        ],
        ShellKind::PowerShell7 => vec![
            config.shells.powershell7_profile_path.clone(),
            default_powershell_profile_path(shell),
        ],
        _ => Vec::new(),
    };
    let mut paths = Vec::new();
    for candidate in candidates.into_iter().flatten() {
        if !paths.contains(&candidate) {
            paths.push(candidate);
        }
    }
    paths
}

/// Tauri command: read the configured rc/profile files and record which
/// aliases are already defined there. File content is only read, never
/// executed, and only metadata is stored: the raw definition line is never
/// persisted or returned to the frontend.
#[tauri::command]
pub fn scan_overrides() -> Result<Vec<serde_json::Value>, String> {
    let paths = AppPaths::discover(None);
    let database_path = paths.root.join("aliases.db");
    if !database_path.exists() {
        // A fresh config has no aliases to scan. Returning early keeps Doctor's
        // `database_missing` finding deterministic instead of creating the DB.
        return Ok(Vec::new());
    }
    let config = paths.load_config().map_err(|error| error.to_string())?;
    let database = Database::open(&database_path).map_err(|error| error.to_string())?;
    let aliases = database.list_aliases().map_err(|error| error.to_string())?;
    let mut found: BTreeMap<(String, ShellKind), aliasmgr_core::storage::OverrideEntry> =
        BTreeMap::new();
    for alias in &aliases {
        for shell in &alias.shells {
            let candidates = override_scan_paths(&config, &paths.root, shell);
            if candidates.is_empty() {
                continue;
            }
            let Some(source) = find_definition_source(&candidates, &alias.name)
                .map_err(|error| error.to_string())?
            else {
                continue;
            };
            found.insert(
                (alias.name.clone(), shell.clone()),
                aliasmgr_core::storage::OverrideEntry {
                    name: alias.name.clone(),
                    shell: shell.clone(),
                    definition_kind: source.kind.as_str().to_owned(),
                    recoverable: source.recoverable,
                },
            );
        }
    }
    let entries: Vec<_> = found.into_values().collect();
    // Replaces every scan-produced row atomically, so a definition removed
    // from the rc file also disappears from the UI.
    database
        .replace_overrides(&entries)
        .map_err(|error| error.to_string())?;
    database
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
            restriction: None,
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
    fn preview_alias_includes_the_restriction_guard() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _config = TempConfig::new("preview-restriction");
        let mut dto = preview_dto();
        dto.restriction = Some(PathRestriction {
            mode: aliasmgr_core::model::RestrictionMode::Whitelist,
            paths: vec![std::env::temp_dir().to_string_lossy().into_owned()],
        });

        let previews = preview_alias(dto).unwrap();
        assert_eq!(previews.len(), 3);
        for preview in &previews {
            assert!(
                preview.error.is_none(),
                "{}: {:?}",
                preview.shell,
                preview.error
            );
            assert!(
                preview.content.contains("__aliasmgr_cwd"),
                "{} preview lacks the guard: {}",
                preview.shell,
                preview.content
            );
            assert!(!preview.content.contains("Set-Alias -Name gsp"));
            assert!(!preview.content.contains("alias gsp="));
        }
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
    fn doctor_reports_missing_files_only_for_referenced_shells() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("doctor-missing");
        Database::open(config.root.join("aliases.db")).unwrap();

        let findings = doctor_status().unwrap();
        assert!(
            findings
                .iter()
                .all(|finding| finding.code != "generated_missing"),
            "unreferenced shells must not be reported missing: {findings:?}"
        );
    }

    #[test]
    fn doctor_reports_unreadable_generated_files() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("doctor-unreadable");
        let database = Database::open(config.root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        drop(database);
        let generated = config.root.join("generated/bash.sh");
        std::fs::create_dir_all(generated.parent().unwrap()).unwrap();
        std::fs::write(&generated, [0xFFu8, 0xFE, 0x00, 0x80]).unwrap();

        let findings = doctor_status().unwrap();
        let unreadable = findings
            .iter()
            .find(|finding| {
                finding.code == "generated_unreadable" && finding.shell.as_deref() == Some("bash")
            })
            .expect("invalid UTF-8 must be reported instead of skipped");
        assert_eq!(unreadable.severity, "warning");
        assert!(unreadable.detail.is_some());
    }

    #[test]
    fn doctor_detects_stale_content_behind_a_valid_checksum() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("doctor-stale-content");
        let database = Database::open(config.root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        database
            .upsert_shell_state(&aliasmgr_core::model::ShellState {
                shell: ShellKind::Bash,
                applied_revision: 2,
                file_checksum: String::new(),
                loader_installed: true,
                status: aliasmgr_core::model::ShellStatus::Ok,
                last_error: None,
            })
            .unwrap();
        drop(database);

        // Self-consistent checksum, but the rendered body predates the alias.
        let body = "# Alias Manager\n# revision: 1\n# managed: \n# retired: \n";
        let checksum = content_checksum(body);
        let generated = config.root.join("generated/bash.sh");
        std::fs::create_dir_all(generated.parent().unwrap()).unwrap();
        std::fs::write(&generated, format!("{body}# file_checksum: {checksum}\n")).unwrap();

        let findings = doctor_status().unwrap();
        assert!(
            findings
                .iter()
                .all(|finding| finding.code != "generated_modified"),
            "checksum-valid file must not be reported as modified"
        );
        let stale = findings
            .iter()
            .find(|finding| finding.code == "stale" && finding.shell.as_deref() == Some("bash"))
            .expect("rendered-content mismatch must be reported as stale");
        assert_eq!(stale.severity, "warning");
        assert!(stale
            .detail
            .as_deref()
            .unwrap()
            .contains("expected checksum"));
    }

    #[test]
    fn config_save_rejects_changing_the_config_directory() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("config-save");
        let requested = config.root.join("elsewhere");
        let result = config_save(SettingsDto {
            config_directory: requested.to_string_lossy().into_owned(),
            default_shell: "bash".into(),
            backup_keep: 1,
            log_keep: 1,
            allow_relative_paths: false,
            language: "zh".into(),
        });
        assert!(result.is_err());
        assert!(
            !requested.exists(),
            "a rejected save must not create directories"
        );
        assert!(!config.root.join("config.toml").exists());
    }

    #[test]
    fn content_import_accepts_bare_arrays_and_uppercase_toml_filenames() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("import-formats");

        let bare = serde_json::to_string(&vec![AliasRecord {
            name: "bare-json".into(),
            executable: "git".into(),
            ..Default::default()
        }])
        .unwrap();
        let preview = import_preview_content(bare, "sample.json".into()).unwrap();
        assert_eq!(preview.imported, vec!["bare-json"]);

        let toml_path = config.root.join("import.TOML");
        aliasmgr_core::transfer::export_toml(
            &toml_path,
            &[AliasRecord {
                name: "toml-one".into(),
                executable: "git".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        let toml_content = std::fs::read_to_string(&toml_path).unwrap();
        let preview = import_preview_content(toml_content, "import.TOML".into()).unwrap();
        assert_eq!(preview.imported, vec!["toml-one"]);

        let file_preview = import_preview(toml_path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(file_preview.imported, vec!["toml-one"]);
    }

    #[test]
    fn scan_overrides_does_not_create_a_missing_database() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("overrides-missing-db");

        let overrides = scan_overrides().unwrap();
        assert!(overrides.is_empty());
        assert!(
            !config.root.join("aliases.db").exists(),
            "scan must not create the database on a fresh config"
        );
    }

    #[test]
    fn scan_overrides_records_metadata_and_clears_stale_rows() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("overrides");
        let paths = AppPaths {
            root: config.root.clone(),
        };
        let rc = config.root.join("bashrc.custom");
        let mut app_config = aliasmgr_core::config::AppConfig::default();
        app_config.shells.bash_rc_path = Some(rc.clone());
        app_config.shells.powershell7_profile_path = Some(rc.clone());
        app_config.save(&paths).unwrap();
        std::fs::write(
            &rc,
            "alias gs='git status'\nfunction multi {\nalias mygs='other'\nSet-Alias -Name psa -Value git -Scope Global\n",
        )
        .unwrap();
        std::fs::write(
            config.root.join(".bashrc"),
            "function deploy() { echo hi; }\n",
        )
        .unwrap();

        let database = Database::open(config.root.join("aliases.db")).unwrap();
        for (name, shells) in [
            ("gs", vec![ShellKind::Bash]),
            ("multi", vec![ShellKind::Bash]),
            ("deploy", vec![ShellKind::Bash]),
            ("psa", vec![ShellKind::PowerShell7]),
        ] {
            database
                .insert_alias(&AliasRecord {
                    name: name.into(),
                    executable: "tool".into(),
                    shells,
                    ..Default::default()
                })
                .unwrap();
        }
        drop(database);
        let rc_before = std::fs::read_to_string(&rc).unwrap();

        let overrides = scan_overrides().unwrap();
        // Repeated scans replace rows instead of appending duplicates.
        let repeated = scan_overrides().unwrap();
        assert_eq!(
            repeated.len(),
            overrides.len(),
            "repeated scans must not duplicate override rows"
        );
        // Scanning only reads the rc file: content and encoding stay untouched.
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), rc_before);

        let find = |list: &[serde_json::Value], name: &str| {
            list.iter()
                .find(|entry| entry["name"] == name)
                .unwrap_or_else(|| panic!("{name} missing from overrides"))
                .clone()
        };

        let gs = find(&overrides, "gs");
        assert_eq!(gs["shell"], "bash");
        assert_eq!(gs["definition_kind"], "alias");
        assert_eq!(gs["recoverable"], true);
        assert!(
            gs.get("original_definition").is_none(),
            "raw rc lines must not cross the IPC boundary"
        );

        let multi = find(&overrides, "multi");
        assert_eq!(multi["definition_kind"], "function");
        assert_eq!(
            multi["recoverable"], false,
            "an opening brace alone is not a complete definition"
        );

        let deploy = find(&overrides, "deploy");
        assert_eq!(deploy["definition_kind"], "function");
        assert_eq!(deploy["recoverable"], true);

        let psa = find(&overrides, "psa");
        assert_eq!(psa["shell"], "powershell7");
        assert_eq!(psa["definition_kind"], "set_alias");
        assert_eq!(psa["recoverable"], true);

        assert!(
            overrides.iter().all(|entry| entry["name"] != "mygs"),
            "a longer name must not create an override for gs"
        );
        assert!(overrides
            .iter()
            .all(|entry| entry.get("original_definition").is_none()));

        // Removing the definitions from the rc file clears their rows.
        std::fs::write(&rc, "# now empty\n").unwrap();
        let after_removal = scan_overrides().unwrap();
        assert_eq!(
            after_removal.len(),
            1,
            "only deploy remains: {after_removal:?}"
        );
        assert_eq!(after_removal[0]["name"], "deploy");
    }

    #[test]
    fn doctor_reports_nothing_for_healthy_multi_shell_install() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("doctor-healthy");
        let aliases = [
            AliasRecord {
                name: "bash-only".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                revision: 2,
                ..Default::default()
            },
            AliasRecord {
                name: "zsh-only".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Zsh],
                revision: 2,
                ..Default::default()
            },
            AliasRecord {
                name: "everywhere".into(),
                executable: "git".into(),
                shells: vec![
                    ShellKind::Bash,
                    ShellKind::Zsh,
                    ShellKind::PowerShell5,
                    ShellKind::PowerShell7,
                ],
                revision: 2,
                ..Default::default()
            },
        ];
        let coordinator = SyncCoordinator::new(&config.root);
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::PowerShell5,
            ShellKind::PowerShell7,
        ] {
            let shell_aliases: Vec<AliasRecord> = aliases
                .iter()
                .filter(|alias| alias.shells.contains(&shell))
                .cloned()
                .collect();
            coordinator
                .apply(&shell_aliases, std::slice::from_ref(&shell), 2)
                .unwrap();
        }

        let findings = doctor_status().unwrap();
        assert!(
            findings.is_empty(),
            "a healthy multi-shell install must report nothing: {findings:?}"
        );
    }

    #[test]
    fn doctor_reports_render_failures_instead_of_ignoring_them() {
        use aliasmgr_core::model::{ShellState, ShellStatus};

        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("doctor-render-failed");
        let database = Database::open(config.root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "broken".into(),
                executable: "./definitely-missing-tool".into(),
                shells: vec![ShellKind::Bash],
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        database
            .upsert_shell_state(&ShellState {
                shell: ShellKind::Bash,
                applied_revision: 2,
                file_checksum: String::new(),
                loader_installed: true,
                status: ShellStatus::Ok,
                last_error: None,
            })
            .unwrap();
        drop(database);

        // A checksum-valid file, so only the fresh render can fail.
        let body = "# Alias Manager\n# revision: 2\n";
        let checksum = content_checksum(body);
        let generated = config.root.join("generated/bash.sh");
        std::fs::create_dir_all(generated.parent().unwrap()).unwrap();
        std::fs::write(&generated, format!("{body}# file_checksum: {checksum}\n")).unwrap();

        let findings = doctor_status().unwrap();
        let failure = findings
            .iter()
            .find(|finding| {
                finding.code == "render_failed" && finding.shell.as_deref() == Some("bash")
            })
            .expect("render failure must be reported");
        assert_eq!(failure.severity, "warning");
        assert!(failure
            .detail
            .as_deref()
            .unwrap()
            .contains("target is missing"));
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
            installed_shells: vec!["bash".to_owned(), "zsh".to_owned()],
            detection_source: "shell_env".to_owned(),
            config_directory: "/home/user/.config/alias-manager".to_owned(),
        };
        let json = serde_json::to_string(&status).expect("serialization failed");
        assert!(json.contains("\"version\":\"0.1.0\""));
        assert!(json.contains("\"detected_shell\":\"bash\""));
        assert!(json.contains("\"installed_shells\":[\"bash\",\"zsh\"]"));
        assert!(json.contains("\"detection_source\":\"shell_env\""));
        assert!(json.contains("\"config_directory\""));
    }

    #[test]
    fn startup_status_null_shell_serializes() {
        let status = StartupStatus {
            version: "0.1.0".to_owned(),
            detected_shell: None,
            installed_shells: Vec::new(),
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
        let retired_revision = database.max_retired_revision().unwrap();
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
        // The tombstone outranks the deleted revision (1 -> 2), like the CLI.
        assert_eq!(retired_revision, 2);
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
            language: "en".into(),
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
        assert_eq!(saved.language, "en");
        assert_eq!(loaded.language, "en");
    }

    #[test]
    fn config_get_defaults_to_chinese_language() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _config = TempConfig::new("config-language-default");
        let loaded = config_get().unwrap();
        assert_eq!(loaded.language, "zh");
    }

    #[test]
    fn config_save_rejects_unknown_language_without_writing() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = TempConfig::new("config-language-invalid");
        let result = config_save(SettingsDto {
            config_directory: config.root.to_string_lossy().into_owned(),
            default_shell: "bash".into(),
            backup_keep: 1,
            log_keep: 1,
            allow_relative_paths: false,
            language: "fr".into(),
        });
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported language"));
        assert!(!config.root.join("config.toml").exists());
    }

    #[test]
    fn config_get_ignores_unknown_persisted_language() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _config = TempConfig::new("config-language-unknown");
        let paths = AppPaths::discover(None);
        let mut app_config = aliasmgr_core::config::AppConfig::default();
        app_config.runtime.language = "fr".into();
        app_config.save(&paths).unwrap();
        assert_eq!(config_get().unwrap().language, "zh");
    }
}
