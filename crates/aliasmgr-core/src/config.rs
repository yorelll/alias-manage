use crate::{error::AliasError, model::ShellKind};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Component, Path, PathBuf},
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
    /// User intent: shells the user chose to integrate. Values use the stable
    /// `ShellKind::as_str()` spellings (`bash`, `zsh`, `powershell5`,
    /// `powershell7`), never serde enum spellings such as `power_shell5`.
    /// Empty by default: loading an old configuration never enables a shell.
    #[serde(default)]
    pub enabled: Vec<String>,
    /// Optional non-standard executable paths keyed by stable shell name.
    #[serde(default)]
    pub executables: std::collections::BTreeMap<String, PathBuf>,
    /// Last executable path the user dismissed or disabled for a detected but
    /// not-enabled shell. The GUI uses this to avoid re-prompting on every
    /// launch; a changed executable (reinstall or different install path)
    /// makes the shell "newly detected" again. Values are display-safe paths,
    /// not secrets, and never affect detection or enablement by themselves.
    #[serde(default)]
    pub seen: std::collections::BTreeMap<String, String>,
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
    /// Sync affected shells automatically after a successful alias write.
    /// Defaults to true for configurations written before this key existed.
    #[serde(default = "default_true")]
    pub auto_sync: bool,
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

pub fn default_true() -> bool {
    true
}

