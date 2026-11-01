use crate::{
    error::AliasError,
    model::{Conflict, ShellKind},
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
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
}
