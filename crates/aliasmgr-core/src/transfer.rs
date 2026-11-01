use crate::{
    error::AliasError,
    model::{record_checksum, AliasRecord, PathMode, ShellKind, TargetType},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
use uuid::Uuid;

pub const FORMAT_VERSION: u32 = 1;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFile { pub format_version: u32, pub exported_at: String, pub exported_by_version: String, pub aliases: Vec<AliasRecord> }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy { Skip, Overwrite, Rename, Ask }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub imported: Vec<String>, pub skipped: Vec<String>, pub warnings: Vec<String>, pub unsupported: Vec<String>,
    #[serde(skip)] pub accepted_records: Vec<AliasRecord>,
}
impl ImportReport { pub fn into_public(self) -> Self { Self { accepted_records: Vec::new(), ..self } } }

/// Environment keys matching any of these substrings (case-insensitive) are
/// treated as secrets: never exported, never imported, never logged.
const SENSITIVE_ENVIRONMENT_PARTS: [&str; 5] = ["PASSWORD", "TOKEN", "SECRET", "API_KEY", "APIKEY"];

fn empty_report() -> ImportReport { ImportReport { imported: vec![], skipped: vec![], warnings: vec![], unsupported: vec![], accepted_records: vec![] } }

fn is_sensitive_environment_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SENSITIVE_ENVIRONMENT_PARTS.iter().any(|part| upper.contains(part))
}

fn filter_sensitive_environment(environment: &mut BTreeMap<String, String>, warnings: &mut Vec<String>) {
    let keys: Vec<_> = environment.keys().filter(|key| is_sensitive_environment_key(key)).cloned().collect();
    for key in keys { environment.remove(&key); warnings.push(format!("sensitive environment variable omitted: {key}")); }
}

/// Exported records must never carry secrets. Removing a key changes the
/// record, so the checksum is recomputed before the record leaves the process.
fn sanitized_for_export(aliases: &[AliasRecord]) -> Result<Vec<AliasRecord>, AliasError> {
    aliases.iter().cloned().map(|mut alias| {
        alias.environment.retain(|key, _| !is_sensitive_environment_key(key));
        alias.record_checksum = record_checksum(&alias)?;
        Ok(alias)
    }).collect()
}

pub fn export_toml(path: &Path, aliases: &[AliasRecord]) -> Result<(), AliasError> {
    let file = ExportFile { format_version: FORMAT_VERSION, exported_at: Utc::now().to_rfc3339(), exported_by_version: env!("CARGO_PKG_VERSION").into(), aliases: sanitized_for_export(aliases)? };
    fs::write(path, toml::to_string_pretty(&file).map_err(|error| AliasError::Config(error.to_string()))?)?;
    Ok(())
}
pub fn read_toml(path: &Path) -> Result<Vec<AliasRecord>, AliasError> { let file: ExportFile = toml::from_str(&fs::read_to_string(path)?).map_err(|error| AliasError::Config(error.to_string()))?; if file.format_version != FORMAT_VERSION { return Err(AliasError::Config(format!("unsupported format_version: {}", file.format_version))); } Ok(file.aliases) }
pub fn import_toml(path: &Path, existing: &BTreeMap<String, AliasRecord>, strategy: ConflictStrategy) -> Result<ImportReport, AliasError> { import_records(read_toml(path)?, existing, strategy).map(|report| report.into_public()) }

pub fn export_json(path: &Path, aliases: &[AliasRecord]) -> Result<(), AliasError> {
    let file = ExportFile { format_version: FORMAT_VERSION, exported_at: Utc::now().to_rfc3339(), exported_by_version: env!("CARGO_PKG_VERSION").into(), aliases: sanitized_for_export(aliases)? };
    fs::write(path, serde_json::to_string_pretty(&file)?)?;
    Ok(())
}
pub fn read_json(path: &Path) -> Result<Vec<AliasRecord>, AliasError> { let file: ExportFile = serde_json::from_str(&fs::read_to_string(path)?)?; if file.format_version != FORMAT_VERSION { return Err(AliasError::Config(format!("unsupported format_version: {}", file.format_version))); } Ok(file.aliases) }
pub fn import_json(path: &Path, existing: &BTreeMap<String, AliasRecord>, strategy: ConflictStrategy) -> Result<ImportReport, AliasError> { import_records(read_json(path)?, existing, strategy).map(|report| report.into_public()) }

fn contains_powershell(shells: &[ShellKind]) -> bool {
    shells.iter().any(|shell| matches!(shell, ShellKind::PowerShell5 | ShellKind::PowerShell7))
}

