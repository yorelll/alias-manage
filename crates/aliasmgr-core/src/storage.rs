use crate::{error::AliasError, migrations, model::AliasRecord};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::{path::Path, str::FromStr};
use uuid::Uuid;

pub struct Database { pub conn: Connection }

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AliasError> {
        let path = path.as_ref();
        let backup = if path.exists() {
            let directory = path.parent().unwrap_or_else(|| Path::new(".")).join("backups/db");
            std::fs::create_dir_all(&directory)?;
            let backup = directory.join(format!("aliases-{}.db", chrono::Utc::now().format("%Y%m%d%H%M%S")));
            std::fs::copy(path, &backup)?;
            Some(backup)
        } else { None };
        let conn = Connection::open(path)?;
        Self::configure(&conn)?;
        if let Err(error) = migrations::migrate(&conn) {
            drop(conn);
            if let Some(backup) = backup { std::fs::copy(backup, path)?; }
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
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA synchronous = FULL; PRAGMA journal_mode = WAL;")?;
        Ok(())
    }

    pub fn insert_alias(&self, alias: &AliasRecord) -> Result<(), AliasError> {
        self.conn.execute(
            "INSERT INTO aliases (id,name,name_folded,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
            params![alias.id.to_string(), alias.name, alias.name.to_lowercase(), alias.description, serde_json::to_string(&alias.target_type)?, alias.executable, serde_json::to_string(&alias.fixed_args)?, alias.pass_args, alias.working_directory, serde_json::to_string(&alias.environment)?, serde_json::to_string(&alias.shells)?, alias.enabled, alias.advanced_shell_mode, serde_json::to_string(&alias.tags)?, serde_json::to_string(&alias.path_mode)?, serde_json::to_string(&alias.path_origin)?, alias.created_at.to_rfc3339(), alias.updated_at.to_rfc3339(), alias.record_checksum, alias.revision,
        ],
        ).map_err(map_constraint)?;
        Ok(())
    }

    pub fn get_alias(&self, id: Uuid) -> Result<Option<AliasRecord>, AliasError> { self.fetch("id", &id.to_string()) }
    pub fn get_alias_by_name(&self, name: &str) -> Result<Option<AliasRecord>, AliasError> { self.fetch("name", name) }

    pub fn has_powershell_case_conflict(&self, name: &str, excluding: Option<Uuid>) -> Result<bool, AliasError> {
        let folded = name.to_lowercase();
        let mut statement = self.conn.prepare("SELECT id, shells_json FROM aliases WHERE name_folded = ?1")?;
        let rows = statement.query_map(params![folded], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, shells): (String, String) = row?;
            if excluding == Uuid::parse_str(&id).ok() { continue; }
            let shells: Vec<crate::model::ShellKind> = serde_json::from_str(&shells)?;
            if shells.iter().any(|shell| matches!(shell, crate::model::ShellKind::PowerShell5 | crate::model::ShellKind::PowerShell7)) { return Ok(true); }
        }
        Ok(false)
    }

    pub fn schema_version(&self) -> Result<i64, AliasError> { Ok(self.conn.query_row("PRAGMA user_version", [], |row| row.get(0))?) }

    pub fn upsert_shell_state(&self, state: &crate::model::ShellState) -> Result<(), AliasError> {
        self.conn.execute("INSERT INTO shell_state(shell, applied_revision, file_checksum, loader_installed, status, last_error) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(shell) DO UPDATE SET applied_revision=excluded.applied_revision,file_checksum=excluded.file_checksum,loader_installed=excluded.loader_installed,status=excluded.status,last_error=excluded.last_error", params![serde_json::to_string(&state.shell)?, state.applied_revision, state.file_checksum, state.loader_installed, serde_json::to_string(&state.status)?, state.last_error])?;
        Ok(())
    }

    pub fn retire_name(&self, name: &str, shell: &crate::model::ShellKind, revision: i64) -> Result<(), AliasError> {
        self.conn.execute("INSERT INTO retired_names(name, shell, definition_kind, retired_at_revision, retired_at) VALUES (?1,?2,'function',?3,datetime('now'))", params![name, serde_json::to_string(shell)?, revision])?;
        Ok(())
    }

    fn fetch(&self, field: &str, value: &str) -> Result<Option<AliasRecord>, AliasError> {
        let sql = format!("SELECT id,name,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision FROM aliases WHERE {field} = ?1");
        self.conn.query_row(&sql, params![value], row_to_alias).optional().map_err(AliasError::from)
    }

    pub fn list_aliases(&self) -> Result<Vec<AliasRecord>, AliasError> {
        let mut statement = self.conn.prepare("SELECT id,name,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision FROM aliases ORDER BY name COLLATE BINARY")?;
        let rows = statement.query_map([], row_to_alias)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn update_alias(&self, alias: &AliasRecord) -> Result<bool, AliasError> {
        let changed = self.conn.execute(
            "UPDATE aliases SET name=?2,name_folded=?3,description=?4,target_type=?5,executable=?6,fixed_args_json=?7,pass_args=?8,working_directory=?9,environment_json=?10,shells_json=?11,enabled=?12,advanced_shell_mode=?13,tags_json=?14,path_mode=?15,path_origin=?16,updated_at=?17,record_checksum=?18,revision=?19 WHERE id=?1",
            params![alias.id.to_string(), alias.name, alias.name.to_lowercase(), alias.description, serde_json::to_string(&alias.target_type)?, alias.executable, serde_json::to_string(&alias.fixed_args)?, alias.pass_args, alias.working_directory, serde_json::to_string(&alias.environment)?, serde_json::to_string(&alias.shells)?, alias.enabled, alias.advanced_shell_mode, serde_json::to_string(&alias.tags)?, serde_json::to_string(&alias.path_mode)?, serde_json::to_string(&alias.path_origin)?, alias.updated_at.to_rfc3339(), alias.record_checksum, alias.revision],
        ).map_err(map_constraint)?;
        Ok(changed == 1)
    }

    pub fn delete_alias(&self, id: Uuid) -> Result<bool, AliasError> { Ok(self.conn.execute("DELETE FROM aliases WHERE id=?1", params![id.to_string()])? == 1) }

    pub fn transaction<T, F>(&mut self, operation: F) -> Result<T, AliasError>
    where F: FnOnce(&Transaction<'_>) -> Result<T, AliasError> {
        let transaction = self.conn.transaction()?;
        let result = operation(&transaction)?;
        transaction.commit()?;
        Ok(result)
    }
}

