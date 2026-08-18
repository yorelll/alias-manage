use crate::{error::AliasError, model::{Conflict, ShellKind}};
use std::{env, fs, path::{Path, PathBuf}, process::Command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerShellInstallation { pub kind: ShellKind, pub executable: PathBuf, pub version: Option<String> }

pub fn powershell_priority(installations: &[PowerShellInstallation]) -> Vec<PowerShellInstallation> {
    let mut sorted = installations.to_vec();
    sorted.sort_by(|left, right| {
        let kind_order = |kind: &ShellKind| if matches!(kind, ShellKind::PowerShell7) { 0 } else { 1 };
        kind_order(&left.kind).cmp(&kind_order(&right.kind)).then_with(|| right.version.cmp(&left.version)).then_with(|| left.executable.cmp(&right.executable))
    });
    sorted
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionContext { pub explicit: Option<ShellKind>, pub parent_shell: Option<ShellKind>, pub shell_env: Option<String>, pub login_shell: Option<ShellKind> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionResult { pub selected: Option<ShellKind>, pub installed: Vec<ShellKind>, pub source: String }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionSource { pub path: PathBuf, pub line: usize, pub definition: String }

pub fn detect(context: &DetectionContext) -> DetectionResult {
    if let Some(shell) = &context.explicit { return DetectionResult { selected: Some(shell.clone()), installed: installed_shells(), source: "explicit".into() }; }
    if let Some(shell) = &context.parent_shell { return DetectionResult { selected: Some(shell.clone()), installed: installed_shells(), source: "parent".into() }; }
    if let Some(shell) = context.shell_env.as_deref().and_then(parse_shell_name) { return DetectionResult { selected: Some(shell), installed: installed_shells(), source: "shell_env".into() }; }
    if let Some(shell) = &context.login_shell { return DetectionResult { selected: Some(shell.clone()), installed: installed_shells(), source: "login_shell".into() }; }
    let installed = installed_shells(); DetectionResult { selected: installed.first().cloned(), installed, source: "installed".into() }
}

pub fn installed_shells() -> Vec<ShellKind> {
    let mut result = Vec::new();
    for (name, shell) in [("bash", ShellKind::Bash), ("zsh", ShellKind::Zsh), ("powershell", ShellKind::PowerShell5), ("pwsh", ShellKind::PowerShell7)] { if command_on_path(name) { result.push(shell); } }
    result
}

pub fn installed_powershells() -> Vec<PowerShellInstallation> {
    [
        ("powershell", ShellKind::PowerShell5),
        ("pwsh", ShellKind::PowerShell7),
    ].into_iter().filter_map(|(name, kind)| {
        let executable = command_path(name)?;
        let output = Command::new(&executable).args(["-NoProfile", "-NonInteractive", "-Command", "$PSVersionTable.PSVersion.ToString()"]).output().ok()?;
        let version = String::from_utf8(output.stdout).ok()?.trim().to_string();
        Some(PowerShellInstallation { kind, executable, version: (!version.is_empty()).then_some(version) })
    }).collect()
}

pub fn conflicts(name: &str) -> Result<Vec<Conflict>, AliasError> {
    let mut result = Vec::new();
    if ["ls", "cp", "mv", "rm", "cat", "gc", "cd", "pwd"].iter().any(|reserved| reserved.eq_ignore_ascii_case(name)) { result.push(Conflict { name: name.into(), reason: "NameReserved or built-in command".into() }); }
    if command_on_path(name) { result.push(Conflict { name: name.into(), reason: "executable found on PATH".into() }); }
    Ok(result)
}

pub fn find_definition_source(paths: &[PathBuf], name: &str) -> Result<Option<DefinitionSource>, AliasError> {
    for path in paths { if let Ok(content) = fs::read_to_string(path) { for (index, line) in content.lines().enumerate() { if line.contains(name) && (line.contains("alias ") || line.contains("function ")) { return Ok(Some(DefinitionSource { path: path.clone(), line: index + 1, definition: line.into() })); } } } }
    Ok(None)
}

fn parse_shell_name(value: &str) -> Option<ShellKind> { match Path::new(value).file_name()?.to_str()? { "bash" => Some(ShellKind::Bash), "zsh" => Some(ShellKind::Zsh), "powershell" => Some(ShellKind::PowerShell5), "pwsh" => Some(ShellKind::PowerShell7), _ => None } }
fn command_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| env::split_paths(&paths).find_map(|path| {
        let direct = path.join(name);
        if direct.is_file() { return Some(direct); }
        if cfg!(windows) {
            [".exe", ".cmd", ".bat"].iter().map(|ext| path.join(format!("{name}{ext}"))).find(|candidate| candidate.is_file())
        } else { None }
    }))
}

fn command_on_path(name: &str) -> bool { command_path(name).is_some() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powershell_priority_prefers_ps7_then_highest_version() {
        let sorted = powershell_priority(&[
            PowerShellInstallation { kind: ShellKind::PowerShell5, executable: PathBuf::from("powershell.exe"), version: Some("5.1".into()) },
            PowerShellInstallation { kind: ShellKind::PowerShell7, executable: PathBuf::from("pwsh-7.2.exe"), version: Some("7.2".into()) },
            PowerShellInstallation { kind: ShellKind::PowerShell7, executable: PathBuf::from("pwsh-7.4.exe"), version: Some("7.4".into()) },
        ]);
        assert_eq!(sorted[0].version.as_deref(), Some("7.4"));
        assert_eq!(sorted[1].version.as_deref(), Some("7.2"));
        assert_eq!(sorted[2].kind, ShellKind::PowerShell5);
    }

    #[test]
    fn detection_precedence_is_explicit_parent_environment_login_installed() {
        let result = detect(&DetectionContext { explicit: Some(ShellKind::Zsh), parent_shell: Some(ShellKind::Bash), shell_env: Some("/bin/bash".into()), login_shell: Some(ShellKind::PowerShell7) });
        assert_eq!(result.selected, Some(ShellKind::Zsh)); assert_eq!(result.source, "explicit");
        let result = detect(&DetectionContext { explicit: None, parent_shell: Some(ShellKind::Bash), shell_env: Some("/bin/zsh".into()), login_shell: None });
        assert_eq!(result.selected, Some(ShellKind::Bash));
    }
    #[test]
    fn reports_reserved_and_path_conflicts() {
        let result = conflicts("ls").unwrap(); assert!(!result.is_empty()); assert!(result[0].reason.contains("built-in"));
    }
    #[test]
    fn finds_simple_definition_source_with_line_number() {
        let path = std::env::temp_dir().join(format!("aliasmgr-detection-{}.rc", std::process::id())); fs::write(&path, "# comment\nalias gs='git status'\n").unwrap();
        let found = find_definition_source(std::slice::from_ref(&path), "gs").unwrap().unwrap(); assert_eq!(found.line, 2); let _ = fs::remove_file(path);
    }
}
