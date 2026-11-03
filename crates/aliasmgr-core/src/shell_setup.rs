//! Shared shell-integration setup used by the CLI, TUI and GUI.
//!
//! The module owns the small amount of policy that used to be duplicated at
//! each front end: which shells the user enabled (`shells.enabled`), how a
//! detected executable maps to a loader target, and what a confirmed setup
//! action does. Path resolution, marker handling, backups, atomic writes, the
//! sync lock and the operation journal all stay in `integration` and `sync`.

use crate::{
    config::{AppConfig, AppPaths},
    detection::{self, ShellDetectionSource, ShellInstallation},
    error::AliasError,
    integration::{self, LoaderOutcome, LoaderTarget},
    model::{ShellKind, SUPPORTED_SHELLS},
    sync::{self, SyncRun},
};
use std::path::{Path, PathBuf};

/// The user's enabled intent, parsed from the stable spellings stored in
/// `config.toml`. Unknown or outdated spellings are ignored on read (loading a
/// configuration is permissive); `AppConfig::validate` rejects them on save.
pub fn enabled_shells(config: &AppConfig) -> Vec<ShellKind> {
    SUPPORTED_SHELLS
        .iter()
        .filter(|kind| {
            config
                .shells
                .enabled
                .iter()
                .any(|value| value.trim() == kind.as_str())
        })
        .cloned()
        .collect()
}

/// Replaces `shells.enabled` with the requested shells and saves the shared
/// configuration atomically. Does not install loaders or sync; callers that
/// want the full setup action use [`apply_shell_setup`]. Never enables a shell
/// that was not explicitly requested.
pub fn set_enabled_shells(paths: &AppPaths, shells: &[ShellKind]) -> Result<AppConfig, AliasError> {
    for kind in shells {
        if !SUPPORTED_SHELLS.contains(kind) {
            return Err(AliasError::Config(format!(
                "unsupported shell for integration: {}",
                kind.as_str()
            )));
        }
    }
    let mut config = paths.load_config()?;
    config.shells.enabled = SUPPORTED_SHELLS
        .iter()
        .filter(|kind| shells.contains(kind))
        .map(|kind| kind.as_str().to_string())
        .collect();
    config.validate()?;
    config.save(paths)?;
    Ok(config)
}

/// Loader status for one shell. `Unavailable` means this platform/config has
/// no profile target yet (for example PowerShell 5.1 on Linux), which the UI
/// must resolve by asking for a profile path rather than by installing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderStatus {
    Installed,
    Missing,
    Unavailable,
    Error { code: String, detail: String },
}

impl LoaderStatus {
    pub fn is_installed(&self) -> bool {
        matches!(self, LoaderStatus::Installed)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LoaderStatus::Installed => "installed",
            LoaderStatus::Missing => "missing",
            LoaderStatus::Unavailable => "unavailable",
            LoaderStatus::Error { .. } => "error",
        }
    }
}

/// The user-visible next step for a shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupAction {
    /// Detected and already integrated; confirming only persists user intent.
    Enable,
    /// Detected; install the loader block and sync after confirmation.
    InstallLoader,
    /// Not detected, or no profile target exists yet; ask for executable and
    /// profile/RC paths.
    NeedsPath,
    /// Windows WSL bash. Its profile lives inside the distribution, so Alias
    /// Manager must not write a Windows profile for it.
    WslManual,
}

impl SetupAction {
    pub fn as_str(self) -> &'static str {
        match self {
            SetupAction::Enable => "enable",
            SetupAction::InstallLoader => "install_loader",
            SetupAction::NeedsPath => "needs_path",
            SetupAction::WslManual => "wsl_manual",
        }
    }
}

/// Read-only plan for one shell. Building a plan never writes configuration,
/// profiles or generated files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSetupPlan {
    pub kind: ShellKind,
    pub detected: bool,
    pub executable: Option<PathBuf>,
    pub source: ShellDetectionSource,
    pub wsl: bool,
    pub enabled: bool,
    pub loader_target: Option<PathBuf>,
    pub loader_status: LoaderStatus,
    pub action: SetupAction,
}