fn map_constraint(error: rusqlite::Error) -> AliasError {
    if matches!(error, rusqlite::Error::SqliteFailure(_, _)) { AliasError::AliasConflict(error.to_string()) } else { AliasError::Database(error) }
}

fn row_to_alias(row: &rusqlite::Row<'_>) -> rusqlite::Result<AliasRecord> {
    let parse = |column: usize| -> rusqlite::Result<String> { row.get(column) };
    let parse_json = |column: usize| -> rusqlite::Result<String> { row.get(column) };
    let parse_time = |column: usize| -> rusqlite::Result<DateTime<Utc>> { DateTime::parse_from_rfc3339(&parse(column)?).map(|value| value.with_timezone(&Utc)).map_err(|error| rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(error))) };
    Ok(AliasRecord {
        id: Uuid::from_str(&parse(0)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
        name: parse(1)?, description: parse(2)?, target_type: serde_json::from_str(&parse(3)?).map_err(json_error(3))?, executable: parse(4)?,
        fixed_args: serde_json::from_str(&parse_json(5)?).map_err(json_error(5))?, pass_args: row.get::<_, i64>(6)? != 0, working_directory: row.get(7)?,
        environment: serde_json::from_str(&parse_json(8)?).map_err(json_error(8))?, shells: serde_json::from_str(&parse_json(9)?).map_err(json_error(9))?, enabled: row.get::<_, i64>(10)? != 0, advanced_shell_mode: row.get::<_, i64>(11)? != 0,
        tags: serde_json::from_str(&parse_json(12)?).map_err(json_error(12))?, path_mode: serde_json::from_str(&parse(13)?).map_err(json_error(13))?, path_origin: serde_json::from_str(&parse(14)?).map_err(json_error(14))?, created_at: parse_time(15)?, updated_at: parse_time(16)?, record_checksum: parse(17)?, revision: row.get(18)?,
    })
}

fn json_error(column: usize) -> impl Fn(serde_json::Error) -> rusqlite::Error {
    move |error| rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crud_and_ordering_work_in_memory() {
        let database = Database::open_in_memory().unwrap();
        let mut first = AliasRecord { name: "zeta".into(), executable: "tool".into(), ..Default::default() };
        let second = AliasRecord { name: "alpha".into(), executable: "tool".into(), ..Default::default() };
        database.insert_alias(&first).unwrap(); database.insert_alias(&second).unwrap();
        assert_eq!(database.get_alias(first.id).unwrap().unwrap(), first);
        assert_eq!(database.get_alias_by_name("alpha").unwrap().unwrap(), second);
        assert_eq!(database.list_aliases().unwrap().iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), vec!["alpha", "zeta"]);
        first.description = "updated".into(); assert!(database.update_alias(&first).unwrap());
        assert_eq!(database.get_alias(first.id).unwrap().unwrap().description, "updated");
        assert!(database.delete_alias(second.id).unwrap());
        assert!(database.get_alias(second.id).unwrap().is_none());
    }

    #[test]
    fn powershell_case_conflicts_are_detected() {
        let database = Database::open_in_memory().unwrap();
        let mut first = AliasRecord { name: "Build".into(), executable: "tool".into(), shells: vec![crate::model::ShellKind::PowerShell7], ..Default::default() };
        database.insert_alias(&first).unwrap();
        assert!(database.has_powershell_case_conflict("build", None).unwrap());
        assert!(database.has_powershell_case_conflict("build", Some(first.id)).unwrap() == false);
        first.name = "other".into();
    }

    #[test]
    fn duplicate_names_are_conflicts() {
        let database = Database::open_in_memory().unwrap();
        let first = AliasRecord { name: "same".into(), executable: "tool".into(), ..Default::default() };
        let second = first.clone(); database.insert_alias(&first).unwrap();
        assert!(matches!(database.insert_alias(&second), Err(AliasError::AliasConflict(_))));
    }

    #[test]
    fn failed_transaction_rolls_back() {
        let mut database = Database::open_in_memory().unwrap();
        let alias = AliasRecord::default();
        let result: Result<(), AliasError> = database.transaction(|transaction| { transaction.execute("INSERT INTO aliases (id,name,name_folded,target_type,executable,fixed_args_json,shells_json,created_at,updated_at) VALUES ('x','rollback','rollback','native_executable','tool','[]','[\"bash\"]',datetime('now'),datetime('now'))", []).map_err(AliasError::from)?; Err(AliasError::InvalidAliasName) });
        assert!(result.is_err()); assert!(database.get_alias_by_name(&alias.name).unwrap().is_none()); assert!(database.get_alias_by_name("rollback").unwrap().is_none());
    }
}
