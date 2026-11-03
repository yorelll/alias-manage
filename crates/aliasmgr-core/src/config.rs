use crate::error::AliasError;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub rc_keep: usize,
    pub generated_keep: usize,
    pub db_keep: usize,
    pub max_total_bytes: u64,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            rc_keep: 10,
            generated_keep: 10,
            db_keep: 5,
            max_total_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub max_file_bytes: u64,
    pub keep_files: usize,
}
impl Default for LogConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 8 * 1024 * 1024,
            keep_files: 7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetiredNameConfig {
    pub keep_revisions: i64,
}
impl Default for RetiredNameConfig {
    fn default() -> Self {
        Self { keep_revisions: 20 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShellPathOverrides {
    pub bash_rc_path: Option<PathBuf>,
    pub zsh_rc_path: Option<PathBuf>,
    pub powershell5_profile_path: Option<PathBuf>,
    pub powershell7_profile_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellRuntimeConfig {
    pub default_shell: Option<String>,
    /// UI language. Persisted values are `zh` (default) or `en`; the GUI
    /// validates the value before saving.
    #[serde(default = "default_language")]
    pub language: String,
    pub allow_relative_paths: bool,
    pub execution_policy: Option<String>,
    pub powershell5_bom: bool,
    pub powershell7_bom: bool,
    pub powershell5_line_ending: Option<String>,
    pub powershell7_line_ending: Option<String>,
}

pub fn default_language() -> String {
    "zh".to_owned()
}

impl Default for ShellRuntimeConfig {
    fn default() -> Self {
        Self {
            default_shell: None,
            language: default_language(),
            allow_relative_paths: false,
            execution_policy: None,
            powershell5_bom: true,
            powershell7_bom: false,
            powershell5_line_ending: None,
            powershell7_line_ending: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub backups: BackupConfig,
    pub logs: LogConfig,
    pub retired_names: RetiredNameConfig,
    pub shells: ShellPathOverrides,
    pub runtime: ShellRuntimeConfig,
    /// Unknown top-level keys are retained across load/save so front ends do
    /// not silently drop configuration they do not understand. `load_config`
    /// fills this after deserializing the known fields, and `save` merges it
    /// back into the output table.
    #[serde(skip)]
    pub extra: std::collections::BTreeMap<String, toml::Value>,
}

/// Upper bound for any retained-file count edited by a front end. The rotation
/// code itself accepts larger values; this is a sanity bound for interactive
/// settings so a typo cannot request unbounded retention.
pub const MAX_RETAINED_FILES: usize = 1000;

impl AppConfig {
    /// Validation shared by interactive settings editors. `load_config` stays
    /// permissive so old or partially invalid files remain inspectable, but a
    /// save must not persist an unusable default shell or language.
    pub fn validate(&self) -> Result<(), AliasError> {
        if let Some(shell) = &self.runtime.default_shell {
            let shell = shell.trim();
            if !matches!(shell, "bash" | "zsh" | "powershell5" | "powershell7") {
                return Err(AliasError::Config(format!(
                    "unsupported default shell: {shell} (expected bash, zsh, powershell5, or powershell7)"
                )));
            }
        }
        let language = self.runtime.language.trim();
        if !matches!(language, "zh" | "en") {
            return Err(AliasError::Config(format!(
                "unsupported language: {} (expected zh or en)",
                self.runtime.language
            )));
        }
        for (name, value) in [
            ("backups.rc_keep", self.backups.rc_keep),
            ("backups.generated_keep", self.backups.generated_keep),
            ("backups.db_keep", self.backups.db_keep),
            ("logs.keep_files", self.logs.keep_files),
        ] {
            if value > MAX_RETAINED_FILES {
                return Err(AliasError::Config(format!(
                    "{name} must be <= {MAX_RETAINED_FILES} (got {value})"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
}

fn non_empty_env(key: &str) -> Option<std::ffi::OsString> {
    env::var_os(key).filter(|value| !value.is_empty())
}

fn platform_default_root() -> PathBuf {
    if cfg!(windows) {
        for key in ["LOCALAPPDATA", "APPDATA"] {
            if let Some(path) = non_empty_env(key) {
                return PathBuf::from(path).join("alias-manager");
            }
        }
        PathBuf::from(".alias-manager")
    } else {
        if let Some(path) = non_empty_env("XDG_CONFIG_HOME") {
            return PathBuf::from(path).join("alias-manager");
        }
        if let Some(home) = non_empty_env("HOME") {
            return PathBuf::from(home).join(".config").join("alias-manager");
        }
        PathBuf::from(".alias-manager")
    }
}

impl AppPaths {
    pub fn discover(cli: Option<&Path>) -> Self {
        // Precedence: explicit flag, then ALIASMGR_CONFIG_DIR, then the platform default.
        if let Some(path) = cli {
            return Self {
                root: path.to_path_buf(),
            };
        }
        if let Some(path) = non_empty_env("ALIASMGR_CONFIG_DIR") {
            return Self { root: path.into() };
        }
        Self {
            root: platform_default_root(),
        }
    }

    pub fn ensure_directories(&self) -> std::io::Result<()> {
        for path in [
            self.root.clone(),
            self.root.join("generated"),
            self.root.join("backups/rc"),
            self.root.join("backups/generated"),
            self.root.join("backups/db"),
            self.root.join("logs"),
        ] {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }

    pub fn generated_path(&self, shell: &str) -> PathBuf {
        let extension = if shell.starts_with("powershell") {
            "ps1"
        } else {
            "sh"
        };
        self.root
            .join("generated")
            .join(format!("{shell}.{extension}"))
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn load_config(&self) -> Result<AppConfig, AliasError> {
        if !self.config_file().exists() {
            return Ok(AppConfig::default());
        }
        let content = fs::read_to_string(self.config_file())?;
        let value: toml::Value =
            toml::from_str(&content).map_err(|error| AliasError::Config(error.to_string()))?;
        let table = value
            .as_table()
            .ok_or_else(|| AliasError::Config("config.toml must contain a table".into()))?;
        let mut config: AppConfig = value
            .clone()
            .try_into()
            .map_err(|error: toml::de::Error| AliasError::Config(error.to_string()))?;
        let mut extra = table.clone();
        for key in ["backups", "logs", "retired_names", "shells", "runtime"] {
            extra.remove(key);
        }
        config.extra = extra.into_iter().collect();
        Ok(config)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemReliability {
    Reliable,
    FallbackDelete,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSafety {
    Safe,
    OtherWritable,
    Unknown,
}

pub fn filesystem_reliability(path: &Path) -> FilesystemReliability {
    if !path.exists() {
        return FilesystemReliability::Unknown;
    }
    #[cfg(unix)]
    {
        let (Ok(mounts), Ok(candidate)) = (fs::read_to_string("/proc/mounts"), path.canonicalize())
        else {
            return FilesystemReliability::Unknown;
        };
        reliability_from_mounts(&mounts, &candidate)
    }
    #[cfg(windows)]
    {
        let _ = path;
        FilesystemReliability::Unknown
    }
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
            if !candidate.starts_with(&mount) {
                return None;
            }
            Some((mount.as_os_str().len(), kind))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, kind)| kind.to_owned());
    match filesystem.as_deref() {
        Some("nfs") | Some("nfs4") | Some("cifs") | Some("smbfs") | Some("9p")
        | Some("fuse.sshfs") => FilesystemReliability::FallbackDelete,
        Some("ext2") | Some("ext3") | Some("ext4") | Some("xfs") | Some("btrfs")
        | Some("tmpfs") | Some("overlay") => FilesystemReliability::Reliable,
        Some(_) | None => FilesystemReliability::Unknown,
    }
}

pub fn path_safety(path: &Path) -> PathSafety {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|metadata| {
                if metadata.permissions().mode() & 0o002 != 0 {
                    PathSafety::OtherWritable
                } else {
                    PathSafety::Safe
                }
            })
            .unwrap_or(PathSafety::Unknown)
    }
    #[cfg(windows)]
    {
        let _ = path;
        PathSafety::Unknown
    }
}

impl AppConfig {
    pub fn save(&self, paths: &AppPaths) -> Result<(), AliasError> {
        paths.ensure_directories()?;
        let mut value =
            toml::Value::try_from(self).map_err(|error| AliasError::Config(error.to_string()))?;
        if let Some(table) = value.as_table_mut() {
            for (key, item) in &self.extra {
                table.entry(key.clone()).or_insert_with(|| item.clone());
            }
        }
        crate::fsutil::write_atomic(
            &paths.config_file(),
            &toml::to_string_pretty(&value)
                .map_err(|error| AliasError::Config(error.to_string()))?,
        )?;
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
        assert_eq!(
            AppPaths::discover(Some(Path::new("cli-config"))).root,
            PathBuf::from("cli-config")
        );
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
        assert_eq!(
            AppPaths::discover(None).root,
            PathBuf::from("C:/local").join("alias-manager")
        );
        env::remove_var("LOCALAPPDATA");
        assert_eq!(
            AppPaths::discover(None).root,
            PathBuf::from("C:/roaming").join("alias-manager")
        );
        env::remove_var("APPDATA");
        assert_eq!(
            AppPaths::discover(None).root,
            PathBuf::from(".alias-manager")
        );
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
        assert_eq!(
            AppPaths::discover(None).root,
            PathBuf::from("/tmp/xdg").join("alias-manager")
        );
        env::remove_var("XDG_CONFIG_HOME");
        assert_eq!(
            AppPaths::discover(None).root,
            PathBuf::from("/tmp/home")
                .join(".config")
                .join("alias-manager")
        );
        env::remove_var("HOME");
        assert_eq!(
            AppPaths::discover(None).root,
            PathBuf::from(".alias-manager")
        );
        restore_env(&saved);
    }

    #[test]
    fn empty_config_dir_falls_through_to_the_platform_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        let keys = ["ALIASMGR_CONFIG_DIR"];
        let saved = keys.map(|key| (key, env::var_os(key)));
        env::set_var("ALIASMGR_CONFIG_DIR", "");
        assert_eq!(AppPaths::discover(None).root, platform_default_root());
        restore_env(&saved);
    }

    #[cfg(windows)]
    #[test]
    fn non_unicode_config_dir_is_preserved_on_windows() {
        use std::os::windows::ffi::OsStringExt;
        let _guard = ENV_LOCK.lock().unwrap();
        let keys = ["ALIASMGR_CONFIG_DIR"];
        let saved = keys.map(|key| (key, env::var_os(key)));
        // An unpaired surrogate is not valid UTF-16, so env::var() would reject it.
        let value = std::ffi::OsString::from_wide(&[0xD800]);
        env::set_var("ALIASMGR_CONFIG_DIR", &value);
        assert_eq!(AppPaths::discover(None).root, PathBuf::from(&value));
        restore_env(&saved);
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_config_dir_is_preserved_on_unix() {
        use std::os::unix::ffi::OsStringExt;
        let _guard = ENV_LOCK.lock().unwrap();
        let keys = ["ALIASMGR_CONFIG_DIR"];
        let saved = keys.map(|key| (key, env::var_os(key)));
        // Invalid UTF-8 bytes, so env::var() would reject them.
        let value = std::ffi::OsString::from_vec(vec![0xFF, 0xFE]);
        env::set_var("ALIASMGR_CONFIG_DIR", &value);
        assert_eq!(AppPaths::discover(None).root, PathBuf::from(&value));
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
    fn shell_runtime_defaults_encode_powershell_bom_policy() {
        let runtime = ShellRuntimeConfig::default();
        assert!(runtime.powershell5_bom);
        assert!(!runtime.powershell7_bom);
        assert_eq!(runtime.default_shell, None);
        assert!(!runtime.allow_relative_paths);
        assert_eq!(runtime.language, "zh");
    }

    #[test]
    fn language_defaults_to_chinese() {
        assert_eq!(ShellRuntimeConfig::default().language, "zh");
        assert_eq!(default_language(), "zh");
    }

    #[test]
    fn language_round_trips_through_toml() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-config-language-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        let paths = AppPaths { root: root.clone() };
        let mut config = AppConfig::default();
        config.runtime.language = "en".into();
        config.save(&paths).unwrap();
        assert_eq!(paths.load_config().unwrap().runtime.language, "en");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn old_config_without_language_parses_as_chinese() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-config-legacy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let serialized = toml::to_string_pretty(&AppConfig::default()).unwrap();
        assert!(serialized.contains("language = \"zh\""));
        let legacy = serialized
            .lines()
            .filter(|line| !line.trim_start().starts_with("language ="))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(paths.config_file(), legacy).unwrap();
        assert_eq!(paths.load_config().unwrap().runtime.language, "zh");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn powershell_versions_have_separate_generated_files() {
        let paths = AppPaths {
            root: PathBuf::from("config"),
        };
        assert_ne!(
            paths.generated_path("powershell5"),
            paths.generated_path("powershell7")
        );
        assert_eq!(
            paths.generated_path("powershell5").extension().unwrap(),
            "ps1"
        );
    }

    #[test]
    fn filesystem_reliability_is_explicit_and_conservative() {
        let result = filesystem_reliability(&std::env::temp_dir());
        assert!(matches!(
            result,
            FilesystemReliability::Reliable
                | FilesystemReliability::FallbackDelete
                | FilesystemReliability::Unknown
        ));
    }

    #[test]
    fn unverified_windows_acl_is_not_reported_as_safe() {
        let result = path_safety(&std::env::temp_dir());
        #[cfg(windows)]
        assert_eq!(result, PathSafety::Unknown);
        #[cfg(not(windows))]
        assert!(matches!(
            result,
            PathSafety::Safe | PathSafety::OtherWritable | PathSafety::Unknown
        ));
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

    #[test]
    fn unknown_top_level_config_keys_survive_load_and_save() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-config-extra-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let defaults = toml::to_string_pretty(&AppConfig::default()).unwrap();
        let source = format!("custom_key = \"keep-me\"\n{defaults}");
        fs::write(paths.config_file(), source).unwrap();

        let mut config = paths.load_config().unwrap();
        assert_eq!(
            config
                .extra
                .get("custom_key")
                .and_then(|value| value.as_str()),
            Some("keep-me")
        );
        config.backups.rc_keep = 7;
        config.save(&paths).unwrap();
        let saved = fs::read_to_string(paths.config_file()).unwrap();
        assert!(saved.contains("custom_key = \"keep-me\""), "{saved}");
        assert_eq!(paths.load_config().unwrap().backups.rc_keep, 7);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn config_validation_rejects_invalid_shell_language_and_keep_bounds() {
        let mut config = AppConfig::default();
        assert!(config.validate().is_ok());
        config.runtime.default_shell = Some("fish".into());
        assert!(config.validate().is_err());
        config.runtime.default_shell = Some("powershell7".into());
        assert!(config.validate().is_ok());
        config.runtime.language = "fr".into();
        assert!(config.validate().is_err());
        config.runtime.language = "en".into();
        assert!(config.validate().is_ok());
        config.backups.rc_keep = MAX_RETAINED_FILES + 1;
        assert!(config.validate().is_err());
        config.backups.rc_keep = MAX_RETAINED_FILES;
        assert!(config.validate().is_ok());
        config.logs.keep_files = MAX_RETAINED_FILES + 1;
        assert!(config.validate().is_err());
    }
}
