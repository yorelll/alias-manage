use crate::{
    config::AppConfig,
    error::AliasError,
    model::{Conflict, ShellKind, SUPPORTED_SHELLS},
};
use std::{
    env,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerShellInstallation {
    pub kind: ShellKind,
    pub executable: PathBuf,
    pub version: Option<String>,
}

pub fn powershell_priority(
    installations: &[PowerShellInstallation],
) -> Vec<PowerShellInstallation> {
    let mut sorted = installations.to_vec();
    sorted.sort_by(|left, right| {
        let kind_order = |kind: &ShellKind| {
            if matches!(kind, ShellKind::PowerShell7) {
                0
            } else {
                1
            }
        };
        kind_order(&left.kind)
            .cmp(&kind_order(&right.kind))
            .then_with(|| right.version.cmp(&left.version))
            .then_with(|| left.executable.cmp(&right.executable))
    });
    sorted
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionContext {
    pub explicit: Option<ShellKind>,
    pub parent_shell: Option<ShellKind>,
    pub shell_env: Option<String>,
    pub login_shell: Option<ShellKind>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionResult {
    pub selected: Option<ShellKind>,
    pub installed: Vec<ShellKind>,
    pub source: String,
}
/// How an existing user definition was written, so the GUI can label it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionKind {
    Alias,
    Function,
    SetAlias,
}

impl DefinitionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DefinitionKind::Alias => "alias",
            DefinitionKind::Function => "function",
            DefinitionKind::SetAlias => "set_alias",
        }
    }
}

/// A strict, metadata-only match. The raw source line is intentionally not
/// returned: callers must not persist or display profile content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionSource {
    pub path: PathBuf,
    pub line: usize,
    pub kind: DefinitionKind,
    /// True only for a complete single-line definition.
    pub recoverable: bool,
}

pub fn detect(context: &DetectionContext) -> DetectionResult {
    if let Some(shell) = &context.explicit {
        return DetectionResult {
            selected: Some(shell.clone()),
            installed: installed_shells(),
            source: "explicit".into(),
        };
    }
    if let Some(shell) = &context.parent_shell {
        return DetectionResult {
            selected: Some(shell.clone()),
            installed: installed_shells(),
            source: "parent".into(),
        };
    }
    if let Some(shell) = context.shell_env.as_deref().and_then(parse_shell_name) {
        return DetectionResult {
            selected: Some(shell),
            installed: installed_shells(),
            source: "shell_env".into(),
        };
    }
    if let Some(shell) = &context.login_shell {
        return DetectionResult {
            selected: Some(shell.clone()),
            installed: installed_shells(),
            source: "login_shell".into(),
        };
    }
    let installed = installed_shells();
    DetectionResult {
        selected: installed.first().cloned(),
        installed,
        source: "installed".into(),
    }
}

pub fn installed_shells() -> Vec<ShellKind> {
    let mut result = Vec::new();
    for (name, shell) in [
        ("bash", ShellKind::Bash),
        ("zsh", ShellKind::Zsh),
        ("powershell", ShellKind::PowerShell5),
        ("pwsh", ShellKind::PowerShell7),
    ] {
        if command_on_path(name) {
            result.push(shell);
        }
    }
    result
}

pub fn installed_powershells() -> Vec<PowerShellInstallation> {
    [
        ("powershell", ShellKind::PowerShell5),
        ("pwsh", ShellKind::PowerShell7),
    ]
    .into_iter()
    .filter_map(|(name, kind)| {
        let executable = command_path(name)?;
        let output = Command::new(&executable)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$PSVersionTable.PSVersion.ToString()",
            ])
            .output()
            .ok()?;
        let version = String::from_utf8(output.stdout).ok()?.trim().to_string();
        Some(PowerShellInstallation {
            kind,
            executable,
            version: (!version.is_empty()).then_some(version),
        })
    })
    .collect()
}

pub fn conflicts(name: &str) -> Result<Vec<Conflict>, AliasError> {
    let mut result = Vec::new();
    if ["ls", "cp", "mv", "rm", "cat", "gc", "cd", "pwd"]
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(name))
    {
        result.push(Conflict {
            name: name.into(),
            reason: "NameReserved or built-in command".into(),
        });
    }
    if command_on_path(name) {
        result.push(Conflict {
            name: name.into(),
            reason: "executable found on PATH".into(),
        });
    }
    Ok(result)
}

