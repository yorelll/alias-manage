//! Shell-integration (loader) installation shared by the CLI and the GUI.
//!
//! Loader path resolution lives here so the two front ends cannot drift: the
//! CLI's `shell install`/`uninstall` and the GUI's `shell_install`/uninstall
//! both call these helpers. When no profile path is configured, PowerShell
//! targets resolve to the platform default (`%USERPROFILE%\Documents\...` on
//! Windows, `$HOME/.config/powershell/profile.ps1` elsewhere); tests must
//! override those paths or the environment and use an isolated config root.

use crate::{
    config::{AppConfig, AppPaths},
    error::AliasError,
    shells::common,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Result of resolving where a shell's loader block lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderTarget {
    pub path: PathBuf,
    /// True for PowerShell profiles, which need the PowerShell loader syntax.
    pub powershell: bool,
}

/// Result of checking or changing the loader block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderOutcome {
    pub shell: String,
    pub target: PathBuf,
    /// True when the install/uninstall call actually changed the profile.
    pub changed: bool,
    /// True when the expected loader block is present at the end of the profile.
    pub installed: bool,
}

/// Default PowerShell profile path used when no override is configured.
pub fn default_powershell_profile_path(shell: &str) -> Option<PathBuf> {
    if cfg!(windows) {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from)?;
        let folder = match shell {
            "powershell5" => "WindowsPowerShell",
            "powershell7" => "PowerShell",
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
            "powershell5" => None,
            "powershell7" => std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".config").join("powershell").join("profile.ps1")),
            _ => None,
        }
    }
}

/// Resolves the profile/RC file for a Shell and whether the loader block must
/// use PowerShell syntax. A configured path always wins over the platform
/// default, which keeps tests and custom setups away from real profiles.
pub fn loader_target(
    config: &AppConfig,
    paths: &AppPaths,
    shell: &str,
) -> Result<LoaderTarget, AliasError> {
    let name = shell.to_ascii_lowercase();
    let (path, powershell) = match name.as_str() {
        "bash" => (
            config
                .shells
                .bash_rc_path
                .clone()
                .unwrap_or_else(|| paths.root.join(".bashrc")),
            false,
        ),
        "zsh" => (
            config
                .shells
                .zsh_rc_path
                .clone()
                .unwrap_or_else(|| paths.root.join(".zshrc")),
            false,
        ),
        "powershell5" => config
            .shells
            .powershell5_profile_path
            .clone()
            .or_else(|| default_powershell_profile_path("powershell5"))
            .map(|path| (path, true))
            .ok_or(AliasError::ShellNotInstalled)?,
        "powershell7" => config
            .shells
            .powershell7_profile_path
            .clone()
            .or_else(|| default_powershell_profile_path("powershell7"))
            .map(|path| (path, true))
            .ok_or(AliasError::ShellNotInstalled)?,
        _ => return Err(AliasError::ShellNotInstalled),
    };
    Ok(LoaderTarget { path, powershell })
}

/// Every loader target that can be resolved on this platform, in stable shell
/// order, deduplicated by profile path. Unsupported shells (PowerShell 5.1
/// outside Windows) are skipped. Used by both uninstall paths so a loader
/// installed at a platform default is still found.
pub fn all_loader_targets(config: &AppConfig, paths: &AppPaths) -> Vec<(String, LoaderTarget)> {
    let mut targets: Vec<(String, LoaderTarget)> = Vec::new();
    for shell in ["bash", "zsh", "powershell5", "powershell7"] {
        let Ok(target) = loader_target(config, paths, shell) else {
            continue;
        };
        if targets
            .iter()
            .any(|(_, existing)| existing.path == target.path)
        {
            continue;
        }
        targets.push((shell.to_string(), target));
    }
    targets
}

fn render_loader(generated: &Path, powershell: bool) -> String {
    if powershell {
        common::render_powershell_loader(generated)
    } else {
        common::render_loader(generated)
    }
}