/// Preview of the marked loader block and its target path. Returned before a
/// confirmed install so the user can see exactly which file would change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderPreview {
    pub target: PathBuf,
    pub powershell: bool,
    pub block: String,
}

/// Options for a confirmed setup action. `sync` defaults to false; the CLI
/// doctor passes `true` because "enable" means "ready in the next shell".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellSetupOptions {
    pub executable: Option<PathBuf>,
    pub profile: Option<PathBuf>,
    pub sync: bool,
}

impl ShellSetupOptions {
    pub fn syncing() -> Self {
        Self {
            sync: true,
            ..Self::default()
        }
    }
}

/// Per-step outcome of a confirmed setup action. Loader and sync failures are
/// reported here after the configuration was saved, so a caller can show the
/// exact failing step instead of claiming the integration is healthy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSetupOutcome {
    pub kind: ShellKind,
    pub enabled: bool,
    pub executable: Option<PathBuf>,
    pub loader_target: Option<PathBuf>,
    pub loader_changed: bool,
    pub loader_installed: bool,
    pub loader_error: Option<String>,
    pub loader_error_code: Option<String>,
    pub sync: Option<SyncRun>,
    pub sync_error: Option<String>,
    pub warnings: Vec<String>,
}

impl ShellSetupOutcome {
    pub fn has_errors(&self) -> bool {
        self.loader_error.is_some()
            || self.sync_error.is_some()
            || self.sync.as_ref().map(SyncRun::failed).unwrap_or(false)
    }
}

fn normalize_override(label: &str, path: &Path) -> Result<PathBuf, AliasError> {
    if path.as_os_str().is_empty() {
        return Err(AliasError::Config(format!("{label} must not be empty")));
    }
    let expanded = crate::config::expand_home_path(path);
    if !expanded.is_absolute() {
        return Err(AliasError::Config(format!(
            "{label} must be absolute; use ~/... to expand the home directory (got {})",
            path.display()
        )));
    }
    Ok(expanded)
}

fn apply_overrides(
    config: &mut AppConfig,
    kind: &ShellKind,
    executable: Option<&Path>,
    profile: Option<&Path>,
) -> Result<(), AliasError> {
    if let Some(path) = executable {
        let path = normalize_override(&format!("executable path for {}", kind.as_str()), path)?;
        config
            .shells
            .executables
            .insert(kind.as_str().to_string(), path);
    }
    if let Some(path) = profile {
        let path = normalize_override(&format!("profile path for {}", kind.as_str()), path)?;
        let slot = match kind {
            ShellKind::Bash => &mut config.shells.bash_rc_path,
            ShellKind::Zsh => &mut config.shells.zsh_rc_path,
            ShellKind::PowerShell5 => &mut config.shells.powershell5_profile_path,
            ShellKind::PowerShell7 => &mut config.shells.powershell7_profile_path,
            _ => {
                return Err(AliasError::Config(format!(
                    "unsupported shell for integration: {}",
                    kind.as_str()
                )))
            }
        };
        *slot = Some(path);
    }
    Ok(())
}

fn effective_config(
    config: &AppConfig,
    kind: &ShellKind,
    executable: Option<&Path>,
    profile: Option<&Path>,
) -> Result<AppConfig, AliasError> {
    let mut effective = config.clone();
    apply_overrides(&mut effective, kind, executable, profile)?;
    Ok(effective)
}

/// Plans one shell without writing anything. `executable_override` and
/// `profile_override` are previewed in memory; callers that want to persist
/// them use [`apply_shell_setup`] or the settings editor.
pub fn plan_shell_setup(
    config: &AppConfig,
    paths: &AppPaths,
    kind: &ShellKind,
    executable_override: Option<&Path>,
    profile_override: Option<&Path>,
) -> Result<ShellSetupPlan, AliasError> {
    let effective = effective_config(config, kind, executable_override, profile_override)?;
    let installation = detection::detect_shell(&effective, kind);
    plan_with_installation(&effective, paths, &installation)
}

