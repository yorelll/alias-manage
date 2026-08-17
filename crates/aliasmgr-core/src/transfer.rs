use crate::{error::AliasError, model::{AliasRecord, TargetType}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

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

fn empty_report() -> ImportReport { ImportReport { imported: vec![], skipped: vec![], warnings: vec![], unsupported: vec![], accepted_records: vec![] } }
pub fn export_toml(path: &Path, aliases: &[AliasRecord]) -> Result<(), AliasError> { fs::write(path, toml::to_string_pretty(&ExportFile { format_version: FORMAT_VERSION, exported_at: Utc::now().to_rfc3339(), exported_by_version: env!("CARGO_PKG_VERSION").into(), aliases: aliases.to_vec() }).map_err(|error| AliasError::Config(error.to_string()))?)?; Ok(()) }
pub fn read_toml(path: &Path) -> Result<Vec<AliasRecord>, AliasError> { let file: ExportFile = toml::from_str(&fs::read_to_string(path)?).map_err(|error| AliasError::Config(error.to_string()))?; if file.format_version != FORMAT_VERSION { return Err(AliasError::Config(format!("unsupported format_version: {}", file.format_version))); } Ok(file.aliases) }
pub fn import_toml(path: &Path, existing: &BTreeMap<String, AliasRecord>, strategy: ConflictStrategy) -> Result<ImportReport, AliasError> { import_records(read_toml(path)?, existing, strategy).map(|report| report.into_public()) }

pub fn export_json(path: &Path, aliases: &[AliasRecord]) -> Result<(), AliasError> { fs::write(path, serde_json::to_string_pretty(&ExportFile { format_version: FORMAT_VERSION, exported_at: Utc::now().to_rfc3339(), exported_by_version: env!("CARGO_PKG_VERSION").into(), aliases: aliases.to_vec() })?)?; Ok(()) }
pub fn read_json(path: &Path) -> Result<Vec<AliasRecord>, AliasError> { let file: ExportFile = serde_json::from_str(&fs::read_to_string(path)?)?; if file.format_version != FORMAT_VERSION { return Err(AliasError::Config(format!("unsupported format_version: {}", file.format_version))); } Ok(file.aliases) }
pub fn import_json(path: &Path, existing: &BTreeMap<String, AliasRecord>, strategy: ConflictStrategy) -> Result<ImportReport, AliasError> { import_records(read_json(path)?, existing, strategy).map(|report| report.into_public()) }

pub fn import_records(mut aliases: Vec<AliasRecord>, existing: &BTreeMap<String, AliasRecord>, strategy: ConflictStrategy) -> Result<ImportReport, AliasError> {
    let mut report = empty_report();
    for mut alias in aliases.drain(..) {
        if alias.advanced_shell_mode || alias.target_type == TargetType::RawShellCommand { report.unsupported.push(alias.name); continue; }
        filter_sensitive_environment(&mut alias.environment, &mut report.warnings);
        if alias.path_mode == crate::model::PathMode::RelativeToWorkingDirectory { report.warnings.push(format!("relative path requires a selected base directory: {}", alias.name)); report.skipped.push(alias.name); continue; }
        if existing.contains_key(&alias.name) { match strategy { ConflictStrategy::Skip | ConflictStrategy::Ask => { report.skipped.push(alias.name); continue; }, ConflictStrategy::Rename => { alias.name = format!("{}_imported", alias.name); }, ConflictStrategy::Overwrite => {} } }
        report.imported.push(alias.name.clone()); report.accepted_records.push(alias);
    }
    Ok(report)
}
fn filter_sensitive_environment(environment: &mut BTreeMap<String, String>, warnings: &mut Vec<String>) { let sensitive = ["PASSWORD", "TOKEN", "SECRET", "API_KEY", "APIKEY"]; let keys: Vec<_> = environment.keys().filter(|key| sensitive.iter().any(|part| key.to_ascii_uppercase().contains(part))).cloned().collect(); for key in keys { environment.remove(&key); warnings.push(format!("sensitive environment variable omitted: {key}")); } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AliasRecord;
    #[test]
    fn export_import_preserves_fields_and_filters_sensitive_values() { let root = std::env::temp_dir().join(format!("aliasmgr-transfer-{}.json", std::process::id())); let mut alias = AliasRecord { name: "gs".into(), executable: "git".into(), ..Default::default() }; alias.environment.insert("TOKEN".into(), "secret".into()); alias.environment.insert("MODE".into(), "test".into()); export_json(&root, &[alias.clone()]).unwrap(); let report = import_json(&root, &BTreeMap::new(), ConflictStrategy::Ask).unwrap(); assert_eq!(report.imported, vec!["gs"]); assert!(!report.warnings.is_empty()); let _ = fs::remove_file(root); }
    #[test]
    fn toml_round_trip_uses_same_metadata_and_conflicts() { let root = std::env::temp_dir().join(format!("aliasmgr-transfer-{}.toml", std::process::id())); let alias = AliasRecord { name: "gs".into(), executable: "git".into(), ..Default::default() }; export_toml(&root, std::slice::from_ref(&alias)).unwrap(); let mut existing = BTreeMap::new(); existing.insert("gs".into(), alias); let report = import_toml(&root, &existing, ConflictStrategy::Skip).unwrap(); assert_eq!(report.skipped, vec!["gs"]); let _ = fs::remove_file(root); }
    #[test]
    fn rejects_unknown_or_missing_format_version() { let root = std::env::temp_dir().join(format!("aliasmgr-transfer-invalid-{}.json", std::process::id())); fs::write(&root, "{\"aliases\":[]}").unwrap(); assert!(import_json(&root, &BTreeMap::new(), ConflictStrategy::Ask).is_err()); let _ = fs::remove_file(root); }
}
