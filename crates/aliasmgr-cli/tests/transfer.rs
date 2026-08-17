use aliasmgr_core::{model::AliasRecord, transfer::{export_json, import_json, ConflictStrategy}};
use aliasmgr_cli::commands;
use std::collections::BTreeMap;

#[test]
fn confirmed_cli_import_persists_new_records_but_preview_does_not_write() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-import-{}", std::process::id()));
    let config = root.join("config");
    let file = root.join("aliases.json");
    let alias = AliasRecord { name: "imported".into(), executable: "git".into(), ..Default::default() };
    std::fs::create_dir_all(&root).unwrap();
    export_json(&file, std::slice::from_ref(&alias)).unwrap();
    let preview = commands::import_preview(Some(config.to_str().unwrap()), file.to_str().unwrap()).unwrap();
    assert_eq!(preview.imported, vec!["imported"]);
    assert!(!config.join("aliases.db").exists());
    let confirmed = commands::import_confirm(Some(config.to_str().unwrap()), file.to_str().unwrap()).unwrap();
    assert_eq!(confirmed.imported, vec!["imported"]);
    assert_eq!(commands::get(Some(config.to_str().unwrap()), "imported").unwrap().unwrap().name, "imported");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn transfer_round_trip_uses_explicit_format_and_report() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-transfer-{}.json", std::process::id()));
    let alias = AliasRecord { name: "gs".into(), executable: "git".into(), ..Default::default() };
    export_json(&root, std::slice::from_ref(&alias)).unwrap();
    let report = import_json(&root, &BTreeMap::new(), ConflictStrategy::Ask).unwrap();
    assert_eq!(report.imported, vec!["gs"]);
    let _ = std::fs::remove_file(root);
}
