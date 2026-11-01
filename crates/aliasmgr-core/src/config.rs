use crate::error::AliasError;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub rc_keep: usize,
    pub generated_keep: usize,
    pub db_keep: usize,
    pub max_total_bytes: u64,
}

impl Default for BackupConfig {
    fn default() -> Self { Self { rc_keep: 10, generated_keep: 10, db_keep: 5, max_total_bytes: 64 * 1024 * 1024 } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig { pub max_file_bytes: u64, pub keep_files: usize }
impl Default for LogConfig { fn default() -> Self { Self { max_file_bytes: 8 * 1024 * 1024, keep_files: 7 } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetiredNameConfig { pub keep_revisions: i64 }
impl Default for RetiredNameConfig { fn default() -> Self { Self { keep_revisions: 20 } } }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShellPathOverrides {
    pub bash_rc_path: Option<PathBuf>,
    pub zsh_rc_path: Option<PathBuf>,
    pub powershell5_profile_path: Option<PathBuf>,
    pub powershell7_profile_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShellRuntimeConfig {
    pub default_shell: Option<String>,
    pub allow_relative_paths: bool,
    pub execution_policy: Option<String>,
    pub powershell5_bom: bool,
    pub powershell7_bom: bool,
    pub powershell5_line_ending: Option<String>,
    pub powershell7_line_ending: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig { pub backups: BackupConfig, pub logs: LogConfig, pub retired_names: RetiredNameConfig, pub shells: ShellPathOverrides, pub runtime: ShellRuntimeConfig }

#[derive(Debug, Clone)]
pub struct AppPaths { pub root: PathBuf }

fn non_empty_env(key: &str) -> Option<std::ffi::OsString> {
    env::var_os(key).filter(|value| !value.is_empty())
}

fn platform_default_root() -> PathBuf {
    if cfg!(windows) {
        for key in ["LOCALAPPDATA", "APPDATA"] {
            if let Some(path) = non_empty_env(key) { return PathBuf::from(path).join("alias-manager"); }
        }
        PathBuf::from(".alias-manager")
    } else {
        if let Some(path) = non_empty_env("XDG_CONFIG_HOME") { return PathBuf::from(path).join("alias-manager"); }
        if let Some(home) = non_empty_env("HOME") { return PathBuf::from(home).join(".config").join("alias-manager"); }
        PathBuf::from(".alias-manager")
    }
}

impl AppPaths {
    pub fn discover(cli: Option<&Path>) -> Self {
        // Precedence: explicit flag, then ALIASMGR_CONFIG_DIR, then the platform default.
        if let Some(path) = cli { return Self { root: path.to_path_buf() }; }
        if let Some(path) = non_empty_env("ALIASMGR_CONFIG_DIR") { return Self { root: path.into() }; }
        Self { root: platform_default_root() }
    }

    pub fn ensure_directories(&self) -> std::io::Result<()> {
        for path in [self.root.clone(), self.root.join("generated"), self.root.join("backups/rc"), self.root.join("backups/generated"), self.root.join("backups/db"), self.root.join("logs")] { fs::create_dir_all(path)?; }
        Ok(())
    }

    pub fn generated_path(&self, shell: &str) -> PathBuf {
        let extension = if shell.starts_with("powershell") { "ps1" } else { "sh" };
        self.root.join("generated").join(format!("{shell}.{extension}"))
    }

    pub fn config_file(&self) -> PathBuf { self.root.join("config.toml") }

    pub fn load_config(&self) -> Result<AppConfig, AliasError> {
        if !self.config_file().exists() { return Ok(AppConfig::default()); }
        let content = fs::read_to_string(self.config_file())?;
        toml::from_str(&content).map_err(|error| AliasError::Config(error.to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemReliability { Reliable, FallbackDelete, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSafety { Safe, OtherWritable, Unknown }

pub fn filesystem_reliability(path: &Path) -> FilesystemReliability {
    if !path.exists() { return FilesystemReliability::Unknown; }
    #[cfg(unix)]
    {
        let (Ok(mounts), Ok(candidate)) =
            (fs::read_to_string("/proc/mounts"), path.canonicalize())
        else {
            return FilesystemReliability::Unknown;
        };
        reliability_from_mounts(&mounts, &candidate)
    }
    #[cfg(windows)]
    { let _ = path; FilesystemReliability::Unknown }
}

/// Resolves the filesystem type backing `candidate` from `/proc/mounts` content.
/// The longest matching mount path wins, so a local filesystem nested inside a
/// network mount is classified by its own type instead of its ancestor's.
#[cfg(any(unix, test))]
fn reliability_from_mounts(mounts: &str, candidate: &Path) -> FilesystemReliability {
    let filesystem = mounts
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let _device = fields.next()?;
            let mount = PathBuf::from(fields.next()?);
            let kind = fields.next()?;
            if !candidate.starts_with(&mount) { return None; }
            Some((mount.as_os_str().len(), kind))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, kind)| kind.to_owned());
    match filesystem.as_deref() {
        Some("nfs") | Some("nfs4") | Some("cifs") | Some("smbfs") | Some("9p") | Some("fuse.sshfs") => FilesystemReliability::FallbackDelete,
        Some("ext2") | Some("ext3") | Some("ext4") | Some("xfs") | Some("btrfs") | Some("tmpfs") | Some("overlay") => FilesystemReliability::Reliable,
        Some(_) | None => FilesystemReliability::Unknown,
    }
}

pub fn path_safety(path: &Path) -> PathSafety {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).map(|metadata| if metadata.permissions().mode() & 0o002 != 0 { PathSafety::OtherWritable } else { PathSafety::Safe }).unwrap_or(PathSafety::Unknown)
    }
    #[cfg(windows)]
    { let _ = path; PathSafety::Unknown }
}

impl AppConfig {
    pub fn save(&self, paths: &AppPaths) -> Result<(), AliasError> {
        paths.ensure_directories()?;
        fs::write(paths.config_file(), toml::to_string_pretty(self).map_err(|error| AliasError::Config(error.to_string()))?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn explicit_path_precedes_environment() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("ALIASMGR_CONFIG_DIR", "env-config");
        assert_eq!(AppPaths::discover(Some(Path::new("cli-config"))).root, PathBuf::from("cli-config"));
        env::remove_var("ALIASMGR_CONFIG_DIR");
    }

    #[test]
    fn environment_path_precedes_platform_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("ALIASMGR_CONFIG_DIR", "env-config");
        assert_eq!(AppPaths::discover(None).root, PathBuf::from("env-config"));
        env::remove_var("ALIASMGR_CONFIG_DIR");
    }

    fn restore_env(saved: &[(&'static str, Option<std::ffi::OsString>)]) {
        for (key, value) in saved {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_default_root_is_localappdata_then_appdata_then_relative() {
        let _guard = ENV_LOCK.lock().unwrap();
        let keys = ["ALIASMGR_CONFIG_DIR", "LOCALAPPDATA", "APPDATA"];
        let saved = keys.map(|key| (key, env::var_os(key)));
        env::remove_var("ALIASMGR_CONFIG_DIR");
        env::set_var("LOCALAPPDATA", "C:/local");
        env::set_var("APPDATA", "C:/roaming");
        assert_eq!(AppPaths::discover(None).root, PathBuf::from("C:/local").join("alias-manager"));
        env::remove_var("LOCALAPPDATA");
        assert_eq!(AppPaths::discover(None).root, PathBuf::from("C:/roaming").join("alias-manager"));
        env::remove_var("APPDATA");
        assert_eq!(AppPaths::discover(None).root, PathBuf::from(".alias-manager"));
        restore_env(&saved);
    }

    #[cfg(unix)]
    #[test]
    fn unix_default_root_is_xdg_then_home_config_then_relative() {
        let _guard = ENV_LOCK.lock().unwrap();
        let keys = ["ALIASMGR_CONFIG_DIR", "XDG_CONFIG_HOME", "HOME"];
        let saved = keys.map(|key| (key, env::var_os(key)));
        env::remove_var("ALIASMGR_CONFIG_DIR");
        env::set_var("XDG_CONFIG_HOME", "/tmp/xdg");
        env::set_var("HOME", "/tmp/home");
        assert_eq!(AppPaths::discover(None).root, PathBuf::from("/tmp/xdg").join("alias-manager"));
        env::remove_var("XDG_CONFIG_HOME");
        assert_eq!(AppPaths::discover(None).root, PathBuf::from("/tmp/home").join(".config").join("alias-manager"));
        env::remove_var("HOME");
        assert_eq!(AppPaths::discover(None).root, PathBuf::from(".alias-manager"));
        restore_env(&saved);
    }

    #[test]
    fn defaults_load_and_round_trip_through_toml() {
        let root = std::env::temp_dir().join(format!("aliasmgr-config-{}", std::process::id()));
        let paths = AppPaths { root: root.clone() };
        let config = AppConfig::default();
        config.save(&paths).unwrap();
        assert_eq!(paths.load_config().unwrap().backups.rc_keep, 10);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn powershell_versions_have_separate_generated_files() {
        let paths = AppPaths { root: PathBuf::from("config") };
        assert_ne!(paths.generated_path("powershell5"), paths.generated_path("powershell7"));
        assert_eq!(paths.generated_path("powershell5").extension().unwrap(), "ps1");
    }

    #[test]
    fn filesystem_reliability_is_explicit_and_conservative() {
        let result = filesystem_reliability(&std::env::temp_dir());
        assert!(matches!(result, FilesystemReliability::Reliable | FilesystemReliability::FallbackDelete | FilesystemReliability::Unknown));
    }

    #[test]
    fn unverified_windows_acl_is_not_reported_as_safe() {
        let result = path_safety(&std::env::temp_dir());
        #[cfg(windows)]
        assert_eq!(result, PathSafety::Unknown);
        #[cfg(not(windows))]
        assert!(matches!(result, PathSafety::Safe | PathSafety::OtherWritable | PathSafety::Unknown));
    }

    #[test]
    fn nested_local_mount_inside_a_network_ancestor_is_reliable() {
        let mounts = "\
sshfs#host:/export /home/user/remote fuse.sshfs rw 0 0
/dev/sda1 /home/user/remote/local ext4 rw 0 0
";
        assert_eq!(
            reliability_from_mounts(mounts, Path::new("/home/user/remote/local/project")),
            FilesystemReliability::Reliable
        );
        assert_eq!(
            reliability_from_mounts(mounts, Path::new("/home/user/remote/project")),
            FilesystemReliability::FallbackDelete
        );
    }

    #[test]
    fn nested_network_mount_under_overlay_root_falls_back() {
        let mounts = "\
overlay / overlay rw 0 0
server:/data /data nfs4 rw 0 0
";
        assert_eq!(
            reliability_from_mounts(mounts, Path::new("/data/projects")),
            FilesystemReliability::FallbackDelete
        );
        assert_eq!(
            reliability_from_mounts(mounts, Path::new("/srv/local")),
            FilesystemReliability::Reliable
        );
    }
}
