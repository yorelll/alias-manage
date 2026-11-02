//! Shell-integration (loader) installation shared by the CLI and the GUI.
//!
//! Loader path resolution lives here so the two front ends cannot drift: the
//! CLI's `shell install` and the GUI's `shell_install` command both call these
//! helpers. Nothing in this module touches the real user profile unless the
//! caller passes a config root whose `config.toml` points at one.

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
    /// True when the loader block is present at the end of the profile.
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

fn render_loader(generated: &Path, powershell: bool) -> String {
    if powershell {
        common::render_powershell_loader(generated)
    } else {
        common::render_loader(generated)
    }
}

fn profile_content(path: &Path) -> String {
    // A missing profile is an empty profile. Other read errors are retried by
    // the write below, which surfaces the real I/O error to the caller.
    fs::read_to_string(path).unwrap_or_default()
}

/// Reports whether the loader block is installed for `shell` without writing.
///
/// A missing profile is reported as "not installed" rather than an error. An
/// unpaired marker is an `UnsafePath` error: the caller should surface it
/// instead of guessing at a repair.
pub fn loader_status(paths: &AppPaths, shell: &str) -> Result<LoaderOutcome, AliasError> {
    let config = paths.load_config()?;
    let target = loader_target(&config, paths, shell)?;
    let content = profile_content(&target.path);
    let installed =
        target.path.exists() && common::loader_position(&content)? == common::LoaderPosition::AtEnd;
    Ok(LoaderOutcome {
        shell: shell.to_ascii_lowercase(),
        target: target.path,
        changed: false,
        installed,
    })
}

/// Installs the marked loader block for `shell`, creating the profile and its
/// parent directory when needed. Re-installing an existing block is a no-op.
pub fn install_loader(paths: &AppPaths, shell: &str) -> Result<LoaderOutcome, AliasError> {
    let config = paths.load_config()?;
    let name = shell.to_ascii_lowercase();
    let target = loader_target(&config, paths, &name)?;
    let generated = paths.generated_path(&name);
    let loader = render_loader(&generated, target.powershell);
    let before = profile_content(&target.path);
    let after = common::install_loader(&before, &loader)?;
    let changed = after != before;
    if changed {
        common::write_loader_file_with_keep(
            &target.path,
            &before,
            &loader,
            config.backups.rc_keep,
        )?;
    }
    Ok(LoaderOutcome {
        shell: name,
        target: target.path,
        changed,
        installed: true,
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
    let before = fs::read_to_string(&target.path)?;
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

    #[cfg(windows)]
    #[test]
    fn resolves_windows_powershell_profile_defaults_from_userprofile() {
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
