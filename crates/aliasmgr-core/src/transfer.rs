use crate::{
    error::AliasError,
    model::{record_checksum, AliasRecord, PathMode, ShellKind, TargetType},
    validation::validate_alias,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
use uuid::Uuid;

pub const FORMAT_VERSION: u32 = 1;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFile {
    pub format_version: u32,
    pub exported_at: String,
    pub exported_by_version: String,
    pub aliases: Vec<AliasRecord>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    Skip,
    Overwrite,
    Rename,
    Ask,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
    pub unsupported: Vec<String>,
    #[serde(skip)]
    pub accepted_records: Vec<AliasRecord>,
}
impl ImportReport {
    pub fn into_public(self) -> Self {
        Self {
            accepted_records: Vec::new(),
            ..self
        }
    }
}

/// Environment keys matching any of these substrings (case-insensitive) are
/// treated as secrets: never exported, never imported, never logged.
const SENSITIVE_ENVIRONMENT_PARTS: [&str; 5] = ["PASSWORD", "TOKEN", "SECRET", "API_KEY", "APIKEY"];

fn empty_report() -> ImportReport {
    ImportReport {
        imported: vec![],
        skipped: vec![],
        warnings: vec![],
        unsupported: vec![],
        accepted_records: vec![],
    }
}

fn is_sensitive_environment_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SENSITIVE_ENVIRONMENT_PARTS
        .iter()
        .any(|part| upper.contains(part))
}

/// Warnings echo names and environment keys into terminals and logs. Control
/// characters (ANSI escapes, newlines) are stripped and the display is capped,
/// so a crafted import file cannot inject terminal sequences or split lines.
const MAX_WARNING_NAME_CHARS: usize = 128;

fn sanitized_name(name: &str) -> String {
    let filtered: Vec<char> = name
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let mut cleaned: String = filtered.iter().take(MAX_WARNING_NAME_CHARS).collect();
    if cleaned.is_empty() {
        cleaned.push_str("<unnamed>");
    }
    if filtered.len() > MAX_WARNING_NAME_CHARS {
        cleaned.push('…');
    }
    cleaned
}

fn filter_sensitive_environment(
    environment: &mut BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) {
    let keys: Vec<_> = environment
        .keys()
        .filter(|key| is_sensitive_environment_key(key))
        .cloned()
        .collect();
    for key in keys {
        environment.remove(&key);
        warnings.push(format!(
            "sensitive environment variable omitted: {}",
            sanitized_name(&key)
        ));
    }
}

/// Exported records must never carry secrets. Removing a key changes the
/// record, so the checksum is recomputed before the record leaves the process.
fn sanitized_for_export(aliases: &[AliasRecord]) -> Result<Vec<AliasRecord>, AliasError> {
    aliases
        .iter()
        .cloned()
        .map(|mut alias| {
            alias
                .environment
                .retain(|key, _| !is_sensitive_environment_key(key));
            alias.record_checksum = record_checksum(&alias)?;
            Ok(alias)
        })
        .collect()
}

pub fn export_toml(path: &Path, aliases: &[AliasRecord]) -> Result<(), AliasError> {
    let file = ExportFile {
        format_version: FORMAT_VERSION,
        exported_at: Utc::now().to_rfc3339(),
        exported_by_version: env!("CARGO_PKG_VERSION").into(),
        aliases: sanitized_for_export(aliases)?,
    };
    fs::write(
        path,
        toml::to_string_pretty(&file).map_err(|error| AliasError::Config(error.to_string()))?,
    )?;
    Ok(())
}
pub fn read_toml(path: &Path) -> Result<Vec<AliasRecord>, AliasError> {
    read_toml_str(&fs::read_to_string(path)?)
}
/// Parses an in-memory TOML export. Mirrors `read_toml` so content imports
/// (GUI paste/drag-drop) honour the same `FORMAT_VERSION` check as files.
pub fn read_toml_str(content: &str) -> Result<Vec<AliasRecord>, AliasError> {
    let file: ExportFile =
        toml::from_str(content).map_err(|error| AliasError::Config(error.to_string()))?;
    export_file_records(file)
}
pub fn import_toml(
    path: &Path,
    existing: &BTreeMap<String, AliasRecord>,
    strategy: ConflictStrategy,
) -> Result<ImportReport, AliasError> {
    import_records(read_toml(path)?, existing, strategy).map(|report| report.into_public())
}