/// Reads a profile, treating a missing file as an empty profile. Other read
/// errors (permissions, a directory at the path, invalid UTF-8) propagate so
/// callers can report a distinct `loader_unreadable` condition instead of
/// silently claiming "not installed".
fn profile_content(path: &Path) -> Result<String, AliasError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

/// Extracts the managed block, including both markers. `loader_position`
/// validates that the markers are paired before this is called.
fn managed_block(content: &str) -> Option<&str> {
    let start = content.find(common::START_MARKER)?;
    let end = content.find(common::END_MARKER)? + common::END_MARKER.len();
    Some(&content[start..end])
}

fn normalized_block(value: &str) -> String {
    value.replace("\r\n", "\n").trim_end().to_string()
}

/// True when the profile contains exactly the expected managed block. This is
/// stricter than "a marker exists": a loader pointing at a different config
/// root must be reported as missing and repaired by `install_loader`.
fn block_matches(content: &str, loader: &str) -> bool {
    managed_block(content)
        .map(|block| normalized_block(block) == normalized_block(loader))
        .unwrap_or(false)
}

/// Reports whether the expected loader block is installed for `shell` without
/// writing.
///
/// A missing profile is reported as "not installed". An unpaired marker is an
/// `UnsafePath` error, and an unreadable profile is an I/O error; callers
/// surface those instead of guessing at a repair.
pub fn loader_status(paths: &AppPaths, shell: &str) -> Result<LoaderOutcome, AliasError> {
    let config = paths.load_config()?;
    let name = shell.to_ascii_lowercase();
    let target = loader_target(&config, paths, &name)?;
    let content = profile_content(&target.path)?;
    let loader = render_loader(&paths.generated_path(&name), target.powershell);
    let installed = target.path.exists()
        && common::loader_position(&content)? == common::LoaderPosition::AtEnd
        && block_matches(&content, &loader);
    Ok(LoaderOutcome {
        shell: name,
        target: target.path,
        changed: false,
        installed,
    })
}

/// Installs the expected loader block for `shell`, creating the profile and its
/// parent directory when needed. A block that is missing, points at another
/// config root, or is not at the end of the profile is replaced/relocated.
pub fn install_loader(paths: &AppPaths, shell: &str) -> Result<LoaderOutcome, AliasError> {
    let config = paths.load_config()?;
    let name = shell.to_ascii_lowercase();
    let target = loader_target(&config, paths, &name)?;
    let generated = paths.generated_path(&name);
    let loader = render_loader(&generated, target.powershell);
    let before = profile_content(&target.path)?;
    let already_installed = target.path.exists()
        && common::loader_position(&before)? == common::LoaderPosition::AtEnd
        && block_matches(&before, &loader);
    if already_installed {
        return Ok(LoaderOutcome {
            shell: name,
            target: target.path,
            changed: false,
            installed: true,
        });
    }

    // Remove any stale/mismatched/relocated block first, then append the
    // expected one at the end. Unpaired markers fail here as UnsafePath.
    let cleaned = common::remove_loader(&before)?;
    let after = common::install_loader(&cleaned, &loader)?;
    let changed = after != before;
    if changed {
        common::write_profile_content_with_keep(&target.path, &after, config.backups.rc_keep)?;
    }
    let installed = common::loader_position(&after)? == common::LoaderPosition::AtEnd
        && block_matches(&after, &loader);
    Ok(LoaderOutcome {
        shell: name,
        target: target.path,
        changed,
        installed,
    })
}