/// Mirrors the storage-level conflict rule: an exact name always conflicts, and
/// incoming PowerShell aliases additionally conflict with existing PowerShell
/// aliases whose name differs only by case.
fn conflicts_with_existing(name: &str, powershell: bool, existing: &BTreeMap<String, AliasRecord>) -> bool {
    if existing.contains_key(name) { return true; }
    powershell && existing.values().any(|record| record.name.ne(name) && record.name.eq_ignore_ascii_case(name) && contains_powershell(&record.shells))
}

fn conflicts_within_file(name: &str, powershell: bool, accepted: &[AliasRecord]) -> bool {
    accepted.iter().any(|record| record.name == name || (powershell && record.name.ne(name) && record.name.eq_ignore_ascii_case(name) && contains_powershell(&record.shells)))
}

pub fn import_records(mut aliases: Vec<AliasRecord>, existing: &BTreeMap<String, AliasRecord>, strategy: ConflictStrategy) -> Result<ImportReport, AliasError> {
    let mut report = empty_report();
    let mut accepted: Vec<AliasRecord> = Vec::new();
    for mut alias in aliases.drain(..) {
        if alias.advanced_shell_mode || alias.target_type == TargetType::RawShellCommand { report.unsupported.push(alias.name); continue; }
        filter_sensitive_environment(&mut alias.environment, &mut report.warnings);
        if alias.path_mode == PathMode::RelativeToWorkingDirectory { report.warnings.push(format!("relative path requires a selected base directory: {}", alias.name)); report.skipped.push(alias.name); continue; }
        let powershell = contains_powershell(&alias.shells);
        if conflicts_with_existing(&alias.name, powershell, existing) {
            match strategy {
                ConflictStrategy::Skip | ConflictStrategy::Ask => { report.skipped.push(alias.name); continue; }
                ConflictStrategy::Rename => {
                    let original = alias.name.clone();
                    let mut candidate = format!("{original}_imported");
                    while conflicts_with_existing(&candidate, powershell, existing) || conflicts_within_file(&candidate, powershell, &accepted) {
                        candidate.push_str("_imported");
                    }
                    alias.name = candidate;
                    report.warnings.push(format!("renamed due to name conflict: {original} -> {}", alias.name));
                }
                ConflictStrategy::Overwrite => {}
            }
        }
        if conflicts_within_file(&alias.name, powershell, &accepted) { report.skipped.push(alias.name); continue; }
        alias.id = Uuid::new_v4();
        alias.record_checksum = record_checksum(&alias)?;
        report.imported.push(alias.name.clone()); accepted.push(alias);
    }
    report.accepted_records = accepted;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AliasRecord;
    #[test]
    fn export_import_preserves_fields_and_filters_sensitive_values() { let root = std::env::temp_dir().join(format!("aliasmgr-transfer-{}.json", std::process::id())); let mut alias = AliasRecord { name: "gs".into(), executable: "git".into(), ..Default::default() }; alias.environment.insert("TOKEN".into(), "secret".into()); alias.environment.insert("MODE".into(), "test".into()); export_json(&root, &[alias.clone()]).unwrap(); let report = import_json(&root, &BTreeMap::new(), ConflictStrategy::Ask).unwrap(); assert_eq!(report.imported, vec!["gs"]); assert!(report.warnings.is_empty()); let _ = fs::remove_file(root); }
    #[test]
    fn toml_round_trip_uses_same_metadata_and_conflicts() { let root = std::env::temp_dir().join(format!("aliasmgr-transfer-{}.toml", std::process::id())); let alias = AliasRecord { name: "gs".into(), executable: "git".into(), ..Default::default() }; export_toml(&root, std::slice::from_ref(&alias)).unwrap(); let mut existing = BTreeMap::new(); existing.insert("gs".into(), alias); let report = import_toml(&root, &existing, ConflictStrategy::Skip).unwrap(); assert_eq!(report.skipped, vec!["gs"]); let _ = fs::remove_file(root); }
    #[test]
    fn rejects_unknown_or_missing_format_version() { let root = std::env::temp_dir().join(format!("aliasmgr-transfer-invalid-{}.json", std::process::id())); fs::write(&root, "{\"aliases\":[]}").unwrap(); assert!(import_json(&root, &BTreeMap::new(), ConflictStrategy::Ask).is_err()); let _ = fs::remove_file(root); }

    fn unique_path(label: &str, extension: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "aliasmgr-transfer-{label}-{}-{}.{extension}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn alias_with_secret(name: &str) -> AliasRecord {
        let mut alias = AliasRecord { name: name.into(), executable: "git".into(), ..Default::default() };
        alias.environment.insert("TOKEN".into(), "secret".into());
        alias.environment.insert("MODE".into(), "test".into());
        alias
    }

    #[test]
    fn exports_never_contain_sensitive_environment_values_and_stay_importable() {
        let json = unique_path("sanitize", "json");
        let toml_path = unique_path("sanitize", "toml");
        let alias = alias_with_secret("gs");
        export_json(&json, std::slice::from_ref(&alias)).unwrap();
        export_toml(&toml_path, std::slice::from_ref(&alias)).unwrap();
        for path in [&json, &toml_path] {
            let text = fs::read_to_string(path).unwrap();
            assert!(!text.contains("secret"), "{} leaked a sensitive value", path.display());
            assert!(!text.to_ascii_uppercase().contains("TOKEN"), "{} leaked a sensitive key", path.display());
            assert!(text.contains("MODE"));
        }
        for records in [read_json(&json).unwrap(), read_toml(&toml_path).unwrap()] {
            assert_eq!(records[0].record_checksum, record_checksum(&records[0]).unwrap());
            let report = import_records(records, &BTreeMap::new(), ConflictStrategy::Ask).unwrap();
            assert_eq!(report.imported, vec!["gs"]);
            assert_eq!(report.accepted_records[0].environment.get("MODE").map(String::as_str), Some("test"));
            assert!(!report.accepted_records[0].environment.contains_key("TOKEN"));
        }
        let _ = fs::remove_file(json);
        let _ = fs::remove_file(toml_path);
    }

    #[test]
    fn ask_strategy_skips_powershell_case_fold_conflicts_for_preview_and_confirm() {
        let database = crate::storage::Database::open_in_memory().unwrap();
        database.insert_alias(&AliasRecord { name: "Foo".into(), executable: "tool".into(), shells: vec![ShellKind::PowerShell5], ..Default::default() }).unwrap();
        let existing: BTreeMap<_, _> = database.list_aliases().unwrap().into_iter().map(|alias| (alias.name.clone(), alias)).collect();
        let incoming = AliasRecord { name: "foo".into(), executable: "tool".into(), shells: vec![ShellKind::PowerShell5], ..Default::default() };
        let preview = import_records(vec![incoming.clone()], &existing, ConflictStrategy::Ask).unwrap();
        assert!(preview.imported.is_empty());
        assert_eq!(preview.skipped, vec!["foo"]);
        assert!(preview.accepted_records.is_empty());
        let confirmed = import_records(vec![incoming], &existing, ConflictStrategy::Ask).unwrap();
        assert_eq!(confirmed.skipped, vec!["foo"]);
        database.insert_aliases_atomic(&confirmed.accepted_records).unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 1);
    }

    #[test]
    fn rename_strategy_appends_suffix_to_case_fold_conflicts() {
        let existing_alias = AliasRecord { name: "Foo".into(), executable: "tool".into(), shells: vec![ShellKind::PowerShell5], ..Default::default() };
        let existing = BTreeMap::from([("Foo".to_string(), existing_alias)]);
        let incoming = AliasRecord { name: "foo".into(), executable: "tool".into(), shells: vec![ShellKind::PowerShell5], ..Default::default() };
        let report = import_records(vec![incoming], &existing, ConflictStrategy::Rename).unwrap();
        assert_eq!(report.imported, vec!["foo_imported"]);
        assert_eq!(report.accepted_records[0].record_checksum, record_checksum(&report.accepted_records[0]).unwrap());
    }

    #[test]
    fn powershell_case_fold_duplicates_within_one_import_file_are_skipped() {
        let first = AliasRecord { name: "Build".into(), executable: "tool".into(), shells: vec![ShellKind::PowerShell7], ..Default::default() };
        let second = AliasRecord { name: "build".into(), executable: "tool".into(), shells: vec![ShellKind::PowerShell7], ..Default::default() };
        let report = import_records(vec![first, second], &BTreeMap::new(), ConflictStrategy::Rename).unwrap();
        assert_eq!(report.imported, vec!["Build"]);
        assert_eq!(report.skipped, vec!["build"]);
    }

    #[test]
    fn pure_bash_case_variants_still_import_together() {
        let first = AliasRecord { name: "CM".into(), executable: "tool".into(), ..Default::default() };
        let second = AliasRecord { name: "cm".into(), executable: "tool".into(), ..Default::default() };
        let report = import_records(vec![first, second], &BTreeMap::new(), ConflictStrategy::Ask).unwrap();
        assert_eq!(report.imported, vec!["CM", "cm"]);
        assert!(report.skipped.is_empty());
    }
}