impl Default for ShellRuntimeConfig {
    fn default() -> Self {
        Self {
            default_shell: None,
            language: default_language(),
            auto_sync: true,
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
        let mut enabled = std::collections::BTreeSet::new();
        for value in &self.shells.enabled {
            let shell = value.trim();
            if ShellKind::parse_supported(shell).is_none() {
                return Err(AliasError::Config(format!(
                    "unsupported enabled shell: {value} (expected bash, zsh, powershell5, or powershell7)"
                )));
            }
            if !enabled.insert(shell.to_string()) {
                return Err(AliasError::Config(format!(
                    "duplicate enabled shell: {shell}"
                )));
            }
        }
        for (name, path) in &self.shells.executables {
            if ShellKind::parse_supported(name).is_none() {
                return Err(AliasError::Config(format!(
                    "unsupported shell executable key: {name} (expected bash, zsh, powershell5, or powershell7)"
                )));
            }
            if path.as_os_str().is_empty() {
                return Err(AliasError::Config(format!(
                    "shell executable path for {name} must not be empty"
                )));
            }
            if !path.is_absolute() {
                return Err(AliasError::Config(format!(
                    "shell executable path for {name} must be absolute (got {})",
                    path.display()
                )));
            }
        }
        for name in self.shells.seen.keys() {
            if ShellKind::parse_supported(name).is_none() {
                return Err(AliasError::Config(format!(
                    "unsupported seen shell key: {name} (expected bash, zsh, powershell5, or powershell7)"
                )));
            }
        }
        for (name, path) in [
            ("bash_rc_path", self.shells.bash_rc_path.as_deref()),
            ("zsh_rc_path", self.shells.zsh_rc_path.as_deref()),
            (
                "powershell5_profile_path",
                self.shells.powershell5_profile_path.as_deref(),
            ),
            (
                "powershell7_profile_path",
                self.shells.powershell7_profile_path.as_deref(),
            ),
        ] {
            let Some(path) = path else {
                continue;
            };
            if path.as_os_str().is_empty() {
                return Err(AliasError::Config(format!(
                    "shell profile path {name} must not be empty"
                )));
            }
            if !path.is_absolute() {
                return Err(AliasError::Config(format!(
                    "shell profile path {name} must be absolute (got {})",
                    path.display()
                )));
            }
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

    /// Expands a leading `~` in every persisted shell path. `load_config` calls
    /// this so a hand-edited `bash_rc_path = "~/.bashrc"` becomes an absolute
    /// path before validation and use; the expanded value is written back on
    /// the next save.
    pub fn expand_shell_paths(&mut self) {
        for path in [
            &mut self.shells.bash_rc_path,
            &mut self.shells.zsh_rc_path,
            &mut self.shells.powershell5_profile_path,
            &mut self.shells.powershell7_profile_path,
        ]
        .into_iter()
        .flatten()
        {
            *path = expand_home_path(path);
        }
        for path in self.shells.executables.values_mut() {
            *path = expand_home_path(path);
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
}

fn non_empty_env(key: &str) -> Option<std::ffi::OsString> {
    env::var_os(key).filter(|value| !value.is_empty())
}

/// The user's home directory as used for default shell profiles. Windows uses
/// `USERPROFILE` first (the shell's `~`) and falls back to `HOME`; Unix uses
/// `HOME`. Returns `None` in environments that do not provide one.
pub fn home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        non_empty_env("USERPROFILE")
            .or_else(|| non_empty_env("HOME"))
            .map(PathBuf::from)
    } else {
        non_empty_env("HOME").map(PathBuf::from)
    }
}

fn expand_home_path_with(path: &Path, home: Option<&Path>) -> PathBuf {
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(first)) if first == "~" => match home {
            Some(home) => home.join(components.as_path()),
            None => path.to_path_buf(),
        },
        _ => path.to_path_buf(),
    }
}

/// Expands a leading `~` (or `~/...` / `~\...`) using [`home_dir`]. Other
/// components are left untouched; `~user` is not expanded. Callers that accept
/// user-entered paths must also require the result to be absolute.
pub fn expand_home_path(path: &Path) -> PathBuf {
    expand_home_path_with(path, home_dir().as_deref())
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
        config.expand_shell_paths();
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
    use crate::ENV_LOCK;

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

    #[test]
    fn auto_sync_defaults_to_true_and_legacy_runtime_tables_inherit_it() {
        assert!(ShellRuntimeConfig::default().auto_sync);
        assert!(default_true());

        let root = std::env::temp_dir().join(format!(
            "aliasmgr-config-auto-sync-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let serialized = toml::to_string_pretty(&AppConfig::default()).unwrap();
        assert!(serialized.contains("auto_sync = true"), "{serialized}");
        let legacy = serialized
            .lines()
            .filter(|line| !line.trim_start().starts_with("auto_sync ="))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(paths.config_file(), legacy).unwrap();
        assert!(paths.load_config().unwrap().runtime.auto_sync);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enabled_and_executables_round_trip_with_stable_spellings() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-config-enabled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let mut config = AppConfig::default();
        config.shells.enabled = vec!["bash".into(), "powershell7".into()];
        config
            .shells
            .executables
            .insert("powershell7".into(), PathBuf::from(r"C:\Tools\pwsh.exe"));
        config
            .shells
            .executables
            .insert("bash".into(), PathBuf::from("/opt/git/bash"));
        config
            .shells
            .seen
            .insert("zsh".into(), "/usr/bin/zsh".into());
        config.save(&paths).unwrap();

        let saved = fs::read_to_string(paths.config_file()).unwrap();
        let value: toml::Value = toml::from_str(&saved).unwrap();
        let enabled = value["shells"]["enabled"].as_array().unwrap();
        assert_eq!(
            enabled
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>(),
            vec!["bash", "powershell7"]
        );
        assert_eq!(
            value["shells"]["executables"]["powershell7"].as_str(),
            Some(r"C:\Tools\pwsh.exe")
        );
        assert!(saved.contains("[shells.executables]"), "{saved}");
        assert!(!saved.contains("power_shell7"), "{saved}");

        let loaded = paths.load_config().unwrap();
        assert_eq!(loaded.shells.enabled, vec!["bash", "powershell7"]);
        assert_eq!(
            loaded.shells.executables.get("powershell7"),
            Some(&PathBuf::from(r"C:\Tools\pwsh.exe"))
        );
        assert_eq!(
            loaded.shells.seen.get("zsh").map(String::as_str),
            Some("/usr/bin/zsh")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_config_without_enabled_or_executables_loads_without_enabling_anything() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-config-legacy-enabled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let mut legacy = AppConfig::default();
        legacy.shells.bash_rc_path = Some(PathBuf::from("/tmp/custom-bashrc"));
        let serialized = toml::to_string_pretty(&legacy).unwrap();
        let serialized = serialized
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("enabled =")
                    && !trimmed.starts_with("executables =")
                    && !trimmed.starts_with("[shells.executables]")
                    && !trimmed.starts_with("bash =")
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(paths.config_file(), serialized).unwrap();

        let loaded = paths.load_config().unwrap();
        assert!(loaded.shells.enabled.is_empty());
        assert!(loaded.shells.executables.is_empty());
        assert_eq!(
            loaded.shells.bash_rc_path,
            Some(PathBuf::from("/tmp/custom-bashrc"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validation_rejects_invalid_enabled_entries_and_executable_maps() {
        let mut config = AppConfig::default();
        config.shells.enabled = vec!["power_shell5".into()];
        assert!(config.validate().is_err());
        config.shells.enabled = vec!["bash".into(), "bash".into()];
        assert!(config.validate().is_err());
        config.shells.enabled = vec![" bash ".into()];
        assert!(
            config.validate().is_ok(),
            "surrounding whitespace is normalized on read"
        );
        config.shells.enabled = vec!["fish".into()];
        assert!(config.validate().is_err());

        config.shells.enabled = vec!["bash".into()];
        config
            .shells
            .executables
            .insert("fish".into(), PathBuf::from("/bin/fish"));
        assert!(config.validate().is_err());
        config.shells.executables.clear();
        config
            .shells
            .executables
            .insert("bash".into(), PathBuf::new());
        assert!(config.validate().is_err());
        config
            .shells
            .executables
            .insert("bash".into(), absolute_test_path());
        assert!(config.validate().is_ok());

        config.shells.seen.insert("fish".into(), "/bin/fish".into());
        assert!(config.validate().is_err());
        config.shells.seen.clear();
        config.shells.seen.insert("bash".into(), "/bin/bash".into());
        assert!(config.validate().is_ok());
    }

    fn absolute_test_path() -> PathBuf {
        std::env::temp_dir().join("aliasmgr-absolute-test")
    }

    #[test]
    fn validation_requires_absolute_executable_and_profile_overrides() {
        let mut config = AppConfig::default();
        config
            .shells
            .executables
            .insert("bash".into(), PathBuf::from("relative/bash"));
        assert!(matches!(
            config.validate(),
            Err(AliasError::Config(message)) if message.contains("must be absolute")
        ));
        config
            .shells
            .executables
            .insert("bash".into(), absolute_test_path());
        config.shells.bash_rc_path = Some(PathBuf::from("relative/bashrc"));
        assert!(matches!(
            config.validate(),
            Err(AliasError::Config(message)) if message.contains("must be absolute")
        ));
        config.shells.bash_rc_path = Some(absolute_test_path().join(".bashrc"));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn leading_tilde_expands_from_the_provided_home_directory() {
        let home = if cfg!(windows) {
            PathBuf::from(r"C:\Users\tester")
        } else {
            PathBuf::from("/home/tester")
        };
        assert_eq!(
            expand_home_path_with(Path::new("~/.bashrc"), Some(&home)),
            home.join(".bashrc")
        );
        assert_eq!(
            expand_home_path_with(Path::new("~/nested/profile.ps1"), Some(&home)),
            home.join("nested/profile.ps1")
        );
        #[cfg(windows)]
        assert_eq!(
            expand_home_path_with(Path::new(r"~\.zshrc"), Some(&home)),
            home.join(".zshrc")
        );
        assert_eq!(expand_home_path_with(Path::new("~"), Some(&home)), home);
        let absent = Path::new("~/.bashrc");
        assert_eq!(expand_home_path_with(absent, None), absent);
        let ordinary = absolute_test_path();
        assert_eq!(expand_home_path_with(&ordinary, Some(&home)), ordinary);
        let user_home = Path::new("~other/.bashrc");
        assert_eq!(
            expand_home_path_with(user_home, Some(&home)),
            PathBuf::from("~other/.bashrc")
        );
    }
}
