use aliasmgr_core::{model::{AliasRecord, ManagedNameSet, ShellKind}, shells::powershell};

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
