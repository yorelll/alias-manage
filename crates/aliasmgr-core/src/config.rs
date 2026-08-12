use std::{env, fs, path::{Path, PathBuf}};

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
    fn powershell_versions_have_separate_generated_files() {
        let paths = AppPaths { root: PathBuf::from("config") };
        assert_ne!(paths.generated_path("powershell5"), paths.generated_path("powershell7"));
        assert_eq!(paths.generated_path("powershell5").extension().unwrap(), "ps1");
    }
}
