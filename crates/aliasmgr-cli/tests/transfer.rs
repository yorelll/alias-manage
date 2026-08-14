use aliasmgr_core::{model::AliasRecord, transfer::{export_json, import_json, ConflictStrategy}};
use std::collections::BTreeMap;

#[test]
fn transfer_round_trip_uses_explicit_format_and_report() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-transfer-{}.json", std::process::id()));
    let alias = AliasRecord { name: "gs".into(), executable: "git".into(), ..Default::default() };
    export_json(&root, std::slice::from_ref(&alias)).unwrap();
    let report = import_json(&root, &BTreeMap::new(), ConflictStrategy::Ask).unwrap();
    assert_eq!(report.imported, vec!["gs"]);
    let _ = std::fs::remove_file(root);
}