pub fn export_json(path: &Path, aliases: &[AliasRecord]) -> Result<(), AliasError> {
    let file = ExportFile {
        format_version: FORMAT_VERSION,
        exported_at: Utc::now().to_rfc3339(),
        exported_by_version: env!("CARGO_PKG_VERSION").into(),
        aliases: sanitized_for_export(aliases)?,
    };
    fs::write(path, serde_json::to_string_pretty(&file)?)?;
    Ok(())
}
pub fn read_json(path: &Path) -> Result<Vec<AliasRecord>, AliasError> {
    read_json_str(&fs::read_to_string(path)?)
}
/// Parses an in-memory JSON export. Mirrors `read_json` so content imports
/// (GUI paste/drag-drop) honour the same `FORMAT_VERSION` check as files.
pub fn read_json_str(content: &str) -> Result<Vec<AliasRecord>, AliasError> {
    export_file_records(serde_json::from_str(content)?)
}

/// Shared `FORMAT_VERSION` gate for both file and in-memory readers.
fn export_file_records(file: ExportFile) -> Result<Vec<AliasRecord>, AliasError> {
    if file.format_version != FORMAT_VERSION {
        return Err(AliasError::Config(format!(
            "unsupported format_version: {}",
            file.format_version
        )));
    }
    Ok(file.aliases)
}
pub fn import_json(
    path: &Path,
    existing: &BTreeMap<String, AliasRecord>,
    strategy: ConflictStrategy,
) -> Result<ImportReport, AliasError> {
    import_records(read_json(path)?, existing, strategy).map(|report| report.into_public())
}

fn contains_powershell(shells: &[ShellKind]) -> bool {
    shells
        .iter()
        .any(|shell| matches!(shell, ShellKind::PowerShell5 | ShellKind::PowerShell7))
}

/// Mirrors the storage-level conflict rule: an exact name always conflicts, and
/// incoming PowerShell aliases additionally conflict with existing PowerShell
/// aliases whose name differs only by case. Folding uses `str::to_lowercase`,
/// exactly like the `name_folded` column, so non-ASCII names agree with storage.
fn conflicts_with_existing(
    name: &str,
    powershell: bool,
    existing: &BTreeMap<String, AliasRecord>,
) -> bool {
    if existing.contains_key(name) {
        return true;
    }
    if !powershell {
        return false;
    }
    let folded = name.to_lowercase();
    existing.values().any(|record| {
        record.name != name
            && record.name.to_lowercase() == folded
            && contains_powershell(&record.shells)
    })
}

fn conflicts_within_file(name: &str, powershell: bool, accepted: &[AliasRecord]) -> bool {
    if accepted.iter().any(|record| record.name == name) {
        return true;
    }
    if !powershell {
        return false;
    }
    let folded = name.to_lowercase();
    accepted
        .iter()
        .any(|record| record.name.to_lowercase() == folded && contains_powershell(&record.shells))
}

