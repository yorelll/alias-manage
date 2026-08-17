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
pub struct AppConfig { pub backups: BackupConfig, pub logs: LogConfig, pub retired_names: RetiredNameConfig, pub shells: ShellPathOverrides }

#[derive(Debug, Clone)]
pub struct AppPaths { pub root: PathBuf }

impl AppPaths {
    pub fn discover(cli: Option<&Path>) -> Self {
        if let Some(path) = cli { return Self { root: path.to_path_buf() }; }
        if let Ok(path) = env::var("ALIASMGR_CONFIG_DIR") { return Self { root: path.into() }; }
        let root = if cfg!(windows) {
            env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into()).into()
        } else {
            env::var("XDG_CONFIG_HOME").map(|path| PathBuf::from(path).join("alias-manager"))
                .unwrap_or_else(|_| PathBuf::from("~/.config/alias-manager"))
        };
        Self { root }
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
        let filesystem = std::fs::read_to_string("/proc/mounts").ok().and_then(|mounts| {
            let candidate = path.canonicalize().ok()?;
            mounts.lines().filter_map(|line| {
                let mut fields = line.split_whitespace();
                let _device = fields.next()?;
                let mount = PathBuf::from(fields.next()?);
                let kind = fields.next()?;
                candidate.starts_with(&mount).then_some(kind.to_owned())
            }).max_by_key(|mount| mount.len())
        });
        return match filesystem.as_deref() {
            Some("nfs") | Some("nfs4") | Some("cifs") | Some("smbfs") | Some("9p") | Some("fuse.sshfs") => FilesystemReliability::FallbackDelete,
            Some("ext2") | Some("ext3") | Some("ext4") | Some("xfs") | Some("btrfs") | Some("tmpfs") | Some("overlay") => FilesystemReliability::Reliable,
            Some(_) | None => FilesystemReliability::Unknown,
        };
    }
    #[cfg(windows)]
    { let _ = path; FilesystemReliability::Unknown }
}

pub fn path_safety(path: &Path) -> PathSafety {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return std::fs::metadata(path).map(|metadata| if metadata.permissions().mode() & 0o002 != 0 { PathSafety::OtherWritable } else { PathSafety::Safe }).unwrap_or(PathSafety::Unknown);
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
}
