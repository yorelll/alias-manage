use crate::{
    error::AliasError,
    migrations,
    model::{record_checksum, AliasRecord},
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::{path::Path, str::FromStr};
use uuid::Uuid;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AliasError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let backup = if path.exists() {
            let directory = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("backups/db");
            std::fs::create_dir_all(&directory)?;
            let backup = directory.join(format!(
                "aliases-{}.db",
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
            ));
            std::fs::copy(path, &backup)?;
            Some(backup)
        } else {
            None
        };

        let conn = Connection::open(path)?;
        Self::configure(&conn)?;
        if let Err(error) = migrations::migrate(&conn) {
            drop(conn);
            if let Some(backup) = backup {
                std::fs::copy(backup, path)?;
            }
            return Err(error);
        }
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self, AliasError> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;
        migrations::migrate(&conn)?;
        Ok(Self { conn })
    }

    fn configure(conn: &Connection) -> Result<(), AliasError> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA synchronous = FULL;",
        )?;
        let journal_mode: String =
            conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            conn.execute_batch("PRAGMA journal_mode = DELETE;")?;
        }
        Ok(())
    }

    pub fn journal_mode(&self) -> Result<String, AliasError> {
        Ok(self
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
    }

    pub fn synchronous_mode(&self) -> Result<i64, AliasError> {
        Ok(self
            .conn
            .query_row("PRAGMA synchronous", [], |row| row.get(0))?)
    }

    pub fn busy_timeout_ms(&self) -> Result<i64, AliasError> {
        Ok(self
            .conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))?)
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool, AliasError> {
        Ok(self
            .conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))?
            != 0)
    }

    pub fn insert_alias(&self, alias: &AliasRecord) -> Result<(), AliasError> {
        if self.get_alias_by_name(&alias.name)?.is_some() {
            return Err(AliasError::ExactNameConflict(alias.name.clone()));
        }
        if has_powershell_shell(&alias.shells)
            && self.has_powershell_case_conflict(&alias.name, None)?
        {
            return Err(AliasError::CaseFoldConflict(alias.name.clone()));
        }
        let checksum = checked_checksum(alias)?;
        self.conn
            .execute(
                "INSERT INTO aliases (id,name,name_folded,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
                params![
                    alias.id.to_string(),
                    alias.name,
                    alias.name.to_lowercase(),
                    alias.description,
                    serde_json::to_string(&alias.target_type)?,
                    alias.executable,
                    serde_json::to_string(&alias.fixed_args)?,
                    alias.pass_args,
                    alias.working_directory,
                    serde_json::to_string(&alias.environment)?,
                    serde_json::to_string(&alias.shells)?,
                    alias.enabled,
                    alias.advanced_shell_mode,
                    serde_json::to_string(&alias.tags)?,
                    serde_json::to_string(&alias.path_mode)?,
                    serde_json::to_string(&alias.path_origin)?,
                    alias.created_at.to_rfc3339(),
                    alias.updated_at.to_rfc3339(),
                    checksum,
                    alias.revision,
                ],
            )
            .map_err(map_constraint)?;
        Ok(())
    }

    pub fn get_alias(&self, id: Uuid) -> Result<Option<AliasRecord>, AliasError> {
        self.fetch("id", &id.to_string())
    }

    pub fn get_alias_by_name(&self, name: &str) -> Result<Option<AliasRecord>, AliasError> {
        self.fetch("name", name)
    }

    pub fn has_powershell_case_conflict(
        &self,
        name: &str,
        excluding: Option<Uuid>,
    ) -> Result<bool, AliasError> {
        let folded = name.to_lowercase();
        let mut statement = self
            .conn
            .prepare("SELECT id, shells_json FROM aliases WHERE name_folded = ?1")?;
        let rows = statement.query_map(params![folded], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, shells): (String, String) = row?;
            if excluding == Uuid::parse_str(&id).ok() {
                continue;
            }
            let shells: Vec<crate::model::ShellKind> = serde_json::from_str(&shells)?;
            if has_powershell_shell(&shells) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn schema_version(&self) -> Result<i64, AliasError> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    pub fn upsert_shell_state(
        &self,
        state: &crate::model::ShellState,
    ) -> Result<(), AliasError> {
        self.conn.execute(
            "INSERT INTO shell_state(shell, applied_revision, file_checksum, loader_installed, status, last_error) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(shell) DO UPDATE SET applied_revision=excluded.applied_revision,file_checksum=excluded.file_checksum,loader_installed=excluded.loader_installed,status=excluded.status,last_error=excluded.last_error",
            params![
                serde_json::to_string(&state.shell)?,
                state.applied_revision,
                state.file_checksum,
                state.loader_installed,
                serde_json::to_string(&state.status)?,
                state.last_error,
            ],
        )?;
        Ok(())
    }

    pub fn retire_name(
        &self,
        name: &str,
        shell: &crate::model::ShellKind,
        revision: i64,
    ) -> Result<(), AliasError> {
        self.conn.execute(
            "INSERT INTO retired_names(name, shell, definition_kind, retired_at_revision, retired_at) VALUES (?1,?2,'function',?3,datetime('now'))",
            params![name, serde_json::to_string(shell)?, revision],
        )?;
        Ok(())
    }

    pub fn managed_name_set(
        &self,
        shell: &crate::model::ShellKind,
    ) -> Result<crate::model::ManagedNameSet, AliasError> {
        let shell_json = serde_json::to_string(shell)?;
        let current = self
            .conn
            .prepare(
                "SELECT name FROM aliases WHERE enabled = 1 AND shells_json LIKE '%' || ?1 || '%'",
            )?
            .query_map(params![shell_json.trim_matches('"')], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        let retired = self
            .conn
            .prepare("SELECT name FROM retired_names WHERE shell = ?1 ORDER BY retired_at_revision")?
            .query_map(params![shell_json], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(crate::model::ManagedNameSet { current, retired })
    }

    pub fn prune_retired_names(&self, minimum_revision: i64) -> Result<usize, AliasError> {
        Ok(self.conn.execute(
            "DELETE FROM retired_names WHERE retired_at_revision < ?1",
            params![minimum_revision],
        )?)
    }

    pub fn record_override(
        &self,
        name: &str,
        shell: &crate::model::ShellKind,
        original_definition: Option<&str>,
        recoverable: bool,
    ) -> Result<(), AliasError> {
        self.conn.execute_batch("CREATE TABLE IF NOT EXISTS overridden_definitions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, shell TEXT NOT NULL, definition_kind TEXT NOT NULL, original_definition TEXT, recoverable INTEGER NOT NULL DEFAULT 0, captured_at TEXT NOT NULL)")?;
        self.conn.execute(
            "INSERT INTO overridden_definitions(name,shell,definition_kind,original_definition,recoverable,captured_at) VALUES (?1,?2,'function',?3,?4,datetime('now'))",
            params![name, serde_json::to_string(shell)?, original_definition, recoverable],
        )?;
        Ok(())
    }

    fn fetch(&self, field: &str, value: &str) -> Result<Option<AliasRecord>, AliasError> {
        let sql = format!("SELECT id,name,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision FROM aliases WHERE {field} = ?1");
        self.conn
            .query_row(&sql, params![value], row_to_alias)
            .optional()?
            .map(verify_checksum)
            .transpose()
    }

    pub fn list_aliases(&self) -> Result<Vec<AliasRecord>, AliasError> {
        let mut statement = self.conn.prepare("SELECT id,name,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision FROM aliases ORDER BY name COLLATE BINARY")?;
        let rows = statement.query_map([], row_to_alias)?;
        rows.map(|row| row.map_err(AliasError::from).and_then(verify_checksum))
            .collect()
    }

    pub fn update_alias(&self, alias: &AliasRecord) -> Result<bool, AliasError> {
        if self
            .conn
            .query_row(
                "SELECT name FROM aliases WHERE id = ?1",
                params![alias.id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .is_none()
        {
            return Ok(false);
        }
        if self
            .conn
            .query_row(
                "SELECT id FROM aliases WHERE name = ?1 AND id != ?2",
                params![alias.name, alias.id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .is_some()
        {
            return Err(AliasError::ExactNameConflict(alias.name.clone()));
        }
        if has_powershell_shell(&alias.shells)
            && self.has_powershell_case_conflict(&alias.name, Some(alias.id))?
        {
            return Err(AliasError::CaseFoldConflict(alias.name.clone()));
        }
        let checksum = checked_checksum(alias)?;
        let changed = self.conn.execute(
            "UPDATE aliases SET name=?2,name_folded=?3,description=?4,target_type=?5,executable=?6,fixed_args_json=?7,pass_args=?8,working_directory=?9,environment_json=?10,shells_json=?11,enabled=?12,advanced_shell_mode=?13,tags_json=?14,path_mode=?15,path_origin=?16,updated_at=?17,record_checksum=?18,revision=?19 WHERE id=?1",
            params![
                alias.id.to_string(),
                alias.name,
                alias.name.to_lowercase(),
                alias.description,
                serde_json::to_string(&alias.target_type)?,
                alias.executable,
                serde_json::to_string(&alias.fixed_args)?,
                alias.pass_args,
                alias.working_directory,
                serde_json::to_string(&alias.environment)?,
                serde_json::to_string(&alias.shells)?,
                alias.enabled,
                alias.advanced_shell_mode,
                serde_json::to_string(&alias.tags)?,
                serde_json::to_string(&alias.path_mode)?,
                serde_json::to_string(&alias.path_origin)?,
                alias.updated_at.to_rfc3339(),
                checksum,
                alias.revision,
            ],
        )
        .map_err(map_constraint)?;
        Ok(changed == 1)
    }

    pub fn delete_alias(&self, id: Uuid) -> Result<bool, AliasError> {
        Ok(self
            .conn
            .execute("DELETE FROM aliases WHERE id=?1", params![id.to_string()])?
            == 1)
    }

    pub fn transaction<T, F>(&mut self, operation: F) -> Result<T, AliasError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T, AliasError>,
    {
        let transaction = self.conn.transaction()?;
        let result = operation(&transaction)?;
        transaction.commit()?;
        Ok(result)
    }
}

fn has_powershell_shell(shells: &[crate::model::ShellKind]) -> bool {
    shells.iter().any(|shell| {
        matches!(
            shell,
            crate::model::ShellKind::PowerShell5 | crate::model::ShellKind::PowerShell7
        )
    })
}

fn checked_checksum(alias: &AliasRecord) -> Result<String, AliasError> {
    let checksum = record_checksum(alias)?;
    if alias.record_checksum.is_empty() || alias.record_checksum == checksum {
        Ok(checksum)
    } else {
        Err(AliasError::ChecksumMismatch)
    }
}

fn verify_checksum(alias: AliasRecord) -> Result<AliasRecord, AliasError> {
    if alias.record_checksum == record_checksum(&alias)? {
        Ok(alias)
    } else {
        Err(AliasError::ChecksumMismatch)
    }
}

fn map_constraint(error: rusqlite::Error) -> AliasError {
    if matches!(error, rusqlite::Error::SqliteFailure(_, _)) {
        AliasError::AliasConflict(error.to_string())
    } else {
        AliasError::Database(error)
    }
}

fn row_to_alias(row: &rusqlite::Row<'_>) -> rusqlite::Result<AliasRecord> {
    let parse = |column: usize| -> rusqlite::Result<String> { row.get(column) };
    let parse_json = |column: usize| -> rusqlite::Result<String> { row.get(column) };
    let parse_time = |column: usize| -> rusqlite::Result<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&parse(column)?)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    column,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
    };
    Ok(AliasRecord {
        id: Uuid::from_str(&parse(0)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        name: parse(1)?,
        description: parse(2)?,
        target_type: serde_json::from_str(&parse(3)?).map_err(json_error(3))?,
        executable: parse(4)?,
        fixed_args: serde_json::from_str(&parse_json(5)?).map_err(json_error(5))?,
        pass_args: row.get::<_, i64>(6)? != 0,
        working_directory: row.get(7)?,
        environment: serde_json::from_str(&parse_json(8)?).map_err(json_error(8))?,
        shells: serde_json::from_str(&parse_json(9)?).map_err(json_error(9))?,
        enabled: row.get::<_, i64>(10)? != 0,
        advanced_shell_mode: row.get::<_, i64>(11)? != 0,
        tags: serde_json::from_str(&parse_json(12)?).map_err(json_error(12))?,
        path_mode: serde_json::from_str(&parse(13)?).map_err(json_error(13))?,
        path_origin: serde_json::from_str(&parse(14)?).map_err(json_error(14))?,
        created_at: parse_time(15)?,
        updated_at: parse_time(16)?,
        record_checksum: parse(17)?,
        revision: row.get(18)?,
    })
}

fn json_error(column: usize) -> impl Fn(serde_json::Error) -> rusqlite::Error {
    move |error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(mut alias: AliasRecord) -> AliasRecord {
        alias.record_checksum = record_checksum(&alias).unwrap();
        alias
    }

    fn database_alias(name: &str) -> AliasRecord {
        checked(AliasRecord {
            name: name.into(),
            executable: "tool".into(),
            ..Default::default()
        })
    }

    #[test]
    fn crud_and_ordering_work_in_memory() {
        let database = Database::open_in_memory().unwrap();
        let mut first = database_alias("zeta");
        let second = database_alias("alpha");
        database.insert_alias(&first).unwrap();
        database.insert_alias(&second).unwrap();
        assert_eq!(database.get_alias(first.id).unwrap().unwrap(), first);
        assert_eq!(database.get_alias_by_name("alpha").unwrap().unwrap(), second);
        assert_eq!(
            database
                .list_aliases()
                .unwrap()
                .iter()
                .map(|alias| alias.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
        first.description = "updated".into();
        first.record_checksum = record_checksum(&first).unwrap();
        assert!(database.update_alias(&first).unwrap());
        assert_eq!(
            database.get_alias(first.id).unwrap().unwrap().description,
            "updated"
        );
        assert!(database.delete_alias(second.id).unwrap());
        assert!(database.get_alias(second.id).unwrap().is_none());
    }

    #[test]
    fn duplicate_names_have_a_distinct_exact_name_error() {
        let database = Database::open_in_memory().unwrap();
        let first = database_alias("same");
        let second = first.clone();
        database.insert_alias(&first).unwrap();
        assert!(matches!(
            database.insert_alias(&second),
            Err(AliasError::ExactNameConflict(name)) if name == "same"
        ));
    }

    #[test]
    fn powershell_case_conflicts_are_rejected_on_insert_and_update() {
        let database = Database::open_in_memory().unwrap();
        let first = AliasRecord {
            name: "Build".into(),
            executable: "tool".into(),
            shells: vec![crate::model::ShellKind::PowerShell7],
            ..Default::default()
        };
        database.insert_alias(&first).unwrap();
        let second = AliasRecord {
            name: "build".into(),
            executable: "tool".into(),
            shells: vec![crate::model::ShellKind::PowerShell5],
            ..Default::default()
        };
        assert!(matches!(
            database.insert_alias(&second),
            Err(AliasError::CaseFoldConflict(name)) if name == "build"
        ));

        let mut bash_alias = AliasRecord {
            name: "other".into(),
            executable: "tool".into(),
            ..Default::default()
        };
        database.insert_alias(&bash_alias).unwrap();
        bash_alias.name = "Build".into();
        bash_alias.shells = vec![crate::model::ShellKind::PowerShell5];
        bash_alias.record_checksum = record_checksum(&bash_alias).unwrap();
        assert!(matches!(
            database.update_alias(&bash_alias),
            Err(AliasError::CaseFoldConflict(name)) if name == "Build"
        ));
    }

    #[test]
    fn pure_bash_case_variants_can_coexist() {
        let database = Database::open_in_memory().unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "CM".into(),
                executable: "tool".into(),
                ..Default::default()
            })
            .unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "cm".into(),
                executable: "tool".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 2);
    }

    #[test]
    fn checksum_is_stored_and_tampering_is_detected() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-storage-checksum-{}",
            std::process::id()
        ));
        let database = Database::open(root.join("aliases.db")).unwrap();
        let alias = database_alias("checked");
        database.insert_alias(&alias).unwrap();
        let stored = database.get_alias_by_name("checked").unwrap().unwrap();
        assert_eq!(stored.record_checksum, record_checksum(&stored).unwrap());
        database
            .conn
            .execute(
                "UPDATE aliases SET description = 'tampered' WHERE id = ?1",
                params![alias.id.to_string()],
            )
            .unwrap();
        assert!(matches!(
            database.get_alias(alias.id),
            Err(AliasError::ChecksumMismatch)
        ));
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn supplied_wrong_checksums_are_rejected_on_insert_and_update() {
        let database = Database::open_in_memory().unwrap();
        let mut new_alias = AliasRecord {
            name: "bad-checksum".into(),
            executable: "tool".into(),
            ..Default::default()
        };
        new_alias.record_checksum = "not-the-record-checksum".into();
        assert!(matches!(
            database.insert_alias(&new_alias),
            Err(AliasError::ChecksumMismatch)
        ));

        let mut existing = database_alias("update-checksum");
        database.insert_alias(&existing).unwrap();
        existing.description = "changed".into();
        assert!(matches!(
            database.update_alias(&existing),
            Err(AliasError::ChecksumMismatch)
        ));
    }

    #[test]
    fn legacy_empty_checksum_is_rejected_when_read() {
        let database = Database::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        let now = Utc::now();
        let legacy = AliasRecord {
            id,
            name: "legacy".into(),
            executable: "tool".into(),
            created_at: now,
            updated_at: now,
            ..Default::default()
        };
        database
            .conn
            .execute(
                "INSERT INTO aliases (id,name,name_folded,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
                params![
                    legacy.id.to_string(),
                    legacy.name,
                    "legacy",
                    serde_json::to_string(&legacy.target_type).unwrap(),
                    legacy.executable,
                    serde_json::to_string(&legacy.fixed_args).unwrap(),
                    legacy.pass_args,
                    legacy.working_directory,
                    serde_json::to_string(&legacy.environment).unwrap(),
                    serde_json::to_string(&legacy.shells).unwrap(),
                    legacy.enabled,
                    legacy.advanced_shell_mode,
                    serde_json::to_string(&legacy.tags).unwrap(),
                    serde_json::to_string(&legacy.path_mode).unwrap(),
                    serde_json::to_string(&legacy.path_origin).unwrap(),
                    legacy.created_at.to_rfc3339(),
                    legacy.updated_at.to_rfc3339(),
                    "",
                    legacy.revision,
                ],
            )
            .unwrap();
        assert!(matches!(
            database.get_alias_by_name("legacy"),
            Err(AliasError::ChecksumMismatch)
        ));
    }

    #[test]
    fn connection_pragmas_are_configured_for_durable_storage() {
        let database = Database::open_in_memory().unwrap();
        assert!(matches!(
            database.journal_mode().unwrap().to_ascii_lowercase().as_str(),
            "wal" | "memory" | "delete"
        ));
        assert_eq!(database.synchronous_mode().unwrap(), 2);
        assert_eq!(database.busy_timeout_ms().unwrap(), 5000);
        assert!(database.foreign_keys_enabled().unwrap());
    }

    #[test]
    fn file_database_uses_wal_when_supported() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-storage-pragmas-{}",
            std::process::id()
        ));
        let database = Database::open(root.join("aliases.db")).unwrap();
        assert!(matches!(
            database.journal_mode().unwrap().to_ascii_lowercase().as_str(),
            "wal" | "delete"
        ));
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn migration_backup_is_created_before_reopening_existing_database() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-storage-backup-{}",
            std::process::id()
        ));
        let path = root.join("aliases.db");
        drop(Database::open(&path).unwrap());
        drop(Database::open(&path).unwrap());
        let backups = std::fs::read_dir(root.join("backups/db"))
            .unwrap()
            .filter_map(Result::ok)
            .count();
        assert!(backups >= 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_migration_restores_the_existing_database_file() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-storage-restore-{}",
            std::process::id()
        ));
        let path = root.join("aliases.db");
        let database = Database::open(&path).unwrap();
        database
            .conn
            .execute("PRAGMA user_version = 999", [])
            .unwrap();
        drop(database);
        assert!(matches!(
            Database::open(&path),
            Err(AliasError::SchemaTooNew)
        ));
        let restored = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(
            restored
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            999
        );
        drop(restored);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn open_creates_parent_directory_for_a_new_database() {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-storage-parent-{}",
            std::process::id()
        ));
        let path = root.join("nested").join("aliases.db");
        drop(Database::open(&path).unwrap());
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn auxiliary_shell_state_and_retired_names_are_stored() {
        let database = Database::open_in_memory().unwrap();
        let state = crate::model::ShellState {
            shell: crate::model::ShellKind::Bash,
            applied_revision: 1,
            file_checksum: "abc".into(),
            loader_installed: true,
            status: crate::model::ShellStatus::Ok,
            last_error: None,
        };
        database.upsert_shell_state(&state).unwrap();
        database
            .retire_name("old", &crate::model::ShellKind::Bash, 1)
            .unwrap();
        assert_eq!(
            database
                .managed_name_set(&crate::model::ShellKind::Bash)
                .unwrap()
                .retired,
            vec!["old"]
        );
        database
            .record_override("old", &crate::model::ShellKind::Bash, Some("fn"), true)
            .unwrap();
        assert_eq!(database.prune_retired_names(2).unwrap(), 1);
    }

    #[test]
    fn failed_transaction_rolls_back() {
        let mut database = Database::open_in_memory().unwrap();
        let result: Result<(), AliasError> = database.transaction(|transaction| {
            transaction
                .execute(
                    "INSERT INTO aliases (id,name,name_folded,target_type,executable,fixed_args_json,shells_json,created_at,updated_at) VALUES ('x','rollback','rollback','native_executable','tool','[]','[\"bash\"]',datetime('now'),datetime('now'))",
                    [],
                )
                .map_err(AliasError::from)?;
            Err(AliasError::InvalidAliasName)
        });
        assert!(result.is_err());
        assert!(database.get_alias_by_name("rollback").unwrap().is_none());
    }
}
