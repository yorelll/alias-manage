use aliasmgr_core::{model::{AliasRecord, ManagedNameSet}, shells::bash};

#[test]
fn bash_generated_script_preserves_arguments_and_quotes() {
    let alias = AliasRecord { name: "cm".into(), executable: "python3".into(), fixed_args: vec!["script.py".into()], ..Default::default() };
    let script = bash::render(&alias, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
    assert!(script.contains("\"$@\""));
    assert!(!script.contains("eval"));
}