/// Same as [`plan_shell_setup`] but reuses an installation row from
/// [`detection::detect_shells`], so setup UIs do not probe versions twice.
pub fn plan_shell_setup_with_installation(
    config: &AppConfig,
    paths: &AppPaths,
    kind: &ShellKind,
    installation: &ShellInstallation,
    executable_override: Option<&Path>,
    profile_override: Option<&Path>,
) -> Result<ShellSetupPlan, AliasError> {
    let effective = effective_config(config, kind, executable_override, profile_override)?;
    plan_with_installation(&effective, paths, installation)
}

fn plan_with_installation(
    config: &AppConfig,
    paths: &AppPaths,
    installation: &ShellInstallation,
) -> Result<ShellSetupPlan, AliasError> {
    let target = integration::loader_target(config, paths, installation.kind.as_str());
    let (loader_target, loader_status) = match target {
        Ok(LoaderTarget { path, .. }) => {
            let status = match integration::loader_status_with_config(
                config,
                paths,
                installation.kind.as_str(),
            ) {
                Ok(outcome) if outcome.installed => LoaderStatus::Installed,
                Ok(_) => LoaderStatus::Missing,
                Err(error) => LoaderStatus::Error {
                    code: integration::loader_error_code(&error).to_string(),
                    detail: error.to_string(),
                },
            };
            (Some(path), status)
        }
        Err(AliasError::ShellNotInstalled) => (None, LoaderStatus::Unavailable),
        Err(error) => (
            None,
            LoaderStatus::Error {
                code: integration::loader_error_code(&error).to_string(),
                detail: error.to_string(),
            },
        ),
    };
    let action = if installation.wsl {
        SetupAction::WslManual
    } else if !installation.detected || loader_target.is_none() {
        SetupAction::NeedsPath
    } else if loader_status.is_installed() {
        SetupAction::Enable
    } else {
        SetupAction::InstallLoader
    };
    Ok(ShellSetupPlan {
        kind: installation.kind.clone(),
        detected: installation.detected,
        executable: installation.executable.clone(),
        source: installation.source,
        wsl: installation.wsl,
        enabled: enabled_shells(config).contains(&installation.kind),
        loader_target,
        loader_status,
        action,
    })
}

