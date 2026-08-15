use aliasmgr_core::{detection::installed_powershells, model::{AliasRecord, ManagedNameSet, ShellKind}, shells::powershell};
use std::{fs, process::Command};

#[cfg(windows)]
#[test]
fn discovers_available_powershell_executables_and_versions() {
    let installations = installed_powershells();
    assert!(installations.iter().any(|item| item.kind == ShellKind::PowerShell5));
    assert!(installations.iter().any(|item| item.kind == ShellKind::PowerShell7));
    assert!(installations.iter().all(|item| !item.executable.as_os_str().is_empty() && item.version.is_some()));
}

#[test]
use std::{fs, process::Command};

fn powershell_command() -> Option<&'static str> {
    if cfg!(windows) { Some("powershell") } else { None }
}

#[test]
fn powershell_versions_render_function_and_argument_array() {
    let alias = AliasRecord { name: "gs".into(), executable: "git.exe".into(), fixed_args: vec!["status".into()], ..Default::default() };
    for shell in [ShellKind::PowerShell5, ShellKind::PowerShell7] {
        let script = powershell::render(&alias, shell, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
        assert!(script.contains("function global:gs"));
        assert!(script.contains("@args"));
        assert!(!script.contains("Invoke-Expression"));
    }
}

#[test]
fn generated_powershell_script_passes_parser_in_windows_powershell_5_1() {
    let Some(command) = powershell_command() else { return; };
    let alias = AliasRecord { name: "gs".into(), executable: "git.exe".into(), fixed_args: vec!["status".into()], ..Default::default() };
    let script = powershell::render(&alias, ShellKind::PowerShell5, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
    let root = std::env::temp_dir().join(format!("aliasmgr-ps-parser-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.ps1");
    fs::write(&path, script).unwrap();
    let path_literal = path.to_string_lossy().replace('\'', "''");
    let probe = format!("& {{ param($path) $tokens = $null; $errors = $null; [void][System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors); if ($errors.Count -gt 0) {{ exit 1 }} }} -path '{}'", path_literal);
    let status = Command::new(command).args(["-NoProfile", "-NonInteractive", "-Command", &probe]).status().unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_files_use_separate_names_for_powershell_versions() {
    let alias = AliasRecord { name: "gs".into(), executable: "git.exe".into(), ..Default::default() };
    let five = powershell::render(&alias, ShellKind::PowerShell5, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
    let seven = powershell::render(&alias, ShellKind::PowerShell7, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
    assert_eq!(five, seven);
}

#[test]
fn powershell_preemption_removes_builtin_alias_before_function_definition() {
    let alias = AliasRecord { name: "ls".into(), executable: "Get-ChildItem".into(), fixed_args: vec!["-Force".into()], ..Default::default() };
    let script = powershell::render(&alias, ShellKind::PowerShell7, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
    assert!(script.contains("Alias:\\ls"));
    assert!(script.contains("Function:\\ls"));
    assert!(script.contains("function global:ls"));
}

#[cfg(windows)]
#[test]
fn powershell_5_1_parser_accepts_generated_file() {
    let alias = AliasRecord { name: "gs".into(), executable: "git.exe".into(), fixed_args: vec!["status".into()], ..Default::default() };
    let script = powershell::render(&alias, ShellKind::PowerShell5, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
    let root = std::env::temp_dir().join(format!("aliasmgr-ps-parser-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.ps1");
    std::fs::write(&path, script).unwrap();
    let path_literal = path.to_string_lossy().replace('\'', "''");
    let probe = format!("& {{ param($path) $tokens = $null; $errors = $null; [void][System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors); if ($errors.Count -gt 0) {{ exit 1 }} }} -path '{}'", path_literal);
    let status = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &probe]).status().unwrap();
    assert!(status.success());
    let _ = std::fs::remove_dir_all(root);
}
