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
    let preview = commands::import_preview(None, file.to_str().unwrap()).unwrap();
    assert_eq!(preview.imported, vec!["imported"]);
    assert!(!config.join("aliases.db").exists());
    let confirmed = commands::import_confirm(Some(config.to_str().unwrap()), file.to_str().unwrap()).unwrap();
    assert_eq!(confirmed.imported, vec!["imported"]);
    assert_eq!(commands::get(Some(config.to_str().unwrap()), "imported").unwrap().name, "imported");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_import_preview_does_not_write_and_yes_persists() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-import-yes-{}", std::process::id()));
    let config = root.join("config");
    let file = root.join("aliases.json");
    let alias = AliasRecord { name: "imported-yes".into(), executable: "git".into(), ..Default::default() };
    std::fs::create_dir_all(&root).unwrap();
    export_json(&file, std::slice::from_ref(&alias)).unwrap();

    let preview = commands::import_file(Some(config.to_str().unwrap()), file.to_str().unwrap(), false).unwrap();
    assert_eq!(preview.imported, vec!["imported-yes"]);
    assert!(!config.join("aliases.db").exists());
    assert!(commands::get(Some(config.to_str().unwrap()), "imported-yes").is_err());

    let confirmed = commands::import_file(Some(config.to_str().unwrap()), file.to_str().unwrap(), true).unwrap();
    assert_eq!(confirmed.imported, vec!["imported-yes"]);
    assert_eq!(commands::get(Some(config.to_str().unwrap()), "imported-yes").unwrap().name, "imported-yes");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn confirmed_import_recomputes_checksums_after_filtering_sensitive_environment() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-import-sensitive-{}", std::process::id()));
    let config = root.join("config");
    let file = root.join("aliases.json");
    let mut alias = AliasRecord { name: "sensitive".into(), executable: "git".into(), ..Default::default() };
    alias.environment.insert("TOKEN".into(), "secret".into());
    alias.environment.insert("PASSWORD".into(), "hunter2".into());
    alias.environment.insert("MODE".into(), "test".into());
    std::fs::create_dir_all(&root).unwrap();
    export_json(&file, std::slice::from_ref(&alias)).unwrap();

    let report = commands::import_file(Some(config.to_str().unwrap()), file.to_str().unwrap(), true).unwrap();
    assert_eq!(report.imported, vec!["sensitive"]);
    assert!(!report.warnings.is_empty());
    let stored = commands::get(Some(config.to_str().unwrap()), "sensitive").unwrap();
    assert!(!stored.environment.contains_key("TOKEN"));
    assert!(!stored.environment.contains_key("PASSWORD"));
    assert_eq!(stored.environment.get("MODE").map(String::as_str), Some("test"));
    assert_eq!(commands::list(Some(config.to_str().unwrap())).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn within_file_duplicate_names_are_skipped_after_the_first() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-import-duplicate-{}", std::process::id()));
    let config = root.join("config");
    let file = root.join("aliases.json");
    let first = AliasRecord { name: "dup".into(), executable: "git".into(), ..Default::default() };
    let second = AliasRecord { name: "dup".into(), executable: "git".into(), tags: vec!["second".into()], ..Default::default() };
    std::fs::create_dir_all(&root).unwrap();
    export_json(&file, &[first, second]).unwrap();

    let preview = commands::import_file(Some(config.to_str().unwrap()), file.to_str().unwrap(), false).unwrap();
    assert_eq!(preview.imported, vec!["dup"]);
    assert_eq!(preview.skipped, vec!["dup"]);

    let confirmed = commands::import_file(Some(config.to_str().unwrap()), file.to_str().unwrap(), true).unwrap();
    assert_eq!(confirmed.imported, vec!["dup"]);
    assert_eq!(confirmed.skipped, vec!["dup"]);
    assert_eq!(commands::list(Some(config.to_str().unwrap())).unwrap().len(), 1);
    let stored = commands::get(Some(config.to_str().unwrap()), "dup").unwrap();
    assert!(stored.tags.is_empty());
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