/// Returns the marked loader block and target path without writing. `Ok(None)`
/// means this platform/configuration has no resolvable profile yet.
pub fn preview_loader(
    config: &AppConfig,
    paths: &AppPaths,
    kind: &ShellKind,
    executable_override: Option<&Path>,
    profile_override: Option<&Path>,
) -> Result<Option<LoaderPreview>, AliasError> {
    let effective = effective_config(config, kind, executable_override, profile_override)?;
    match integration::expected_loader_for(&effective, paths, kind.as_str()) {
        Ok((target, block)) => Ok(Some(LoaderPreview {
            target: target.path,
            powershell: target.powershell,
            block,
        })),
        Err(AliasError::ShellNotInstalled) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Default profile/RC path for a shell, including configured overrides. Used
/// as the prefilled value in path prompts.
pub fn default_profile_path(
    config: &AppConfig,
    paths: &AppPaths,
    kind: &ShellKind,
) -> Option<PathBuf> {
    integration::loader_target(config, paths, kind.as_str())
        .ok()
        .map(|target| target.path)
}

/// Confirmed setup: persist enabled intent and any path overrides, install the
/// loader when possible, then sync the shell. The caller must have obtained
/// explicit confirmation before calling this.
pub fn apply_shell_setup(
    paths: &AppPaths,
    kind: &ShellKind,
    options: &ShellSetupOptions,
) -> Result<ShellSetupOutcome, AliasError> {
    if !SUPPORTED_SHELLS.contains(kind) {
        return Err(AliasError::Config(format!(
            "unsupported shell for integration: {}",
            kind.as_str()
        )));
    }
    let mut config = paths.load_config()?;
    apply_overrides(
        &mut config,
        kind,
        options.executable.as_deref(),
        options.profile.as_deref(),
    )?;
    let installation = detection::detect_shell(&config, kind);
    if installation.wsl {
        return Err(AliasError::ShellNotReady(format!(
            "{} resolves to the WSL launcher; run aliasmgr doctor inside the WSL distribution",
            kind.label()
        )));
    }
    if !installation.detected {
        return Err(AliasError::ShellNotReady(format!(
            "{} executable was not found; provide the executable path in doctor",
            kind.label()
        )));
    }
    let target = match integration::loader_target(&config, paths, kind.as_str()) {
        Ok(target) => target,
        Err(AliasError::ShellNotInstalled) => {
            return Err(AliasError::ShellNotReady(format!(
                "{} has no profile path on this platform; provide one in doctor",
                kind.label()
            )))
        }
        Err(error) => return Err(error),
    };

    let mut enabled = enabled_shells(&config);
    if !enabled.contains(kind) {
        enabled.push(kind.clone());
        enabled = SUPPORTED_SHELLS
            .iter()
            .filter(|candidate| enabled.contains(candidate))
            .cloned()
            .collect();
    }
    config.shells.enabled = enabled
        .iter()
        .map(|candidate| candidate.as_str().to_string())
        .collect();
    config.validate()?;
    config.save(paths)?;

    let mut warnings = Vec::new();
    let (loader_changed, loader_installed, loader_error, loader_error_code) =
        match integration::install_loader_with_config(&config, paths, kind.as_str()) {
            Ok(LoaderOutcome {
                changed, installed, ..
            }) => (changed, installed, None, None),
            Err(error) => (
                false,
                false,
                Some(error.to_string()),
                Some(integration::loader_error_code(&error).to_string()),
            ),
        };
    if let Some(code) = &loader_error_code {
        warnings.push(format!("loader install failed ({code})"));
    }

    let (sync_run, sync_error) = if options.sync {
        match sync::sync_shells(&paths.root, std::slice::from_ref(kind)) {
            Ok(run) => (Some(run), None),
            Err(error) => (None, Some(error.to_string())),
        }
    } else {
        (None, None)
    };

    Ok(ShellSetupOutcome {
        kind: kind.clone(),
        enabled: true,
        executable: installation.executable,
        loader_target: Some(target.path),
        loader_changed,
        loader_installed,
        loader_error,
        loader_error_code,
        sync: sync_run,
        sync_error,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use std::fs;

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-shell-setup-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn fake_executable(root: &Path) -> PathBuf {
        let path = root.join(if cfg!(windows) { "bash.exe" } else { "bash" });
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        path
    }

    fn configured_bash(root: &Path) -> (AppPaths, AppConfig) {
        let paths = AppPaths {
            root: root.to_path_buf(),
        };
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(root.join("bashrc"));
        config
            .shells
            .executables
            .insert("bash".into(), fake_executable(root));
        config.save(&paths).unwrap();
        (paths, config)
    }

    #[test]
    fn enabled_shells_uses_stable_spellings_and_canonical_order() {
        let mut config = AppConfig::default();
        config.shells.enabled = vec![
            "powershell7".into(),
            "bash".into(),
            "power_shell5".into(),
            "fish".into(),
            "bash".into(),
        ];
        assert_eq!(
            enabled_shells(&config),
            vec![ShellKind::Bash, ShellKind::PowerShell7]
        );
        config.shells.enabled = vec!["zsh".into(), "powershell5".into()];
        assert_eq!(
            enabled_shells(&config),
            vec![ShellKind::Zsh, ShellKind::PowerShell5]
        );
    }

    #[test]
    fn set_enabled_shells_persists_deduplicated_canonical_values() {
        let root = temp_root("enabled");
        let paths = AppPaths { root: root.clone() };
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(root.join("custom-bashrc"));
        config.save(&paths).unwrap();

        let saved = set_enabled_shells(
            &paths,
            &[
                ShellKind::PowerShell7,
                ShellKind::Bash,
                ShellKind::PowerShell7,
            ],
        )
        .unwrap();
        assert_eq!(
            saved.shells.enabled,
            vec!["bash".to_string(), "powershell7".to_string()]
        );
        let reloaded = paths.load_config().unwrap();
        assert_eq!(reloaded.shells.enabled, saved.shells.enabled);
        assert_eq!(
            reloaded.shells.bash_rc_path,
            Some(root.join("custom-bashrc"))
        );

        let cleared = set_enabled_shells(&paths, &[]).unwrap();
        assert!(cleared.shells.enabled.is_empty());
        assert!(paths.load_config().unwrap().shells.enabled.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn set_enabled_shells_rejects_unsupported_shells_without_writing() {
        let root = temp_root("unsupported");
        let paths = AppPaths { root: root.clone() };
        AppConfig::default().save(&paths).unwrap();
        assert!(matches!(
            set_enabled_shells(&paths, &[ShellKind::Fish]),
            Err(AliasError::Config(message)) if message.contains("unsupported")
        ));
        assert!(paths.load_config().unwrap().shells.enabled.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn plan_reports_needs_path_for_undetected_shells_and_does_not_write() {
        let root = temp_root("plan-missing");
        let paths = AppPaths { root: root.clone() };
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(root.join("bashrc"));
        // An explicitly configured but missing path is authoritative, so PATH
        // and known installation directories cannot make this test flaky on a
        // machine that happens to have Bash installed.
        config
            .shells
            .executables
            .insert("bash".into(), root.join("missing-bash"));
        let plan = plan_shell_setup(&config, &paths, &ShellKind::Bash, None, None).unwrap();
        assert!(!plan.detected);
        assert_eq!(plan.action, SetupAction::NeedsPath);
        assert!(!paths.config_file().exists());
        assert!(!root.join("bashrc").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn plan_previews_a_manual_install_without_writing_and_apply_is_idempotent() {
        let root = temp_root("plan-apply");
        let (paths, config) = configured_bash(&root);
        let profile = root.join("bashrc");
        let plan = plan_shell_setup(&config, &paths, &ShellKind::Bash, None, None).unwrap();
        assert!(plan.detected);
        assert_eq!(plan.source, ShellDetectionSource::Manual);
        assert_eq!(plan.action, SetupAction::InstallLoader);
        assert_eq!(plan.loader_target, Some(profile.clone()));
        assert_eq!(plan.loader_status, LoaderStatus::Missing);

        let preview = preview_loader(&config, &paths, &ShellKind::Bash, None, None)
            .unwrap()
            .unwrap();
        assert_eq!(preview.target, profile);
        assert!(preview.block.contains(crate::shells::common::START_MARKER));
        assert!(preview.block.contains(crate::shells::common::END_MARKER));
        assert!(!profile.exists(), "preview must not write the profile");

        // No aliases exist yet, so sync is a successful no-op.
        let first =
            apply_shell_setup(&paths, &ShellKind::Bash, &ShellSetupOptions::syncing()).unwrap();
        assert!(first.enabled);
        assert!(first.loader_changed);
        assert!(first.loader_installed);
        assert!(first.loader_error.is_none());
        assert!(first.sync_error.is_none());
        assert!(profile.exists());
        assert!(paths
            .load_config()
            .unwrap()
            .shells
            .enabled
            .contains(&"bash".to_string()));

        let second =
            apply_shell_setup(&paths, &ShellKind::Bash, &ShellSetupOptions::syncing()).unwrap();
        assert!(!second.loader_changed, "apply must be idempotent");
        assert!(second.loader_installed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_syncs_an_existing_alias_and_reports_loader_and_sync_steps() {
        let root = temp_root("apply-sync");
        let (paths, _) = configured_bash(&root);
        let database = Database::open(root.join("aliases.db")).unwrap();
        let alias = crate::model::AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            shells: vec![ShellKind::Bash],
            ..Default::default()
        };
        database.insert_alias(&alias).unwrap();

        let outcome =
            apply_shell_setup(&paths, &ShellKind::Bash, &ShellSetupOptions::syncing()).unwrap();
        assert!(outcome.loader_installed);
        let run = outcome.sync.expect("sync step runs");
        assert_eq!(run.results.len(), 1);
        assert_eq!(run.results[0].shell, ShellKind::Bash);
        assert!(!run.failed());
        assert!(root.join("generated/bash.sh").exists());
        let reload = sync::reload_command(&root, &ShellKind::Bash);
        assert!(reload.contains("bash.sh"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_rejects_missing_executables_without_enabling_the_shell() {
        let root = temp_root("apply-missing");
        let paths = AppPaths { root: root.clone() };
        AppConfig::default().save(&paths).unwrap();
        let missing = root.join("does-not-exist");
        let options = ShellSetupOptions {
            executable: Some(missing),
            profile: Some(root.join("bashrc")),
            sync: true,
        };
        assert!(matches!(
            apply_shell_setup(&paths, &ShellKind::Bash, &options),
            Err(AliasError::ShellNotReady(message)) if message.contains("not found")
        ));
        assert!(paths.load_config().unwrap().shells.enabled.is_empty());
        assert!(!root.join("bashrc").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_does_not_write_a_windows_profile_for_wsl_bash() {
        if !cfg!(windows) {
            return;
        }
        let root = temp_root("apply-wsl");
        let paths = AppPaths { root: root.clone() };
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(root.join("bashrc"));
        let wsl = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("System32")
            .join("bash.exe");
        if !wsl.is_file() {
            let _ = fs::remove_dir_all(root);
            return;
        }
        config.shells.executables.insert("bash".into(), wsl);
        config.save(&paths).unwrap();
        let plan = plan_shell_setup(&config, &paths, &ShellKind::Bash, None, None).unwrap();
        assert!(plan.wsl);
        assert_eq!(plan.action, SetupAction::WslManual);
        assert!(matches!(
            apply_shell_setup(&paths, &ShellKind::Bash, &ShellSetupOptions::syncing()),
            Err(AliasError::ShellNotReady(message)) if message.contains("WSL")
        ));
        assert!(paths.load_config().unwrap().shells.enabled.is_empty());
        assert!(!root.join("bashrc").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_rejects_relative_paths_without_enabling_the_shell() {
        let root = temp_root("apply-relative");
        let paths = AppPaths { root: root.clone() };
        AppConfig::default().save(&paths).unwrap();
        let executable = fake_executable(&root);
        let options = ShellSetupOptions {
            executable: Some(executable),
            profile: Some(PathBuf::from("relative/.bashrc")),
            sync: false,
        };
        assert!(matches!(
            apply_shell_setup(&paths, &ShellKind::Bash, &options),
            Err(AliasError::Config(message))
                if message.contains("must be absolute") && message.contains("~/")
        ));
        assert!(paths.load_config().unwrap().shells.enabled.is_empty());
        assert!(!root.join("relative").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_reports_loader_failure_after_persisting_enabled_intent() {
        let root = temp_root("apply-loader-failure");
        let paths = AppPaths { root: root.clone() };
        let profile = root.join("bashrc");
        fs::write(
            &profile,
            format!("{}\n", crate::shells::common::START_MARKER),
        )
        .unwrap();
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(profile.clone());
        config
            .shells
            .executables
            .insert("bash".into(), fake_executable(&root));
        config.save(&paths).unwrap();

        let outcome =
            apply_shell_setup(&paths, &ShellKind::Bash, &ShellSetupOptions::syncing()).unwrap();
        assert!(outcome.enabled);
        assert!(outcome.loader_error.is_some());
        assert_eq!(outcome.loader_error_code.as_deref(), Some("loader_unsafe"));
        assert!(!outcome.loader_installed);
        assert!(paths
            .load_config()
            .unwrap()
            .shells
            .enabled
            .contains(&"bash".to_string()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_persists_profile_and_executable_overrides_for_later_plans() {
        let root = temp_root("apply-overrides");
        let paths = AppPaths { root: root.clone() };
        AppConfig::default().save(&paths).unwrap();
        let executable = fake_executable(&root);
        let profile = root.join("custom.rc");
        let options = ShellSetupOptions {
            executable: Some(executable.clone()),
            profile: Some(profile.clone()),
            sync: false,
        };
        // Bash has a default config-root RC, so this apply can proceed without
        // touching a real user profile.
        apply_shell_setup(&paths, &ShellKind::Bash, &options).unwrap();
        let config = paths.load_config().unwrap();
        assert_eq!(
            config.shells.executables.get("bash").cloned(),
            Some(executable)
        );
        assert_eq!(config.shells.bash_rc_path, Some(profile));
        let _ = fs::remove_dir_all(root);
    }
}