/// Characters that may legally follow a definition name.
fn name_boundary(rest: &str) -> bool {
    match rest.chars().next() {
        None => true,
        Some(character) => {
            character.is_whitespace() || matches!(character, '=' | '(' | '{' | ')' | ';' | ',')
        }
    }
}

/// Parses one line strictly. Only exact-name definitions match, comments are
/// skipped, and `recoverable` is true only for a complete single-line shape.
fn parse_definition(line: &str, name: &str) -> Option<(DefinitionKind, bool)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    // `alias <name>=<value>` (bash/zsh).
    if let Some(rest) = trimmed.strip_prefix("alias ") {
        if let Some(after) = rest
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
        {
            let value = after.trim_start();
            let recoverable = !value.is_empty() && !value.starts_with('#');
            return Some((DefinitionKind::Alias, recoverable));
        }
    }

    // `function <name>` / `function global:<name>` (bash, zsh, PowerShell).
    if let Some(rest) = trimmed.strip_prefix("function ") {
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("global:").unwrap_or(rest);
        if let Some(after) = rest.strip_prefix(name) {
            if name_boundary(after) {
                return Some((DefinitionKind::Function, after.contains('}')));
            }
        }
    }

    // `<name>() { ... }` (bash/zsh shorthand).
    if let Some(after) = trimmed
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix("()"))
    {
        let body = after.trim_start();
        if body.starts_with('{') {
            return Some((DefinitionKind::Function, body.contains('}')));
        }
    }

    // `Set-Alias ... -Name <name> ...` (PowerShell, case-insensitive command).
    let lower = trimmed.to_ascii_lowercase();
    if let Some(after_command) = lower.strip_prefix("set-alias") {
        if name_boundary(after_command) {
            if let Some(index) = lower.find("-name") {
                let rest = trimmed[index + "-name".len()..].trim_start();
                if let Some(after) = rest.strip_prefix(name) {
                    if name_boundary(after) {
                        let after_lower = after.to_ascii_lowercase();
                        let recoverable = after_lower
                            .find("-value")
                            .map(|value_index| {
                                !after[value_index + "-value".len()..].trim().is_empty()
                            })
                            .unwrap_or(false);
                        return Some((DefinitionKind::SetAlias, recoverable));
                    }
                }
            }
        }
    }

    None
}

/// Finds an exact-name definition in the first matching file. Only reads files
/// and never executes them.
pub fn find_definition_source(
    paths: &[PathBuf],
    name: &str,
) -> Result<Option<DefinitionSource>, AliasError> {
    for path in paths {
        if let Ok(content) = fs::read_to_string(path) {
            for (index, line) in content.lines().enumerate() {
                if let Some((kind, recoverable)) = parse_definition(line, name) {
                    return Ok(Some(DefinitionSource {
                        path: path.clone(),
                        line: index + 1,
                        kind,
                        recoverable,
                    }));
                }
            }
        }
    }
    Ok(None)
}

fn parse_shell_name(value: &str) -> Option<ShellKind> {
    match Path::new(value).file_name()?.to_str()? {
        "bash" => Some(ShellKind::Bash),
        "zsh" => Some(ShellKind::Zsh),
        "powershell" => Some(ShellKind::PowerShell5),
        "pwsh" => Some(ShellKind::PowerShell7),
        _ => None,
    }
}
fn command_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|path| {
            let direct = path.join(name);
            if direct.is_file() {
                return Some(direct);
            }
            if cfg!(windows) {
                [".exe", ".cmd", ".bat"]
                    .iter()
                    .map(|ext| path.join(format!("{name}{ext}")))
                    .find(|candidate| candidate.is_file())
            } else {
                None
            }
        })
    })
}

fn command_on_path(name: &str) -> bool {
    command_path(name).is_some()
}

