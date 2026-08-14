use aliasmgr_core::{model::{AliasRecord, ManagedNameSet}, shells::zsh};

#[test]
fn zsh_generated_script_has_preemption_and_argument_forwarding() {
    let alias = AliasRecord { name: "gs".into(), executable: "git".into(), fixed_args: vec!["status".into()], ..Default::default() };
    let script = zsh::render(&alias, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
    assert!(script.contains("unalias 'gs'"));
    assert!(script.contains("\"$@\""));
}