pub fn import_records(
    mut aliases: Vec<AliasRecord>,
    existing: &BTreeMap<String, AliasRecord>,
    strategy: ConflictStrategy,
) -> Result<ImportReport, AliasError> {
    let mut report = empty_report();
    let mut accepted: Vec<AliasRecord> = Vec::new();
    for mut alias in aliases.drain(..) {
        if alias.advanced_shell_mode || alias.target_type == TargetType::RawShellCommand {
            report.unsupported.push(alias.name);
            continue;
        }
        filter_sensitive_environment(&mut alias.environment, &mut report.warnings);
        if alias.path_mode == PathMode::RelativeToWorkingDirectory {
            report.warnings.push(format!(
                "relative path requires a selected base directory: {}",
                sanitized_name(&alias.name)
            ));
            report.skipped.push(alias.name);
            continue;
        }
        let powershell = contains_powershell(&alias.shells);
        if conflicts_with_existing(&alias.name, powershell, existing) {
            match strategy {
                ConflictStrategy::Skip | ConflictStrategy::Ask => {
                    report.skipped.push(alias.name);
                    continue;
                }
                ConflictStrategy::Rename => {
                    let original = alias.name.clone();
                    let mut candidate = format!("{original}_imported");
                    while conflicts_with_existing(&candidate, powershell, existing)
                        || conflicts_within_file(&candidate, powershell, &accepted)
                    {
                        candidate.push_str("_imported");
                    }
                    alias.name = candidate;
                    report.warnings.push(format!(
                        "renamed due to name conflict: {} -> {}",
                        sanitized_name(&original),
                        sanitized_name(&alias.name)
                    ));
                }
                ConflictStrategy::Overwrite => {}
            }
        }
        if conflicts_within_file(&alias.name, powershell, &accepted) {
            report.skipped.push(alias.name);
            continue;
        }
        // Imports must satisfy the same rules as `add`/`create`; otherwise a
        // file could persist names the CLI itself rejects (invalid characters
        // or PowerShell-reserved names). The warning carries only the error
        // text, never environment values.
        if let Err(error) = validate_alias(&alias) {
            report.warnings.push(format!(
                "invalid alias rejected: {} ({error})",
                sanitized_name(&alias.name)
            ));
            report.skipped.push(alias.name);
            continue;
        }
        alias.id = Uuid::new_v4();
        alias.record_checksum = record_checksum(&alias)?;
        report.imported.push(alias.name.clone());
        accepted.push(alias);
    }
    report.accepted_records = accepted;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AliasRecord;
    #[test]
    fn export_import_preserves_fields_and_filters_sensitive_values() {
        let root =
            std::env::temp_dir().join(format!("aliasmgr-transfer-{}.json", std::process::id()));
        let mut alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        };
        alias.environment.insert("TOKEN".into(), "secret".into());
        alias.environment.insert("MODE".into(), "test".into());
        export_json(&root, &[alias.clone()]).unwrap();
        let report = import_json(&root, &BTreeMap::new(), ConflictStrategy::Ask).unwrap();
        assert_eq!(report.imported, vec!["gs"]);
        assert!(report.warnings.is_empty());
        let _ = fs::remove_file(root);
    }
    #[test]
    fn toml_round_trip_uses_same_metadata_and_conflicts() {
        let root =
            std::env::temp_dir().join(format!("aliasmgr-transfer-{}.toml", std::process::id()));
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        };
        export_toml(&root, std::slice::from_ref(&alias)).unwrap();
        let mut existing = BTreeMap::new();
        existing.insert("gs".into(), alias);
        let report = import_toml(&root, &existing, ConflictStrategy::Skip).unwrap();
        assert_eq!(report.skipped, vec!["gs"]);
        let _ = fs::remove_file(root);
    }
    #[test]
    fn rejects_unknown_or_missing_format_version() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-transfer-invalid-{}.json",
            std::process::id()
        ));
        fs::write(&root, "{\"aliases\":[]}").unwrap();
        assert!(import_json(&root, &BTreeMap::new(), ConflictStrategy::Ask).is_err());
        let _ = fs::remove_file(root);
    }

    #[test]
    fn string_readers_accept_in_memory_exports_and_honour_format_version() {
        let file = ExportFile {
            format_version: FORMAT_VERSION,
            exported_at: "2026-11-01T00:00:00Z".into(),
            exported_by_version: "0.1.0".into(),
            aliases: vec![AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                ..Default::default()
            }],
        };
        let json = serde_json::to_string(&file).unwrap();
        let toml_text = toml::to_string_pretty(&file).unwrap();
        assert_eq!(read_json_str(&json).unwrap()[0].name, "gs");
        assert_eq!(read_toml_str(&toml_text).unwrap()[0].name, "gs");

        let unsupported = ExportFile {
            format_version: FORMAT_VERSION + 1,
            ..file
        };
        assert!(read_json_str(&serde_json::to_string(&unsupported).unwrap()).is_err());
        assert!(read_toml_str(&toml::to_string_pretty(&unsupported).unwrap()).is_err());
    }

    fn unique_path(label: &str, extension: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "aliasmgr-transfer-{label}-{}-{}.{extension}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn alias_with_secret(name: &str) -> AliasRecord {
        let mut alias = AliasRecord {
            name: name.into(),
            executable: "git".into(),
            ..Default::default()
        };
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
            assert!(
                !text.contains("secret"),
                "{} leaked a sensitive value",
                path.display()
            );
            assert!(
                !text.to_ascii_uppercase().contains("TOKEN"),
                "{} leaked a sensitive key",
                path.display()
            );
            assert!(text.contains("MODE"));
        }
        for records in [read_json(&json).unwrap(), read_toml(&toml_path).unwrap()] {
            assert_eq!(
                records[0].record_checksum,
                record_checksum(&records[0]).unwrap()
            );
            let report = import_records(records, &BTreeMap::new(), ConflictStrategy::Ask).unwrap();
            assert_eq!(report.imported, vec!["gs"]);
            assert_eq!(
                report.accepted_records[0]
                    .environment
                    .get("MODE")
                    .map(String::as_str),
                Some("test")
            );
            assert!(!report.accepted_records[0].environment.contains_key("TOKEN"));
        }
        let _ = fs::remove_file(json);
        let _ = fs::remove_file(toml_path);
    }

    #[test]
    fn ask_strategy_skips_powershell_case_fold_conflicts_for_preview_and_confirm() {
        let database = crate::storage::Database::open_in_memory().unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "Foo".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::PowerShell5],
                ..Default::default()
            })
            .unwrap();
        let existing: BTreeMap<_, _> = database
            .list_aliases()
            .unwrap()
            .into_iter()
            .map(|alias| (alias.name.clone(), alias))
            .collect();
        let incoming = AliasRecord {
            name: "foo".into(),
            executable: "tool".into(),
            shells: vec![ShellKind::PowerShell5],
            ..Default::default()
        };
        let preview =
            import_records(vec![incoming.clone()], &existing, ConflictStrategy::Ask).unwrap();
        assert!(preview.imported.is_empty());
        assert_eq!(preview.skipped, vec!["foo"]);
        assert!(preview.accepted_records.is_empty());
        let confirmed = import_records(vec![incoming], &existing, ConflictStrategy::Ask).unwrap();
        assert_eq!(confirmed.skipped, vec!["foo"]);
        database
            .insert_aliases_atomic(&confirmed.accepted_records)
            .unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 1);
    }

    #[test]
    fn non_ascii_case_fold_conflicts_are_skipped_in_preview_and_confirm() {
        let database = crate::storage::Database::open_in_memory().unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "Ä".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::PowerShell5],
                ..Default::default()
            })
            .unwrap();
        let existing: BTreeMap<_, _> = database
            .list_aliases()
            .unwrap()
            .into_iter()
            .map(|alias| (alias.name.clone(), alias))
            .collect();
        let incoming = AliasRecord {
            name: "ä".into(),
            executable: "tool".into(),
            shells: vec![ShellKind::PowerShell5],
            ..Default::default()
        };
        let preview =
            import_records(vec![incoming.clone()], &existing, ConflictStrategy::Ask).unwrap();
        assert!(preview.imported.is_empty());
        assert_eq!(preview.skipped, vec!["ä"]);
        let confirmed = import_records(vec![incoming], &existing, ConflictStrategy::Ask).unwrap();
        assert_eq!(confirmed.skipped, vec!["ä"]);
        assert!(confirmed.accepted_records.is_empty());
        database
            .insert_aliases_atomic(&confirmed.accepted_records)
            .unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 1);
    }

    #[test]
    fn rename_strategy_appends_suffix_to_case_fold_conflicts() {
        let existing_alias = AliasRecord {
            name: "Foo".into(),
            executable: "tool".into(),
            shells: vec![ShellKind::PowerShell5],
            ..Default::default()
        };
        let existing = BTreeMap::from([("Foo".to_string(), existing_alias)]);
        let incoming = AliasRecord {
            name: "foo".into(),
            executable: "tool".into(),
            shells: vec![ShellKind::PowerShell5],
            ..Default::default()
        };
        let report = import_records(vec![incoming], &existing, ConflictStrategy::Rename).unwrap();
        assert_eq!(report.imported, vec!["foo_imported"]);
        assert_eq!(
            report.accepted_records[0].record_checksum,
            record_checksum(&report.accepted_records[0]).unwrap()
        );
    }

    #[test]
    fn powershell_case_fold_duplicates_within_one_import_file_are_skipped() {
        let first = AliasRecord {
            name: "Build".into(),
            executable: "tool".into(),
            shells: vec![ShellKind::PowerShell7],
            ..Default::default()
        };
        let second = AliasRecord {
            name: "build".into(),
            executable: "tool".into(),
            shells: vec![ShellKind::PowerShell7],
            ..Default::default()
        };
        let report = import_records(
            vec![first, second],
            &BTreeMap::new(),
            ConflictStrategy::Rename,
        )
        .unwrap();
        assert_eq!(report.imported, vec!["Build"]);
        assert_eq!(report.skipped, vec!["build"]);
    }

    #[test]
    fn pure_bash_case_variants_still_import_together() {
        let first = AliasRecord {
            name: "CM".into(),
            executable: "tool".into(),
            ..Default::default()
        };
        let second = AliasRecord {
            name: "cm".into(),
            executable: "tool".into(),
            ..Default::default()
        };
        let report =
            import_records(vec![first, second], &BTreeMap::new(), ConflictStrategy::Ask).unwrap();
        assert_eq!(report.imported, vec!["CM", "cm"]);
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn invalid_names_are_skipped_with_a_warning_and_never_accepted() {
        let invalid = AliasRecord {
            name: "1badname".into(),
            executable: "git".into(),
            ..Default::default()
        };
        let valid = AliasRecord {
            name: "good".into(),
            executable: "git".into(),
            ..Default::default()
        };
        let report = import_records(
            vec![invalid, valid],
            &BTreeMap::new(),
            ConflictStrategy::Ask,
        )
        .unwrap();
        assert_eq!(report.imported, vec!["good"]);
        assert_eq!(report.skipped, vec!["1badname"]);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("1badname"));
        assert!(report.warnings[0].contains("invalid alias name"));
        assert_eq!(report.accepted_records.len(), 1);
        assert_eq!(report.accepted_records[0].name, "good");

        let database = crate::storage::Database::open_in_memory().unwrap();
        database
            .insert_aliases_atomic(&report.accepted_records)
            .unwrap();
        let names: Vec<_> = database
            .list_aliases()
            .unwrap()
            .into_iter()
            .map(|alias| alias.name)
            .collect();
        assert_eq!(names, vec!["good"]);
    }

    #[test]
    fn powershell_reserved_names_are_skipped_on_import() {
        let reserved = AliasRecord {
            name: "ls".into(),
            executable: "Get-ChildItem".into(),
            shells: vec![ShellKind::PowerShell5],
            ..Default::default()
        };
        let report = import_records(
            vec![reserved.clone()],
            &BTreeMap::new(),
            ConflictStrategy::Ask,
        )
        .unwrap();
        assert!(report.imported.is_empty());
        assert_eq!(report.skipped, vec!["ls"]);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("name is reserved"));
        assert!(report.accepted_records.is_empty());

        // Preview and confirm run the same evaluation and agree.
        let confirmed =
            import_records(vec![reserved], &BTreeMap::new(), ConflictStrategy::Ask).unwrap();
        assert_eq!(confirmed.skipped, vec!["ls"]);
        assert!(confirmed.accepted_records.is_empty());
    }

    #[test]
    fn import_warnings_strip_control_characters_and_cap_displayed_names() {
        let control = AliasRecord {
            name: "bad\u{1b}[31mname".into(),
            executable: "git".into(),
            ..Default::default()
        };
        let long_name = "a".repeat(200);
        let long = AliasRecord {
            name: long_name.clone(),
            executable: "git".into(),
            ..Default::default()
        };
        let relative = AliasRecord {
            name: "re\u{1b}l".into(),
            executable: "git".into(),
            path_mode: PathMode::RelativeToWorkingDirectory,
            ..Default::default()
        };
        let report = import_records(
            vec![control, long, relative],
            &BTreeMap::new(),
            ConflictStrategy::Ask,
        )
        .unwrap();
        assert_eq!(report.skipped.len(), 3);
        for warning in &report.warnings {
            assert!(
                !warning.chars().any(char::is_control),
                "warning contains control characters: {warning:?}"
            );
        }
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("bad[31mname")));
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("relative path") && warning.contains("rel")));
        assert!(report
            .warnings
            .iter()
            .all(|warning| !warning.contains(&long_name)));
    }

    #[test]
    fn rename_warnings_strip_control_characters() {
        let name = "evil\u{1b}name".to_string();
        let existing = BTreeMap::from([(
            name.clone(),
            AliasRecord {
                name: name.clone(),
                executable: "tool".into(),
                ..Default::default()
            },
        )]);
        let incoming = AliasRecord {
            name,
            executable: "tool".into(),
            ..Default::default()
        };
        let report = import_records(vec![incoming], &existing, ConflictStrategy::Rename).unwrap();
        let warning = report
            .warnings
            .iter()
            .find(|warning| warning.contains("renamed"))
            .expect("rename must emit a warning");
        assert!(
            !warning.chars().any(char::is_control),
            "rename warning contains control characters: {warning:?}"
        );
        assert!(warning.contains("evilname"));
    }
}
