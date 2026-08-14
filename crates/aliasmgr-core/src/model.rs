use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetType {
    NativeExecutable,
    PythonScript,
    PowerShellScript,
    Batch,
    ShellScript,
    JavaJar,
    Cmdlet,
    ChangeDirectory,
    RawShellCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum ShellKind {
    Bash,
    Zsh,
    PowerShell5,
    PowerShell7,
    Fish,
    PosixSh,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PathMode { Absolute, RelativeToWorkingDirectory }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PathOrigin { GuiFilePicker, CliArgument, Import }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AliasRecord {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub target_type: TargetType,
    pub executable: String,
    pub fixed_args: Vec<String>,
    pub pass_args: bool,
    pub working_directory: Option<String>,
    pub environment: BTreeMap<String, String>,
    pub shells: Vec<ShellKind>,
    pub enabled: bool,
    pub advanced_shell_mode: bool,
    pub tags: Vec<String>,
    pub path_mode: PathMode,
    pub path_origin: PathOrigin,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub record_checksum: String,
    pub revision: i64,
}

pub fn record_checksum(alias: &AliasRecord) -> Result<String, serde_json::Error> {
    let mut value = alias.clone(); value.record_checksum.clear(); value.updated_at = value.created_at;
    let json = serde_json::to_vec(&value)?; use sha2::{Digest, Sha256}; let mut digest = Sha256::new(); digest.update(json); Ok(format!("{:x}", digest.finalize()))
}

impl Default for AliasRecord {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(), name: String::new(), description: String::new(),
            target_type: TargetType::NativeExecutable, executable: String::new(),
            fixed_args: Vec::new(), pass_args: true, working_directory: None,
            environment: BTreeMap::new(), shells: vec![ShellKind::Bash], enabled: true,
            advanced_shell_mode: false, tags: Vec::new(), path_mode: PathMode::Absolute,
            path_origin: PathOrigin::CliArgument, created_at: now, updated_at: now,
            record_checksum: String::new(), revision: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conflict { pub name: String, pub reason: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShellStatus { Ok, Stale, Failed, LoaderMissing, Unknown }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellState {
    pub shell: ShellKind,
    pub applied_revision: i64,
    pub file_checksum: String,
    pub loader_installed: bool,
    pub status: ShellStatus,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedNameSet { pub current: Vec<String>, pub retired: Vec<String> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellSyncResult { pub shell: ShellKind, pub status: ShellStatus, pub error: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncReceipt { pub revision: i64, pub results: Vec<ShellSyncResult> }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_is_stable_for_same_content() {
        let alias = AliasRecord::default();
        assert_eq!(record_checksum(&alias).unwrap(), record_checksum(&alias).unwrap());
    }

    #[test]
    fn alias_round_trips_as_json() {
        let alias = AliasRecord::default();
        let json = serde_json::to_string(&alias).unwrap();
        let decoded: AliasRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(alias, decoded);
        assert!(json.contains("created_at"));
    }

    #[test]
    fn unknown_enum_value_is_rejected() {
        assert!(serde_json::from_str::<ShellKind>("\"unknown\"").is_err());
    }
}