/// How the executable for a supported shell was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellDetectionSource {
    /// `shells.executables.<shell>` from `config.toml`.
    Manual,
    /// Found by looking up the shell's executable name on `PATH`.
    Path,
    /// Found in a well-known installation directory.
    KnownPath,
    /// The executable is the Windows WSL launcher (`System32\bash.exe` or its
    /// WindowsApps alias). Its profile lives inside a Linux distribution.
    Wsl,
    /// No executable was found. Detection is best effort; the caller must
    /// still offer a manual path.
    NotFound,
}

impl ShellDetectionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ShellDetectionSource::Manual => "manual",
            ShellDetectionSource::Path => "path",
            ShellDetectionSource::KnownPath => "known_path",
            ShellDetectionSource::Wsl => "wsl",
            ShellDetectionSource::NotFound => "not_found",
        }
    }
}

/// One row of shared shell detection. All four supported shells are always
/// returned, including shells that were not found, because automatic detection
/// cannot be guaranteed to see every install method (portable builds, Store
/// aliases, shells installed after this process started, WSL, PATH changes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellInstallation {
    pub kind: ShellKind,
    pub executable: Option<PathBuf>,
    pub detected: bool,
    pub source: ShellDetectionSource,
    /// Version reported by a short read-only probe. `None` means unknown
    /// (probe failed, timed out, or was skipped for WSL), not "not installed".
    pub version: Option<String>,
    /// True only for the Windows WSL bash launcher.
    pub wsl: bool,
}

/// Version probes are bounded so a hung launcher cannot block setup UI.
pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Detects all four supported shells using `PATH`, known installation paths
/// and the manual `shells.executables` overrides. Read-only: it never changes
/// the environment, registry or profiles.
pub fn detect_shells(config: &AppConfig) -> Vec<ShellInstallation> {
    detect_shells_with_timeout(config, DEFAULT_PROBE_TIMEOUT)
}

/// Detects one supported shell. Used by setup planning to avoid probing every
/// shell when only one plan is requested.
pub fn detect_shell(config: &AppConfig, kind: &ShellKind) -> ShellInstallation {
    detect_one(
        config,
        &DetectionEnvironment::from_process(),
        kind.clone(),
        DEFAULT_PROBE_TIMEOUT,
    )
}

/// Same as [`detect_shells`] with an explicit probe timeout. Exposed for
/// diagnostics and tests; production callers should use the default.
pub fn detect_shells_with_timeout(config: &AppConfig, timeout: Duration) -> Vec<ShellInstallation> {
    detect_shells_in(config, &DetectionEnvironment::from_process(), timeout)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DetectionEnvironment {
    pub path: Option<OsString>,
    pub system_root: Option<PathBuf>,
    pub program_files: Option<PathBuf>,
    pub program_files_x86: Option<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    pub system_drive: Option<PathBuf>,
}

impl DetectionEnvironment {
    fn from_process() -> Self {
        Self {
            path: env::var_os("PATH"),
            system_root: env::var_os("SystemRoot").map(PathBuf::from),
            program_files: env::var_os("ProgramFiles").map(PathBuf::from),
            program_files_x86: env::var_os("ProgramFiles(x86)").map(PathBuf::from),
            local_app_data: env::var_os("LOCALAPPDATA").map(PathBuf::from),
            system_drive: env::var_os("SystemDrive").map(PathBuf::from),
        }
    }
}

pub(crate) fn detect_shells_in(
    config: &AppConfig,
    environment: &DetectionEnvironment,
    timeout: Duration,
) -> Vec<ShellInstallation> {
    SUPPORTED_SHELLS
        .iter()
        .map(|kind| detect_one(config, environment, kind.clone(), timeout))
        .collect()
}

fn detect_one(
    config: &AppConfig,
    environment: &DetectionEnvironment,
    kind: ShellKind,
    timeout: Duration,
) -> ShellInstallation {
    let key = kind.as_str();
    if let Some(executable) = config.shells.executables.get(key) {
        // A configured path is authoritative: if it does not exist, report the
        // shell as not detected instead of silently falling back to another
        // installation the user did not choose.
        let detected = executable.is_file();
        let wsl = detected && is_wsl_bash(executable, environment);
        return installation(
            kind,
            Some(executable.clone()),
            detected,
            ShellDetectionSource::Manual,
            wsl,
            timeout,
        );
    }

    if let Some(executable) = command_path_in(environment.path.as_ref(), executable_name(&kind)) {
        let wsl = is_wsl_bash(&executable, environment);
        let source = if wsl {
            ShellDetectionSource::Wsl
        } else {
            ShellDetectionSource::Path
        };
        return installation(kind, Some(executable), true, source, wsl, timeout);
    }

    for (executable, wsl) in known_candidates(&kind, environment) {
        if executable.is_file() {
            let source = if wsl {
                ShellDetectionSource::Wsl
            } else {
                ShellDetectionSource::KnownPath
            };
            return installation(kind, Some(executable), true, source, wsl, timeout);
        }
    }

    installation(
        kind,
        None,
        false,
        ShellDetectionSource::NotFound,
        false,
        timeout,
    )
}

fn installation(
    kind: ShellKind,
    executable: Option<PathBuf>,
    detected: bool,
    source: ShellDetectionSource,
    wsl: bool,
    timeout: Duration,
) -> ShellInstallation {
    let version = match (&executable, detected, wsl) {
        (Some(path), true, false) => probe_version(path, &kind, timeout),
        _ => None,
    };
    ShellInstallation {
        kind,
        executable,
        detected,
        source,
        version,
        wsl,
    }
}

fn executable_name(kind: &ShellKind) -> &'static str {
    match kind {
        ShellKind::Bash => "bash",
        ShellKind::Zsh => "zsh",
        ShellKind::PowerShell5 => "powershell",
        ShellKind::PowerShell7 => "pwsh",
        ShellKind::Fish => "fish",
        ShellKind::PosixSh => "sh",
    }
}