/// Removes the marked loader block for `shell`. A missing profile or a profile
/// without a block is a successful no-op.
pub fn uninstall_loader(paths: &AppPaths, shell: &str) -> Result<LoaderOutcome, AliasError> {
    let config = paths.load_config()?;
    let name = shell.to_ascii_lowercase();
    let target = loader_target(&config, paths, &name)?;
    if !target.path.exists() {
        return Ok(LoaderOutcome {
            shell: name,
            target: target.path,
            changed: false,
            installed: false,
        });
    }
    let before = profile_content(&target.path)?;
    let cleaned = common::remove_loader(&before)?;
    let changed = cleaned != before;
    if changed {
        common::write_profile_content_with_keep(&target.path, &cleaned, config.backups.rc_keep)?;
    }
    Ok(LoaderOutcome {
        shell: name,
        target: target.path,
        changed,
        installed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that mutate process-wide profile environment vars.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-integration-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn configured_paths(label: &str, shell: &str) -> (PathBuf, PathBuf) {
        let root = temp_root(label);
        let paths = AppPaths { root: root.clone() };
        let profile = root.join("profile.rc");
        let mut config = AppConfig::default();
        match shell {
            "bash" => config.shells.bash_rc_path = Some(profile.clone()),
            "zsh" => config.shells.zsh_rc_path = Some(profile.clone()),
            "powershell5" => config.shells.powershell5_profile_path = Some(profile.clone()),
            "powershell7" => config.shells.powershell7_profile_path = Some(profile.clone()),
            _ => unreachable!(),
        }
        config.save(&paths).unwrap();
        (root, profile)
    }

    #[test]
    fn installs_a_loader_block_once_and_reports_it() {
        let (root, profile) = configured_paths("install", "bash");
        let paths = AppPaths { root: root.clone() };

        let first = install_loader(&paths, "bash").unwrap();
        assert!(first.changed);
        assert!(first.installed);
        assert_eq!(first.target, profile);
        let content = fs::read_to_string(&profile).unwrap();
        assert!(content.contains(common::START_MARKER));
        assert!(content.contains(&paths.generated_path("bash").to_string_lossy().into_owned()));

        let second = install_loader(&paths, "bash").unwrap();
        assert!(!second.changed, "re-install must be idempotent");
        assert!(loader_status(&paths, "bash").unwrap().installed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_a_missing_profile_and_parent_directory() {
        let root = temp_root("nested");
        let profile = root.join("nested/deeper/profile.rc");
        let paths = AppPaths { root: root.clone() };
        let mut config = AppConfig::default();
        config.shells.zsh_rc_path = Some(profile.clone());
        config.save(&paths).unwrap();

        let outcome = install_loader(&paths, "zsh").unwrap();
        assert!(outcome.changed);
        assert!(profile.exists());
        assert!(fs::read_to_string(&profile)
            .unwrap()
            .contains(&paths.generated_path("zsh").to_string_lossy().into_owned()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn uninstall_removes_only_the_managed_block() {
        let (root, profile) = configured_paths("uninstall", "bash");
        let paths = AppPaths { root: root.clone() };
        fs::write(&profile, "user content\n").unwrap();
        install_loader(&paths, "bash").unwrap();

        let outcome = uninstall_loader(&paths, "bash").unwrap();
        assert!(outcome.changed);
        assert!(!outcome.installed);
        assert_eq!(fs::read_to_string(&profile).unwrap(), "user content\n");

        let again = uninstall_loader(&paths, "bash").unwrap();
        assert!(!again.changed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_distinguishes_missing_profile_and_unpaired_markers() {
        let (root, profile) = configured_paths("status", "powershell7");
        let paths = AppPaths { root: root.clone() };
        assert!(!loader_status(&paths, "powershell7").unwrap().installed);

        fs::write(&profile, format!("{}\n", common::START_MARKER)).unwrap();
        assert!(matches!(
            loader_status(&paths, "powershell7"),
            Err(AliasError::UnsafePath)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn powershell_loader_uses_powershell_syntax() {
        let (root, profile) = configured_paths("powershell", "powershell5");
        let paths = AppPaths { root: root.clone() };
        install_loader(&paths, "powershell5").unwrap();
        let content = fs::read_to_string(&profile).unwrap();
        assert!(content.contains("Test-Path -LiteralPath"));
        assert!(content.contains(
            &paths
                .generated_path("powershell5")
                .to_string_lossy()
                .into_owned()
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsupported_shell_names_are_rejected() {
        let root = temp_root("unsupported");
        let paths = AppPaths { root: root.clone() };
        assert!(matches!(
            install_loader(&paths, "fish"),
            Err(AliasError::ShellNotInstalled)
        ));
        assert!(matches!(
            uninstall_loader(&paths, "sh"),
            Err(AliasError::ShellNotInstalled)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn install_failure_surfaces_the_io_error() {
        let root = temp_root("install-failure");
        let directory = root.join("profile-is-a-directory");
        fs::create_dir_all(&directory).unwrap();
        let paths = AppPaths { root: root.clone() };
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(directory);
        config.save(&paths).unwrap();

        let result = install_loader(&paths, "bash");
        assert!(result.is_err(), "writing over a directory must fail");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn loader_status_propagates_unreadable_profiles() {
        let root = temp_root("unreadable-profile");
        let directory = root.join("profile-is-a-directory");
        fs::create_dir_all(&directory).unwrap();
        let paths = AppPaths { root: root.clone() };
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(directory);
        config.save(&paths).unwrap();

        // A directory at the profile path is not "no loader"; it is unreadable.
        assert!(matches!(
            loader_status(&paths, "bash"),
            Err(AliasError::Io(_))
        ));
        assert!(matches!(
            uninstall_loader(&paths, "bash"),
            Err(AliasError::Io(_))
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_loader_for_another_config_root_is_reported_missing_and_repaired() {
        let shared = temp_root("shared-profile");
        let profile = shared.join("profile.rc");
        let root_a = temp_root("root-a");
        let root_b = temp_root("root-b");
        for root in [&root_a, &root_b] {
            let paths = AppPaths { root: root.clone() };
            let mut config = AppConfig::default();
            config.shells.bash_rc_path = Some(profile.clone());
            config.save(&paths).unwrap();
        }
        let paths_a = AppPaths {
            root: root_a.clone(),
        };
        let paths_b = AppPaths {
            root: root_b.clone(),
        };

        install_loader(&paths_a, "bash").unwrap();
        let content_a = fs::read_to_string(&profile).unwrap();
        assert!(content_a.contains(
            &paths_a
                .generated_path("bash")
                .to_string_lossy()
                .into_owned()
        ));

        let status = loader_status(&paths_b, "bash").unwrap();
        assert!(
            !status.installed,
            "a loader pointing at another config root must not count as installed"
        );

        let repaired = install_loader(&paths_b, "bash").unwrap();
        assert!(repaired.changed);
        assert!(repaired.installed);
        let content_b = fs::read_to_string(&profile).unwrap();
        assert!(content_b.contains(
            &paths_b
                .generated_path("bash")
                .to_string_lossy()
                .into_owned()
        ));
        assert!(!content_b.contains(
            &paths_a
                .generated_path("bash")
                .to_string_lossy()
                .into_owned()
        ));
        assert_eq!(
            content_b.matches(common::START_MARKER).count(),
            1,
            "repair must leave exactly one managed block"
        );
        let _ = fs::remove_dir_all(shared);
        let _ = fs::remove_dir_all(root_a);
        let _ = fs::remove_dir_all(root_b);
    }

    #[test]
    fn a_relocated_loader_is_reported_missing_and_moved_to_the_end() {
        let (root, profile) = configured_paths("relocated", "bash");
        let paths = AppPaths { root: root.clone() };
        let loader = common::render_loader(&paths.generated_path("bash"));
        fs::write(&profile, format!("user before\n{loader}user after\n")).unwrap();

        let status = loader_status(&paths, "bash").unwrap();
        assert!(
            !status.installed,
            "a block before user content is not at the end"
        );

        let repaired = install_loader(&paths, "bash").unwrap();
        assert!(repaired.changed);
        assert!(repaired.installed);
        let content = fs::read_to_string(&profile).unwrap();
        assert!(content.starts_with("user before\nuser after\n"));
        assert!(content.trim_end().ends_with(common::END_MARKER));
        assert_eq!(content.matches(common::START_MARKER).count(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn all_loader_targets_deduplicates_and_skips_unsupported_shells() {
        let root = temp_root("all-targets");
        let paths = AppPaths { root: root.clone() };
        let shared = root.join("shared.rc");
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(shared.clone());
        config.shells.zsh_rc_path = Some(shared.clone());
        config.shells.powershell7_profile_path = Some(root.join("ps7.ps1"));
        config.save(&paths).unwrap();

        let targets = all_loader_targets(&config, &paths);
        let paths_only: Vec<_> = targets
            .iter()
            .map(|(_, target)| target.path.clone())
            .collect();
        assert_eq!(
            paths_only.iter().filter(|path| **path == shared).count(),
            1,
            "the same profile must be visited once"
        );
        assert!(
            targets.iter().any(|(shell, _)| shell == "bash")
                || targets.iter().any(|(shell, _)| shell == "zsh"),
            "the shared profile must be visited once under the first shell"
        );
        assert!(targets.iter().any(|(shell, _)| shell == "powershell7"));
        #[cfg(not(windows))]
        assert!(
            !targets.iter().any(|(shell, _)| shell == "powershell5"),
            "PowerShell 5.1 has no default profile outside Windows"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn env_overridden_default_profile_is_installed_and_uninstalled() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let home = temp_root("default-profile-env");
        let previous = std::env::var_os("USERPROFILE");
        std::env::set_var("USERPROFILE", &home);

        let root = temp_root("default-profile-config");
        let paths = AppPaths { root: root.clone() };
        // No configured profile: resolution must use the platform default.
        AppConfig::default().save(&paths).unwrap();
        let installed = install_loader(&paths, "powershell7").unwrap();
        let expected = home
            .join("Documents")
            .join("PowerShell")
            .join("Microsoft.PowerShell_profile.ps1");
        assert_eq!(installed.target, expected);
        assert!(expected.exists());
        assert!(loader_status(&paths, "powershell7").unwrap().installed);

        let removed = uninstall_loader(&paths, "powershell7").unwrap();
        assert!(removed.changed);
        assert!(
            !expected.exists()
                || !fs::read_to_string(&expected)
                    .unwrap()
                    .contains(common::START_MARKER)
        );

        match previous {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
        let _ = fs::remove_dir_all(home);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(not(windows))]
    #[test]
    fn env_overridden_default_profile_is_installed_and_uninstalled() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let home = temp_root("default-profile-env");
        let previous = std::env::var_os("HOME");
        std::env::set_var("HOME", &home);

        let root = temp_root("default-profile-config");
        let paths = AppPaths { root: root.clone() };
        AppConfig::default().save(&paths).unwrap();
        let installed = install_loader(&paths, "powershell7").unwrap();
        let expected = home.join(".config").join("powershell").join("profile.ps1");
        assert_eq!(installed.target, expected);
        assert!(expected.exists());
        assert!(loader_status(&paths, "powershell7").unwrap().installed);

        let removed = uninstall_loader(&paths, "powershell7").unwrap();
        assert!(removed.changed);
        assert!(!fs::read_to_string(&expected)
            .unwrap()
            .contains(common::START_MARKER));

        match previous {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        let _ = fs::remove_dir_all(home);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn resolves_windows_powershell_profile_defaults_from_userprofile() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let home = temp_root("profile-defaults");
        let home_string = home.to_string_lossy().into_owned();
        // Scope the process-wide override so it is always restored.
        let previous = std::env::var_os("USERPROFILE");
        std::env::set_var("USERPROFILE", &home);
        let five = default_powershell_profile_path("powershell5");
        let seven = default_powershell_profile_path("powershell7");
        let bash = default_powershell_profile_path("bash");
        match previous {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }

        assert_eq!(
            five,
            Some(
                PathBuf::from(&home_string)
                    .join("Documents")
                    .join("WindowsPowerShell")
                    .join("Microsoft.PowerShell_profile.ps1")
            )
        );
        assert_eq!(
            seven,
            Some(
                PathBuf::from(&home_string)
                    .join("Documents")
                    .join("PowerShell")
                    .join("Microsoft.PowerShell_profile.ps1")
            )
        );
        assert_eq!(bash, None);
        let _ = fs::remove_dir_all(home);
    }
}