fn command_path_in(path: Option<&OsString>, name: &str) -> Option<PathBuf> {
    env::split_paths(path?).find_map(|directory| {
        let direct = directory.join(name);
        if direct.is_file() {
            return Some(direct);
        }
        if cfg!(windows) {
            [".exe", ".cmd", ".bat"]
                .iter()
                .map(|extension| directory.join(format!("{name}{extension}")))
                .find(|candidate| candidate.is_file())
        } else {
            None
        }
    })
}

fn known_candidates(kind: &ShellKind, environment: &DetectionEnvironment) -> Vec<(PathBuf, bool)> {
    let mut candidates = Vec::new();
    if cfg!(windows) {
        let system_root = environment
            .system_root
            .clone()
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let program_files = environment
            .program_files
            .clone()
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
        let program_files_x86 = environment
            .program_files_x86
            .clone()
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
        let system_drive = environment
            .system_drive
            .clone()
            .unwrap_or_else(|| PathBuf::from(r"C:\"));
        match kind {
            ShellKind::PowerShell5 => {
                candidates.push((
                    system_root
                        .join("System32")
                        .join("WindowsPowerShell")
                        .join("v1.0")
                        .join("powershell.exe"),
                    false,
                ));
            }
            ShellKind::PowerShell7 => {
                for base in [&program_files, &program_files_x86] {
                    candidates.push((base.join("PowerShell").join("7").join("pwsh.exe"), false));
                    candidates.push((
                        base.join("PowerShell").join("7-preview").join("pwsh.exe"),
                        false,
                    ));
                }
                if let Some(local) = &environment.local_app_data {
                    candidates.push((
                        local.join("Microsoft").join("WindowsApps").join("pwsh.exe"),
                        false,
                    ));
                }
            }
            ShellKind::Bash => {
                for base in [&program_files, &program_files_x86] {
                    candidates.push((base.join("Git").join("bin").join("bash.exe"), false));
                    candidates.push((
                        base.join("Git").join("usr").join("bin").join("bash.exe"),
                        false,
                    ));
                }
                for base in [
                    system_drive.join("msys64"),
                    system_drive.join("cygwin64"),
                    system_drive.join("cygwin"),
                ] {
                    candidates.push((base.join("usr").join("bin").join("bash.exe"), false));
                    candidates.push((base.join("bin").join("bash.exe"), false));
                }
                // The WSL launcher is a valid bash, but its profile is inside
                // the distribution and must not be written from Windows.
                candidates.push((system_root.join("System32").join("bash.exe"), true));
                if let Some(local) = &environment.local_app_data {
                    candidates.push((
                        local.join("Microsoft").join("WindowsApps").join("bash.exe"),
                        true,
                    ));
                }
            }
            ShellKind::Zsh => {
                for base in [
                    system_drive.join("msys64"),
                    system_drive.join("cygwin64"),
                    system_drive.join("cygwin"),
                ] {
                    candidates.push((base.join("usr").join("bin").join("zsh.exe"), false));
                    candidates.push((base.join("bin").join("zsh.exe"), false));
                }
            }
            _ => {}
        }
    } else {
        let name = executable_name(kind);
        for directory in ["/bin", "/usr/bin", "/usr/local/bin"] {
            candidates.push((PathBuf::from(directory).join(name), false));
        }
    }
    candidates
}

fn normalize_for_comparison(path: &Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

fn is_wsl_bash(path: &Path, environment: &DetectionEnvironment) -> bool {
    if !cfg!(windows) || path.file_name().and_then(|name| name.to_str()) != Some("bash.exe") {
        return false;
    }
    let system_root = environment
        .system_root
        .clone()
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let launcher = system_root.join("System32").join("bash.exe");
    if normalize_for_comparison(path) == normalize_for_comparison(&launcher) {
        return true;
    }
    environment.local_app_data.as_ref().is_some_and(|local| {
        normalize_for_comparison(path)
            == normalize_for_comparison(
                &local.join("Microsoft").join("WindowsApps").join("bash.exe"),
            )
    })
}

/// Runs a bounded, read-only version probe. Any failure is reported as
/// unknown (`None`) and never changes the detected flag.
pub(crate) fn probe_version(
    executable: &Path,
    kind: &ShellKind,
    timeout: Duration,
) -> Option<String> {
    let mut command = Command::new(executable);
    match kind {
        ShellKind::PowerShell5 | ShellKind::PowerShell7 => {
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$PSVersionTable.PSVersion.ToString()",
            ]);
        }
        _ => {
            command.arg("--version");
        }
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: probing must not flash a console for the user.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let stdout = child.stdout.take()?;
    let mut output = String::new();
    std::io::BufReader::new(stdout)
        .read_to_string(&mut output)
        .ok()?;
    let value = output.lines().next().unwrap_or_default().trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powershell_priority_prefers_ps7_then_highest_version() {
        let sorted = powershell_priority(&[
            PowerShellInstallation {
                kind: ShellKind::PowerShell5,
                executable: PathBuf::from("powershell.exe"),
                version: Some("5.1".into()),
            },
            PowerShellInstallation {
                kind: ShellKind::PowerShell7,
                executable: PathBuf::from("pwsh-7.2.exe"),
                version: Some("7.2".into()),
            },
            PowerShellInstallation {
                kind: ShellKind::PowerShell7,
                executable: PathBuf::from("pwsh-7.4.exe"),
                version: Some("7.4".into()),
            },
        ]);
        assert_eq!(sorted[0].version.as_deref(), Some("7.4"));
        assert_eq!(sorted[1].version.as_deref(), Some("7.2"));
        assert_eq!(sorted[2].kind, ShellKind::PowerShell5);
    }

    #[test]
    fn detection_precedence_is_explicit_parent_environment_login_installed() {
        let result = detect(&DetectionContext {
            explicit: Some(ShellKind::Zsh),
            parent_shell: Some(ShellKind::Bash),
            shell_env: Some("/bin/bash".into()),
            login_shell: Some(ShellKind::PowerShell7),
        });
        assert_eq!(result.selected, Some(ShellKind::Zsh));
        assert_eq!(result.source, "explicit");
        let result = detect(&DetectionContext {
            explicit: None,
            parent_shell: Some(ShellKind::Bash),
            shell_env: Some("/bin/zsh".into()),
            login_shell: None,
        });
        assert_eq!(result.selected, Some(ShellKind::Bash));
    }
    #[test]
    fn reports_reserved_and_path_conflicts() {
        let result = conflicts("ls").unwrap();
        assert!(!result.is_empty());
        assert!(result[0].reason.contains("built-in"));
    }
    #[test]
    fn finds_simple_definition_source_with_line_number() {
        let path =
            std::env::temp_dir().join(format!("aliasmgr-detection-{}.rc", std::process::id()));
        fs::write(&path, "# comment\nalias gs='git status'\n").unwrap();
        let found = find_definition_source(std::slice::from_ref(&path), "gs")
            .unwrap()
            .unwrap();
        assert_eq!(found.line, 2);
        assert_eq!(found.kind, DefinitionKind::Alias);
        assert!(found.recoverable);
        let _ = fs::remove_file(path);
    }

    fn unique_rc(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aliasmgr-detection-{label}-{}-{}.rc",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ))
    }

    #[test]
    fn exact_name_matching_ignores_substring_definitions() {
        let path = unique_rc("exact");
        fs::write(
            &path,
            "alias mygs='git'\nfunction myfoo {\nfunction global:myps {\nmybar() {\nSet-Alias -Name mysa -Value git\n",
        )
        .unwrap();
        for name in ["gs", "foo", "ps", "bar", "sa"] {
            assert!(
                find_definition_source(std::slice::from_ref(&path), name)
                    .unwrap()
                    .is_none(),
                "{name} must not match a longer name"
            );
        }
        for name in ["mygs", "myfoo", "myps", "mybar", "mysa"] {
            assert!(
                find_definition_source(std::slice::from_ref(&path), name)
                    .unwrap()
                    .is_some(),
                "{name} must match its exact definition"
            );
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn definition_shapes_report_kind_and_recoverability() {
        let path = unique_rc("shapes");
        fs::write(
            &path,
            "alias ga='git add'\nalias gb=\nfunction fc { echo hi; }\nfunction fd {\nfunction global:pe { echo hi }\nfe() { echo hi; }\nff() {\nSet-Alias -Name sa -Value git\nSet-Alias -Name sb\n# alias gc='git commit'\n",
        )
        .unwrap();
        let cases = [
            ("ga", DefinitionKind::Alias, true),
            ("gb", DefinitionKind::Alias, false),
            ("fc", DefinitionKind::Function, true),
            ("fd", DefinitionKind::Function, false),
            ("pe", DefinitionKind::Function, true),
            ("fe", DefinitionKind::Function, true),
            ("ff", DefinitionKind::Function, false),
            ("sa", DefinitionKind::SetAlias, true),
            ("sb", DefinitionKind::SetAlias, false),
        ];
        for (name, kind, recoverable) in cases {
            let found = find_definition_source(std::slice::from_ref(&path), name)
                .unwrap()
                .unwrap_or_else(|| panic!("{name} must match"));
            assert_eq!(found.kind, kind, "{name} kind");
            assert_eq!(found.recoverable, recoverable, "{name} recoverability");
        }
        assert!(find_definition_source(std::slice::from_ref(&path), "gc")
            .unwrap()
            .is_none());
        let _ = fs::remove_file(path);
    }

    fn temp_detection_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-detect-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn empty_environment() -> DetectionEnvironment {
        DetectionEnvironment {
            path: None,
            ..Default::default()
        }
    }

    fn executable_name_for_test(kind: &ShellKind) -> &'static str {
        if cfg!(windows) {
            match kind {
                ShellKind::Bash => "bash.exe",
                ShellKind::Zsh => "zsh.exe",
                ShellKind::PowerShell5 => "powershell.exe",
                ShellKind::PowerShell7 => "pwsh.exe",
                _ => "sh.exe",
            }
        } else {
            executable_name(kind)
        }
    }

    fn write_fake_executable(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn always_returns_all_four_shells_even_when_none_are_detected() {
        let mut config = AppConfig::default();
        let root = temp_detection_root("none");
        for kind in SUPPORTED_SHELLS {
            config
                .shells
                .executables
                .insert(kind.as_str().into(), root.join("missing"));
        }
        let rows = detect_shells_in(&config, &empty_environment(), Duration::from_millis(50));
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows.iter().map(|row| row.kind.clone()).collect::<Vec<_>>(),
            SUPPORTED_SHELLS.to_vec()
        );
        for row in rows {
            assert!(!row.detected);
            // A configured-but-missing path is still the manual source: it is
            // authoritative and must not silently fall back to PATH.
            assert_eq!(row.source, ShellDetectionSource::Manual);
            assert_eq!(row.version, None);
            assert!(!row.wsl);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn shells_without_any_configuration_use_the_not_found_source_when_nothing_matches() {
        // PowerShell 5.1 has no known path on Unix, so an empty environment
        // cannot detect it.
        #[cfg(unix)]
        {
            let rows = detect_shells_in(
                &AppConfig::default(),
                &empty_environment(),
                Duration::from_millis(50),
            );
            let ps5 = rows
                .iter()
                .find(|row| row.kind == ShellKind::PowerShell5)
                .unwrap();
            assert!(!ps5.detected);
            assert_eq!(ps5.source, ShellDetectionSource::NotFound);
            assert_eq!(ps5.executable, None);
        }
        #[cfg(windows)]
        {
            // On Windows the system PowerShell is a known path, so the
            // not-found source is covered by the all-four test above with an
            // explicit missing manual path.
        }
    }

    #[test]
    fn manual_executable_has_priority_over_path_and_is_authoritative_when_missing() {
        let root = temp_detection_root("manual");
        let manual = root.join(executable_name_for_test(&ShellKind::Bash));
        write_fake_executable(&manual, "#!/bin/sh\nexit 0\n");
        let path_dir = root.join("path-dir");
        write_fake_executable(
            &path_dir.join(executable_name_for_test(&ShellKind::Bash)),
            "#!/bin/sh\nexit 0\n",
        );
        let mut config = AppConfig::default();
        config
            .shells
            .executables
            .insert("bash".into(), manual.clone());
        let mut environment = empty_environment();
        environment.path = Some(path_dir.into_os_string());
        let rows = detect_shells_in(&config, &environment, Duration::from_millis(50));
        let bash = rows.iter().find(|row| row.kind == ShellKind::Bash).unwrap();
        assert!(bash.detected);
        assert_eq!(bash.executable.as_deref(), Some(manual.as_path()));
        assert_eq!(bash.source, ShellDetectionSource::Manual);

        config
            .shells
            .executables
            .insert("bash".into(), root.join("not-there"));
        let rows = detect_shells_in(&config, &environment, Duration::from_millis(50));
        let bash = rows.iter().find(|row| row.kind == ShellKind::Bash).unwrap();
        assert!(!bash.detected, "manual path must not fall back to PATH");
        assert_eq!(bash.source, ShellDetectionSource::Manual);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn path_lookup_finds_each_supported_executable_name() {
        let root = temp_detection_root("path");
        let path_dir = root.join("path-dir");
        for kind in SUPPORTED_SHELLS {
            write_fake_executable(
                &path_dir.join(executable_name_for_test(&kind)),
                "#!/bin/sh\nexit 0\n",
            );
        }
        let mut environment = empty_environment();
        environment.path = Some(path_dir.into_os_string());
        let rows = detect_shells_in(
            &AppConfig::default(),
            &environment,
            Duration::from_millis(50),
        );
        for kind in SUPPORTED_SHELLS {
            let row = rows.iter().find(|row| row.kind == kind).unwrap();
            assert!(row.detected, "{kind:?} should be detected on PATH");
            assert_eq!(row.source, ShellDetectionSource::Path);
            assert_eq!(
                row.executable.as_ref().and_then(|path| path.file_name()),
                Some(std::ffi::OsStr::new(executable_name_for_test(&kind)))
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_known_install_paths_are_detected_without_path_entries() {
        let root = temp_detection_root("known-win");
        let system_root = root.join("Windows");
        let program_files = root.join("Program Files");
        let local_app_data = root.join("LocalAppData");
        let system_drive = root.join("C");
        let ps5 = system_root.join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let ps7 = program_files.join("PowerShell/7/pwsh.exe");
        write_fake_executable(&ps5, "");
        write_fake_executable(&ps7, "");
        let environment = DetectionEnvironment {
            path: Some(OsString::new()),
            system_root: Some(system_root),
            program_files: Some(program_files),
            program_files_x86: Some(root.join("Program Files (x86)")),
            local_app_data: Some(local_app_data),
            system_drive: Some(system_drive),
        };
        let rows = detect_shells_in(
            &AppConfig::default(),
            &environment,
            Duration::from_millis(50),
        );
        let ps5 = rows
            .iter()
            .find(|row| row.kind == ShellKind::PowerShell5)
            .unwrap();
        assert!(ps5.detected);
        assert_eq!(ps5.source, ShellDetectionSource::KnownPath);
        assert!(ps5.executable.as_ref().unwrap().ends_with("powershell.exe"));
        let ps7 = rows
            .iter()
            .find(|row| row.kind == ShellKind::PowerShell7)
            .unwrap();
        assert!(ps7.detected);
        assert_eq!(ps7.source, ShellDetectionSource::KnownPath);
        assert!(ps7.executable.as_ref().unwrap().ends_with("pwsh.exe"));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_wsl_launcher_is_marked_and_never_probed() {
        let root = temp_detection_root("wsl");
        let system_root = root.join("Windows");
        let wsl = system_root.join("System32/bash.exe");
        write_fake_executable(&wsl, "");
        let environment = DetectionEnvironment {
            path: Some(OsString::new()),
            system_root: Some(system_root),
            program_files: Some(root.join("Program Files")),
            program_files_x86: Some(root.join("Program Files (x86)")),
            local_app_data: Some(root.join("LocalAppData")),
            system_drive: Some(root.join("C")),
        };
        let rows = detect_shells_in(
            &AppConfig::default(),
            &environment,
            Duration::from_millis(50),
        );
        let bash = rows.iter().find(|row| row.kind == ShellKind::Bash).unwrap();
        assert!(bash.detected);
        assert!(bash.wsl);
        assert_eq!(bash.source, ShellDetectionSource::Wsl);
        assert_eq!(bash.version, None, "WSL must not be launched for a probe");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn unix_known_candidates_cover_the_standard_bin_directories() {
        let candidates = known_candidates(&ShellKind::Zsh, &empty_environment());
        let values = candidates
            .iter()
            .map(|(path, wsl)| (path.clone(), *wsl))
            .collect::<Vec<_>>();
        assert!(values.contains(&(PathBuf::from("/bin/zsh"), false)));
        assert!(values.contains(&(PathBuf::from("/usr/bin/zsh"), false)));
        assert!(values.contains(&(PathBuf::from("/usr/local/bin/zsh"), false)));
    }

    #[test]
    fn probe_failure_only_marks_the_version_unknown() {
        let root = temp_detection_root("probe-failure");
        let executable = root.join(executable_name_for_test(&ShellKind::Bash));
        // A non-executable/empty file is still a detected path.
        fs::write(&executable, b"").unwrap();
        let mut config = AppConfig::default();
        config.shells.executables.insert("bash".into(), executable);
        let rows = detect_shells_in(&config, &empty_environment(), Duration::from_millis(200));
        let bash = rows.iter().find(|row| row.kind == ShellKind::Bash).unwrap();
        assert!(bash.detected);
        assert_eq!(bash.version, None);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn version_probe_reads_the_first_line_and_times_out() {
        let root = temp_detection_root("probe-unix");
        let version = root.join("version-sh");
        write_fake_executable(&version, "#!/bin/sh\necho '7.4.1'\necho second-line\n");
        assert_eq!(
            probe_version(&version, &ShellKind::PowerShell7, Duration::from_secs(2)),
            Some("7.4.1".into())
        );

        let slow = root.join("slow-sh");
        write_fake_executable(&slow, "#!/bin/sh\nsleep 5\n");
        let started = Instant::now();
        assert_eq!(
            probe_version(&slow, &ShellKind::Bash, Duration::from_millis(150)),
            None
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "probe must respect its timeout"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_version_probe_reads_output_and_times_out() {
        let root = temp_detection_root("probe-win");
        let version = root.join("version.cmd");
        write_fake_executable(&version, "@echo off\r\necho 7.4.1\r\necho second-line\r\n");
        assert_eq!(
            probe_version(&version, &ShellKind::PowerShell7, Duration::from_secs(2)),
            Some("7.4.1".into())
        );

        let slow = root.join("slow.cmd");
        write_fake_executable(&slow, "@echo off\r\nping -n 6 127.0.0.1 >nul\r\n");
        let started = Instant::now();
        assert_eq!(
            probe_version(&slow, &ShellKind::Bash, Duration::from_millis(150)),
            None
        );
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "probe must respect its timeout"
        );
        let _ = fs::remove_dir_all(root);
    }
}
