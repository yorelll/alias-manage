use crate::{
    error::AliasError,
    migrations,
    model::{record_checksum, AliasRecord, PathRestriction, ShellKind},
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    str::FromStr,
};
use uuid::Uuid;

/// One row produced by an override scan. Only metadata is stored; the raw
/// profile line is deliberately not part of this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideEntry {
    pub name: String,
    pub shell: crate::model::ShellKind,
    pub definition_kind: String,
    pub recoverable: bool,
}

pub struct Database {
    pub conn: Connection,
    /// Directory the database file lives in. `None` for in-memory and
    /// read-only handles, which cannot make reliability claims about storage.
    root: Option<PathBuf>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AliasError> {
        let path = path.as_ref();
        let root = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf);
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
        // Rotate only once the new connection has migrated successfully: a
        // corrupt database must not prune the good history it fails to open.
        if let (Some(backup), Some(parent)) = (backup.as_deref(), path.parent()) {
            rotate_database_backups(&parent.join("backups/db"), backup, root.as_deref());
        }
        Ok(Self { conn, root })
    }

    pub fn open_in_memory() -> Result<Self, AliasError> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;
        migrations::migrate(&conn)?;
        Ok(Self { conn, root: None })
    }

    /// Opens an existing database without backups, migrations, journal-mode
    /// changes or any other write. Only `busy_timeout` is configured. Used by
    /// read-only paths such as import preview so they cannot mutate storage.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, AliasError> {
        let conn =
            Connection::open_with_flags(path.as_ref(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
        Ok(Self { conn, root: None })
    }

    /// Filesystem root the database was opened from, if any. Exposed to tests
    /// so the reliability wiring can be asserted against the same mapping.
    #[cfg(test)]
    fn root(&self) -> Option<&Path> {
        self.root.as_deref()
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

    /// Reports storage that cannot guarantee crash-safe behaviour. Databases
    /// without a filesystem root (in-memory, read-only) never report it.
    pub fn reliability_error(&self) -> Option<AliasError> {
        let root = self.root.as_deref()?;
        reliability_error_for(crate::config::filesystem_reliability(root))
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
        if crate::model::contains_powershell(&alias.shells)
            && self.has_powershell_case_conflict(&alias.name, None)?
        {
            return Err(AliasError::CaseFoldConflict(alias.name.clone()));
        }
        let checksum = checked_checksum(alias)?;
        self.conn
            .execute(
                "INSERT INTO aliases (id,name,name_folded,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision,restriction_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
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
                    serde_json::to_string(&alias.restriction)?,
                ],
            )
            .map_err(map_constraint)?;
        Ok(())
    }

    pub fn get_alias(&self, id: Uuid) -> Result<Option<AliasRecord>, AliasError> {
        self.fetch("id", &id.to_string())
    }

    /// Inserts all records in one transaction; any failure rolls back every
    /// record inserted by this call.
    pub fn insert_aliases_atomic(&self, aliases: &[AliasRecord]) -> Result<(), AliasError> {
        let transaction = self.conn.unchecked_transaction()?;
        for alias in aliases {
            self.insert_alias(alias)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Persists imported records in one transaction. `overwrite_existing` maps
    /// names that existed when the import was evaluated to their ids. Only
    /// those exact rows may be updated; a row inserted or replaced by another
    /// front end after the preview is a conflict, never a silent overwrite.
    pub fn import_aliases_atomic(
        &self,
        aliases: &[AliasRecord],
        overwrite_existing: &BTreeMap<String, Uuid>,
    ) -> Result<(), AliasError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        for alias in aliases {
            if let Some(expected_id) = overwrite_existing.get(&alias.name).copied() {
                match self.get_alias_by_name(&alias.name)? {
                    Some(existing) if existing.id == expected_id => {
                        let mut record = alias.clone();
                        record.id = existing.id;
                        record.record_checksum = record_checksum(&record)?;
                        // Removing a shell from the overwritten record must retire
                        // the old name for that shell so the next sync emits cleanup.
                        for shell in existing
                            .shells
                            .iter()
                            .filter(|shell| !record.shells.contains(shell))
                        {
                            self.retire_name(
                                &existing.name,
                                shell,
                                existing.revision.saturating_add(1),
                            )?;
                        }
                        if !self.update_alias(&record)? {
                            return Err(AliasError::Config(format!(
                                "import overwrite could not update {}",
                                alias.name
                            )));
                        }
                        continue;
                    }
                    Some(_) => {
                        return Err(AliasError::AliasConflict(format!(
                            "import overwrite target changed after preview: {}",
                            alias.name
                        )))
                    }
                    None => {
                        // The row was deleted after preview; insert the imported
                        // record instead of silently resurrecting the old id.
                    }
                }
            }
            self.insert_alias(alias)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_alias_by_name(&self, name: &str) -> Result<Option<AliasRecord>, AliasError> {
        self.fetch("name", name)
    }

    /// SQL-backed mirror of `transfer::powershell_casefold_conflict` and the
    /// model's PowerShell rule. It queries the `name_folded` index, decodes
    /// each candidate's shells, and delegates the adapter check to
    /// `model::contains_powershell`.
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
            if crate::model::contains_powershell(&shells) {
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

    pub fn shell_states(&self) -> Result<Vec<crate::model::ShellState>, AliasError> {
        let mut statement = self.conn.prepare("SELECT shell, applied_revision, file_checksum, loader_installed, status, last_error FROM shell_state ORDER BY shell")?;
        let rows = statement.query_map([], |row| {
            Ok(crate::model::ShellState {
                shell: serde_json::from_str(&row.get::<_, String>(0)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
                applied_revision: row.get(1)?,
                file_checksum: row.get(2)?,
                loader_installed: row.get::<_, i64>(3)? != 0,
                status: serde_json::from_str(&row.get::<_, String>(4)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
                last_error: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AliasError::from)
    }

    pub fn upsert_shell_state(&self, state: &crate::model::ShellState) -> Result<(), AliasError> {
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
            .prepare(
                "SELECT name FROM retired_names WHERE shell = ?1 ORDER BY retired_at_revision",
            )?
            .query_map(params![shell_json], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(crate::model::ManagedNameSet { current, retired })
    }

    pub fn max_retired_revision(&self) -> Result<i64, AliasError> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(retired_at_revision), 0) FROM retired_names",
            [],
            |row| row.get::<_, i64>(0),
        )?)
    }

    pub fn prune_retired_names(&self, minimum_revision: i64) -> Result<usize, AliasError> {
        Ok(self.conn.execute(
            "DELETE FROM retired_names WHERE retired_at_revision < ?1",
            params![minimum_revision],
        )?)
    }

    fn ensure_overrides_table(&self) -> Result<(), AliasError> {
        self.conn.execute_batch("CREATE TABLE IF NOT EXISTS overridden_definitions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, shell TEXT NOT NULL, definition_kind TEXT NOT NULL, original_definition TEXT, recoverable INTEGER NOT NULL DEFAULT 0, captured_at TEXT NOT NULL)")?;
        Ok(())
    }

    pub fn overridden_definitions(&self) -> Result<Vec<serde_json::Value>, AliasError> {
        self.ensure_overrides_table()?;
        let mut statement = self.conn.prepare("SELECT name, shell, definition_kind, original_definition, recoverable, captured_at FROM overridden_definitions ORDER BY captured_at DESC")?;
        let rows = statement.query_map([], |row| {
            // `shell` is stored as a JSON string for round-tripping; expose the
            // plain shell name so the GUI never renders `"bash"` with quotes.
            let stored_shell: String = row.get(1)?;
            // Stored values are JSON strings whose serde spelling differs for
            // PowerShell (`power_shell7`); expose the canonical shell name.
            let shell = match serde_json::from_str::<crate::model::ShellKind>(&stored_shell) {
                Ok(kind) => kind.as_str().to_owned(),
                Err(_) => stored_shell.trim_matches('"').to_owned(),
            };
            // Read the raw column so the row shape stays aligned, but never
            // expose definitions over IPC (defense in depth).
            let _original_definition: Option<String> = row.get(3)?;
            Ok(serde_json::json!({
                "name": row.get::<_, String>(0)?,
                "shell": shell,
                "definition_kind": row.get::<_, String>(2)?,
                "recoverable": row.get::<_, i64>(4)? != 0,
                "captured_at": row.get::<_, String>(5)?,
            }))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AliasError::from)
    }

    /// Records (or replaces) the override for one `(name, shell)` pair.
    ///
    /// Repeated scans must not accumulate duplicate rows, so the existing row
    /// is replaced inside a transaction.
    pub fn record_override(
        &self,
        name: &str,
        shell: &crate::model::ShellKind,
        original_definition: Option<&str>,
        recoverable: bool,
    ) -> Result<(), AliasError> {
        self.record_override_kind(name, shell, "function", original_definition, recoverable)
    }

    /// Like [`Database::record_override`], but records the shape that produced
    /// the match (`alias`, `function` or `set_alias`).
    pub fn record_override_kind(
        &self,
        name: &str,
        shell: &crate::model::ShellKind,
        definition_kind: &str,
        original_definition: Option<&str>,
        recoverable: bool,
    ) -> Result<(), AliasError> {
        self.ensure_overrides_table()?;
        let shell_json = serde_json::to_string(shell)?;
        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM overridden_definitions WHERE name = ?1 AND shell = ?2",
            params![name, shell_json],
        )?;
        transaction.execute(
            "INSERT INTO overridden_definitions(name,shell,definition_kind,original_definition,recoverable,captured_at) VALUES (?1,?2,?3,?4,?5,datetime('now'))",
            params![name, shell_json, definition_kind, original_definition, recoverable],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Replaces every scan-produced override row atomically. Rows for aliases
    /// whose profile definition disappeared are removed because the table is
    /// cleared before the current findings are inserted.
    pub fn replace_overrides(&self, entries: &[OverrideEntry]) -> Result<(), AliasError> {
        self.ensure_overrides_table()?;
        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute("DELETE FROM overridden_definitions", [])?;
        for entry in entries {
            let shell_json = serde_json::to_string(&entry.shell)?;
            transaction.execute(
                "INSERT INTO overridden_definitions(name,shell,definition_kind,original_definition,recoverable,captured_at) VALUES (?1,?2,?3,NULL,?4,datetime('now'))",
                params![entry.name, shell_json, entry.definition_kind, entry.recoverable],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Column list shared by `fetch` and `list_aliases`. A read-only handle may
    /// open a database that predates the restriction column (import preview
    /// never migrates), so fall back to a literal JSON `null` in that case.
    fn alias_columns(&self) -> Result<String, AliasError> {
        let has_restriction = self
            .conn
            .prepare("PRAGMA table_info(aliases)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .iter()
            .any(|name| name == "restriction_json");
        let restriction = if has_restriction {
            "restriction_json"
        } else {
            "'null'"
        };
        Ok(format!("id,name,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision,{restriction}"))
    }

    fn fetch(&self, field: &str, value: &str) -> Result<Option<AliasRecord>, AliasError> {
        let sql = format!(
            "SELECT {} FROM aliases WHERE {field} = ?1",
            self.alias_columns()?
        );
        self.conn
            .query_row(&sql, params![value], row_to_alias)
            .optional()?
            .map(verify_checksum)
            .transpose()
    }

    pub fn list_aliases(&self) -> Result<Vec<AliasRecord>, AliasError> {
        let sql = format!(
            "SELECT {} FROM aliases ORDER BY name COLLATE BINARY",
            self.alias_columns()?
        );
        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map([], row_to_alias)?;
        rows.map(|row| row.map_err(AliasError::from).and_then(verify_checksum))
            .collect()
    }

    pub fn update_alias(&self, alias: &AliasRecord) -> Result<bool, AliasError> {
        self.update_alias_inner(alias, None)
    }

    /// Optimistic-concurrency update: writes only while the stored revision is
    /// still `expected_revision`. The revision guard is part of the UPDATE, so
    /// a concurrent CLI/GUI write cannot be silently overwritten.
    pub fn update_alias_checked(
        &self,
        alias: &AliasRecord,
        expected_revision: i64,
    ) -> Result<bool, AliasError> {
        self.update_alias_inner(alias, Some(expected_revision))
    }

    fn update_alias_inner(
        &self,
        alias: &AliasRecord,
        expected_revision: Option<i64>,
    ) -> Result<bool, AliasError> {
        let stored = self
            .conn
            .query_row(
                "SELECT created_at, revision FROM aliases WHERE id = ?1",
                params![alias.id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let Some((stored_created_at, stored_revision)) = stored else {
            return Ok(false);
        };
        if let Some(expected) = expected_revision {
            if stored_revision != expected {
                return Err(AliasError::AliasConflict(format!(
                    "alias {} changed since revision {} (found {})",
                    alias.name, expected, stored_revision
                )));
            }
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
        if crate::model::contains_powershell(&alias.shells)
            && self.has_powershell_case_conflict(&alias.name, Some(alias.id))?
        {
            return Err(AliasError::CaseFoldConflict(alias.name.clone()));
        }
        checked_checksum(alias)?;
        // created_at is DB-owned and immutable: callers only supply a checksum over the
        // record they built, so restore the stored timestamp before recomputing it.
        // The revision is likewise DB-owned and advances on every successful write.
        let mut record = alias.clone();
        record.created_at = DateTime::parse_from_rfc3339(&stored_created_at)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| AliasError::Config(error.to_string()))?;
        record.revision = stored_revision.saturating_add(1);
        record.updated_at = Utc::now();
        let checksum = record_checksum(&record)?;
        let update_columns = "UPDATE aliases SET name=?2,name_folded=?3,description=?4,target_type=?5,executable=?6,fixed_args_json=?7,pass_args=?8,working_directory=?9,environment_json=?10,shells_json=?11,enabled=?12,advanced_shell_mode=?13,tags_json=?14,path_mode=?15,path_origin=?16,updated_at=?17,record_checksum=?18,revision=?19,restriction_json=?20 WHERE id=?1";
        let optional_text = |value: Option<String>| {
            value
                .map(rusqlite::types::Value::Text)
                .unwrap_or(rusqlite::types::Value::Null)
        };
        let values = vec![
            rusqlite::types::Value::Text(record.id.to_string()),
            rusqlite::types::Value::Text(record.name.clone()),
            rusqlite::types::Value::Text(record.name.to_lowercase()),
            rusqlite::types::Value::Text(record.description.clone()),
            rusqlite::types::Value::Text(serde_json::to_string(&record.target_type)?),
            rusqlite::types::Value::Text(record.executable.clone()),
            rusqlite::types::Value::Text(serde_json::to_string(&record.fixed_args)?),
            rusqlite::types::Value::Integer(record.pass_args as i64),
            optional_text(record.working_directory.clone()),
            rusqlite::types::Value::Text(serde_json::to_string(&record.environment)?),
            rusqlite::types::Value::Text(serde_json::to_string(&record.shells)?),
            rusqlite::types::Value::Integer(record.enabled as i64),
            rusqlite::types::Value::Integer(record.advanced_shell_mode as i64),
            rusqlite::types::Value::Text(serde_json::to_string(&record.tags)?),
            rusqlite::types::Value::Text(serde_json::to_string(&record.path_mode)?),
            rusqlite::types::Value::Text(serde_json::to_string(&record.path_origin)?),
            rusqlite::types::Value::Text(record.updated_at.to_rfc3339()),
            rusqlite::types::Value::Text(checksum),
            rusqlite::types::Value::Integer(record.revision),
            rusqlite::types::Value::Text(serde_json::to_string(&record.restriction)?),
        ];
        let changed = match expected_revision {
            Some(expected) => {
                let guarded = values
                    .iter()
                    .cloned()
                    .chain(std::iter::once(rusqlite::types::Value::Integer(expected)));
                self.conn
                    .execute(
                        &format!("{update_columns} AND revision=?21"),
                        rusqlite::params_from_iter(guarded),
                    )
                    .map_err(map_constraint)?
            }
            None => self
                .conn
                .execute(update_columns, rusqlite::params_from_iter(values))
                .map_err(map_constraint)?,
        };
        if expected_revision.is_some() && changed == 0 {
            return Err(AliasError::AliasConflict(format!(
                "alias {} changed during save",
                alias.name
            )));
        }
        Ok(changed == 1)
    }

    /// Revision-checked update that also writes the supplied tombstones in the
    /// same transaction. A conflict or validation failure rolls both back, so a
    /// refused rename cannot leave a stale retired name behind.
    pub fn update_alias_checked_with_tombstones(
        &self,
        alias: &AliasRecord,
        expected_revision: i64,
        tombstones: &[(String, ShellKind, i64)],
    ) -> Result<bool, AliasError> {
        // IMMEDIATE takes the write lock before the revision read. A DEFERRED
        // transaction could read an older WAL snapshot and then fail the write
        // with SQLITE_BUSY_SNAPSHOT instead of the intended conflict path.
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let changed = self.update_alias_inner(alias, Some(expected_revision))?;
        if changed {
            for (name, shell, revision) in tombstones {
                tx.execute(
                    "INSERT INTO retired_names(name, shell, definition_kind, retired_at_revision, retired_at) VALUES (?1,?2,'function',?3,datetime('now'))",
                    params![name, serde_json::to_string(shell)?, revision],
                )?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn delete_alias(&self, id: Uuid) -> Result<bool, AliasError> {
        Ok(self
            .conn
            .execute("DELETE FROM aliases WHERE id=?1", params![id.to_string()])?
            == 1)
    }

    /// Deletes a record and records a tombstone for every shell it targeted in
    /// one transaction. `expected_revision` makes deletion conflict-safe: the
    /// record is left untouched when the stored revision has moved on.
    pub fn delete_alias_with_tombstones(
        &self,
        id: Uuid,
        expected_revision: Option<i64>,
    ) -> Result<bool, AliasError> {
        // IMMEDIATE takes the write lock before the revision read. A DEFERRED
        // transaction could read an older WAL snapshot and then fail the write
        // with SQLITE_BUSY_SNAPSHOT instead of the intended conflict path.
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let stored = tx
            .query_row(
                "SELECT name, revision, shells_json FROM aliases WHERE id = ?1",
                params![id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((name, revision, shells_json)) = stored else {
            return Ok(false);
        };
        if let Some(expected) = expected_revision {
            if revision != expected {
                return Err(AliasError::AliasConflict(format!(
                    "alias {name} changed since revision {expected} (found {revision})"
                )));
            }
        }
        let shells: Vec<ShellKind> = serde_json::from_str(&shells_json)?;
        for shell in shells {
            tx.execute(
                "INSERT INTO retired_names(name, shell, definition_kind, retired_at_revision, retired_at) VALUES (?1,?2,'function',?3,datetime('now'))",
                params![
                    name,
                    serde_json::to_string(&shell)?,
                    revision.saturating_add(1)
                ],
            )?;
        }
        let deleted = tx.execute("DELETE FROM aliases WHERE id=?1", params![id.to_string()])?;
        tx.commit()?;
        Ok(deleted == 1)
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

fn reliability_error_for(reliability: crate::config::FilesystemReliability) -> Option<AliasError> {
    match reliability {
        crate::config::FilesystemReliability::FallbackDelete => {
            Some(AliasError::UnreliableFilesystem)
        }
        crate::config::FilesystemReliability::Reliable
        | crate::config::FilesystemReliability::Unknown => None,
    }
}

/// Prunes the database backup directory to `AppConfig.backups.db_keep`,
/// counting the backup that was just written. Rotation is best-effort: a
/// failure here must never stop the database from opening.
fn rotate_database_backups(directory: &Path, newest: &Path, config_root: Option<&Path>) {
    let Some(config_root) = config_root else {
        return;
    };
    let config = crate::config::AppPaths {
        root: config_root.to_path_buf(),
    }
    .load_config()
    .unwrap_or_default();
    // `prune_oldest` keeps up to `keep` unprotected files and then leaves the
    // protected one in place, so reserve one slot for the fresh backup.
    let older_slots = config.backups.db_keep.saturating_sub(1);
    let _ = crate::rotation::prune_oldest(
        directory,
        older_slots,
        std::slice::from_ref(&newest.to_path_buf()),
    );
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
        restriction: serde_json::from_str::<Option<PathRestriction>>(&parse_json(19)?)
            .map_err(json_error(19))?,
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

    /// Unique, self-cleaning temp directory so a leftover database from an
    /// interrupted previous run cannot poison this one.
    fn temp_root(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-storage-{label}-{}-{}-{sequence}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn database_alias(name: &str) -> AliasRecord {
        checked(AliasRecord {
            name: name.into(),
            executable: "tool".into(),
            ..Default::default()
        })
    }

    #[test]
    fn restriction_round_trips_and_sql_null_maps_to_none() {
        let database = Database::open_in_memory().unwrap();
        let mut alias = database_alias("restricted");
        alias.restriction = Some(PathRestriction {
            mode: crate::model::RestrictionMode::Blacklist,
            paths: vec!["/tmp/cache".into()],
        });
        let alias = checked(alias);
        database.insert_alias(&alias).unwrap();
        let stored = database.get_alias(alias.id).unwrap().unwrap();
        assert_eq!(stored.restriction, alias.restriction);
        assert_eq!(record_checksum(&stored).unwrap(), stored.record_checksum);

        let plain = database_alias("plain");
        database.insert_alias(&plain).unwrap();
        assert_eq!(
            database.get_alias(plain.id).unwrap().unwrap().restriction,
            None
        );
        let raw: String = database
            .conn
            .query_row(
                "SELECT restriction_json FROM aliases WHERE id = ?1",
                params![plain.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw, "null");
    }

    #[test]
    fn read_only_handle_reads_a_pre_restriction_database() {
        let root = temp_root("legacy-readonly");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("aliases.db");
        let alias = database_alias("legacy");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(crate::migrations::INITIAL)
                .unwrap();
            connection.execute_batch("PRAGMA user_version = 1").unwrap();
            connection
                .execute(
                    "INSERT INTO aliases (id,name,name_folded,description,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
                    params![
                        alias.id.to_string(),
                        alias.name,
                        alias.name.to_lowercase(),
                        alias.description,
                        serde_json::to_string(&alias.target_type).unwrap(),
                        alias.executable,
                        serde_json::to_string(&alias.fixed_args).unwrap(),
                        alias.pass_args,
                        alias.working_directory,
                        serde_json::to_string(&alias.environment).unwrap(),
                        serde_json::to_string(&alias.shells).unwrap(),
                        alias.enabled,
                        alias.advanced_shell_mode,
                        serde_json::to_string(&alias.tags).unwrap(),
                        serde_json::to_string(&alias.path_mode).unwrap(),
                        serde_json::to_string(&alias.path_origin).unwrap(),
                        alias.created_at.to_rfc3339(),
                        alias.updated_at.to_rfc3339(),
                        alias.record_checksum,
                        alias.revision,
                    ],
                )
                .unwrap();
        }
        let database = Database::open_read_only(&path).unwrap();
        let listed = database.list_aliases().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "legacy");
        assert_eq!(listed[0].restriction, None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn crud_and_ordering_work_in_memory() {
        let database = Database::open_in_memory().unwrap();
        let mut first = database_alias("zeta");
        let second = database_alias("alpha");
        database.insert_alias(&first).unwrap();
        database.insert_alias(&second).unwrap();
        assert_eq!(database.get_alias(first.id).unwrap().unwrap(), first);
        assert_eq!(
            database.get_alias_by_name("alpha").unwrap().unwrap(),
            second
        );
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
        bash_alias.name = "build".into();
        bash_alias.shells = vec![crate::model::ShellKind::PowerShell5];
        bash_alias.record_checksum = record_checksum(&bash_alias).unwrap();
        assert!(matches!(
            database.update_alias(&bash_alias),
            Err(AliasError::CaseFoldConflict(name)) if name == "build"
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
        let root = temp_root("checksum");
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
                "INSERT INTO aliases (id,name,name_folded,target_type,executable,fixed_args_json,pass_args,working_directory,environment_json,shells_json,enabled,advanced_shell_mode,tags_json,path_mode,path_origin,created_at,updated_at,record_checksum,revision) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
                params![
                    legacy.id.to_string(),
                    "legacy",
                    "legacy",
                    serde_json::to_string(&legacy.target_type).unwrap(),
                    "tool",
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
            database
                .journal_mode()
                .unwrap()
                .to_ascii_lowercase()
                .as_str(),
            "wal" | "memory" | "delete"
        ));
        assert_eq!(database.synchronous_mode().unwrap(), 2);
        assert_eq!(database.busy_timeout_ms().unwrap(), 5000);
        assert!(database.foreign_keys_enabled().unwrap());
    }

    #[test]
    fn file_database_uses_wal_when_supported() {
        let root = temp_root("pragmas");
        let database = Database::open(root.join("aliases.db")).unwrap();
        assert!(matches!(
            database
                .journal_mode()
                .unwrap()
                .to_ascii_lowercase()
                .as_str(),
            "wal" | "delete"
        ));
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn migration_backup_is_created_before_reopening_existing_database() {
        let root = temp_root("backup");
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
        let root = temp_root("restore");
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
        let root = temp_root("parent");
        let path = root.join("nested").join("aliases.db");
        drop(Database::open(&path).unwrap());
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn shell_state_round_trips_for_durable_doctor_reads() {
        let database = Database::open_in_memory().unwrap();
        let state = crate::model::ShellState {
            shell: crate::model::ShellKind::Bash,
            applied_revision: 1,
            file_checksum: "abc".into(),
            loader_installed: true,
            status: crate::model::ShellStatus::Stale,
            last_error: Some("generated file is old".into()),
        };
        database.upsert_shell_state(&state).unwrap();
        assert_eq!(database.shell_states().unwrap(), vec![state]);
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
    fn record_override_is_idempotent_per_name_and_shell_and_normalizes_shell() {
        use crate::model::ShellKind;

        let database = Database::open_in_memory().unwrap();
        database
            .record_override("gs", &ShellKind::Bash, Some("alias gs='git'"), true)
            .unwrap();
        // A second scan replaces the row instead of appending a duplicate.
        database
            .record_override("gs", &ShellKind::Bash, Some("alias gs='git status'"), true)
            .unwrap();
        let rows = database.overridden_definitions().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["shell"], "bash");
        assert_eq!(rows[0]["definition_kind"], "function");
        assert!(
            rows[0].get("original_definition").is_none(),
            "raw definitions must never cross the IPC boundary"
        );

        // A different shell is a distinct override.
        database
            .record_override("gs", &ShellKind::Zsh, None, false)
            .unwrap();
        let rows = database.overridden_definitions().unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row["shell"] == "zsh"));
        assert!(rows
            .iter()
            .all(|row| !row["shell"].as_str().unwrap().contains('"')));
        assert!(rows
            .iter()
            .all(|row| row.get("original_definition").is_none()));
    }

    #[test]
    fn replace_overrides_drops_rows_without_current_definitions() {
        use crate::model::ShellKind;

        let database = Database::open_in_memory().unwrap();
        let entries = vec![OverrideEntry {
            name: "gs".into(),
            shell: ShellKind::Bash,
            definition_kind: "alias".into(),
            recoverable: true,
        }];
        database.replace_overrides(&entries).unwrap();
        // A second identical scan keeps exactly one metadata row.
        database.replace_overrides(&entries).unwrap();
        let rows = database.overridden_definitions().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["definition_kind"], "alias");
        assert_eq!(rows[0]["shell"], "bash");
        assert!(rows[0].get("original_definition").is_none());

        // The definition disappeared from the rc file: the row must go away.
        database.replace_overrides(&[]).unwrap();
        assert!(database.overridden_definitions().unwrap().is_empty());
    }

    #[test]
    fn record_override_kind_round_trips_and_stays_unique() {
        use crate::model::ShellKind;

        let database = Database::open_in_memory().unwrap();
        database
            .record_override_kind("gs", &ShellKind::Bash, "alias", None, true)
            .unwrap();
        database
            .record_override_kind("gs", &ShellKind::Bash, "function", None, false)
            .unwrap();
        let rows = database.overridden_definitions().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["definition_kind"], "function");
        assert_eq!(rows[0]["recoverable"], false);
        assert!(rows[0].get("original_definition").is_none());
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

    #[test]
    fn insert_aliases_atomic_rolls_back_every_record_on_failure() {
        let database = Database::open_in_memory().unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "taken".into(),
                executable: "git".into(),
                ..Default::default()
            })
            .unwrap();
        let first = AliasRecord {
            name: "first".into(),
            executable: "git".into(),
            ..Default::default()
        };
        let duplicate = AliasRecord {
            name: "taken".into(),
            executable: "git".into(),
            ..Default::default()
        };
        assert!(database.insert_aliases_atomic(&[first, duplicate]).is_err());
        assert_eq!(
            database
                .list_aliases()
                .unwrap()
                .iter()
                .map(|alias| alias.name.as_str())
                .collect::<Vec<_>>(),
            vec!["taken"]
        );
    }

    #[test]
    fn open_read_only_reads_without_backups_or_writes() {
        let root = temp_root("readonly");
        let path = root.join("aliases.db");
        {
            let database = Database::open(&path).unwrap();
            database
                .insert_alias(&AliasRecord {
                    name: "gs".into(),
                    executable: "git".into(),
                    ..Default::default()
                })
                .unwrap();
        }
        let database = Database::open_read_only(&path).unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 1);
        assert!(!root.join("backups/db").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn in_memory_database_reports_no_reliability_error() {
        let database = Database::open_in_memory().unwrap();
        assert!(database.root().is_none());
        assert!(database.reliability_error().is_none());
    }

    #[test]
    fn file_database_reliability_error_matches_the_filesystem_mapping() {
        let root = temp_root("reliability");
        let database = Database::open(root.join("aliases.db")).unwrap();
        let filesystem = database.root().expect("file databases know their root");
        let expected =
            reliability_error_for(crate::config::filesystem_reliability(filesystem)).is_some();
        assert_eq!(database.reliability_error().is_some(), expected);
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fallback_delete_reliability_maps_to_unreliable_filesystem() {
        use crate::config::FilesystemReliability;
        assert!(matches!(
            reliability_error_for(FilesystemReliability::FallbackDelete),
            Some(AliasError::UnreliableFilesystem)
        ));
        assert!(reliability_error_for(FilesystemReliability::Reliable).is_none());
        assert!(reliability_error_for(FilesystemReliability::Unknown).is_none());
    }

    fn backup_names(directory: &Path) -> std::collections::BTreeSet<String> {
        std::fs::read_dir(directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn reopening_a_database_rotates_old_backups_to_the_configured_limit() {
        let root = temp_root("rotate");
        let path = root.join("aliases.db");
        {
            let database = Database::open(&path).unwrap();
            database
                .insert_alias(&AliasRecord {
                    name: "kept".into(),
                    executable: "git".into(),
                    ..Default::default()
                })
                .unwrap();
        }
        let mut config = crate::config::AppConfig::default();
        config.backups.db_keep = 2;
        config
            .save(&crate::config::AppPaths { root: root.clone() })
            .unwrap();
        let backups = root.join("backups/db");
        std::fs::create_dir_all(&backups).unwrap();
        for index in 0..10 {
            std::fs::write(backups.join(format!("aliases-fake-{index}.db")), "fake").unwrap();
        }
        let before = backup_names(&backups);
        let database = Database::open(&path).unwrap();
        let after = backup_names(&backups);
        assert_eq!(
            after.len(),
            2,
            "expected exactly db_keep (2) backups, found {after:?}"
        );
        let fresh = after
            .iter()
            .find(|name| !before.contains(*name))
            .expect("the newly created backup must be retained");
        assert!(fresh.starts_with("aliases-") && fresh.ends_with(".db"));
        assert_eq!(
            database
                .get_alias_by_name("kept")
                .unwrap()
                .map(|alias| alias.name),
            Some("kept".to_string())
        );
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_migration_does_not_prune_existing_backups() {
        let root = temp_root("corrupt");
        let path = root.join("aliases.db");
        let database = Database::open(&path).unwrap();
        database
            .conn
            .execute("PRAGMA user_version = 999", [])
            .unwrap();
        drop(database);
        let backups = root.join("backups/db");
        std::fs::create_dir_all(&backups).unwrap();
        let mut seeded = std::collections::BTreeSet::new();
        for index in 0..10 {
            let name = format!("aliases-good-{index}.db");
            std::fs::write(backups.join(&name), "good").unwrap();
            seeded.insert(name);
        }
        assert!(matches!(
            Database::open(&path),
            Err(AliasError::SchemaTooNew)
        ));
        let after = backup_names(&backups);
        assert!(
            seeded.is_subset(&after),
            "a failed open pruned good backups: {after:?}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bundled_sqlite_contains_the_wal_reset_corruption_fix() {
        let database = Database::open_in_memory().unwrap();
        let version: String = database
            .conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .unwrap();
        let parts = version
            .split('.')
            .map(|part| part.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>();
        let tuple = (
            *parts.first().unwrap_or(&0),
            *parts.get(1).unwrap_or(&0),
            *parts.get(2).unwrap_or(&0),
        );
        assert!(
            tuple >= (3, 51, 3),
            "bundled SQLite {version} predates the WAL-reset database corruption fix (>=3.51.3 required)"
        );
    }

    #[test]
    fn import_aliases_atomic_rolls_back_mid_batch_conflicts() {
        let database = Database::open_in_memory().unwrap();
        let mut first = AliasRecord {
            name: "one".into(),
            executable: "first".into(),
            ..Default::default()
        };
        first.record_checksum = record_checksum(&first).unwrap();
        let mut duplicate = AliasRecord {
            name: "one".into(),
            executable: "second".into(),
            ..Default::default()
        };
        duplicate.record_checksum = record_checksum(&duplicate).unwrap();
        assert!(database
            .import_aliases_atomic(&[first, duplicate], &BTreeMap::new())
            .is_err());
        assert!(database.list_aliases().unwrap().is_empty());
    }

    #[test]
    fn import_aliases_atomic_does_not_overwrite_a_row_inserted_after_preview() {
        let database = Database::open_in_memory().unwrap();
        let mut winner = AliasRecord {
            name: "one".into(),
            executable: "external".into(),
            ..Default::default()
        };
        winner.record_checksum = record_checksum(&winner).unwrap();
        database.insert_alias(&winner).unwrap();
        let mut incoming = AliasRecord {
            name: "one".into(),
            executable: "imported".into(),
            ..Default::default()
        };
        incoming.record_checksum = record_checksum(&incoming).unwrap();
        // Preview saw no row for "one"; another front end inserted `winner`
        // before our BEGIN IMMEDIATE. Inserting must conflict, not update.
        assert!(matches!(
            database.import_aliases_atomic(&[incoming], &BTreeMap::new()),
            Err(AliasError::ExactNameConflict(name)) if name == "one"
        ));
        assert_eq!(
            database
                .get_alias_by_name("one")
                .unwrap()
                .unwrap()
                .executable,
            "external"
        );
    }

    #[test]
    fn import_aliases_atomic_overwrite_updates_existing_rows() {
        let database = Database::open_in_memory().unwrap();
        let mut original = AliasRecord {
            name: "gs".into(),
            executable: "old".into(),
            shells: vec![ShellKind::Bash, ShellKind::Zsh],
            ..Default::default()
        };
        original.record_checksum = record_checksum(&original).unwrap();
        database.insert_alias(&original).unwrap();
        let mut incoming = original.clone();
        incoming.id = Uuid::new_v4();
        incoming.executable = "new".into();
        incoming.shells = vec![ShellKind::Bash];
        incoming.record_checksum = record_checksum(&incoming).unwrap();
        let overwrite = BTreeMap::from([("gs".to_string(), original.id)]);
        database
            .import_aliases_atomic(std::slice::from_ref(&incoming), &overwrite)
            .unwrap();
        let saved = database.get_alias(original.id).unwrap().unwrap();
        assert_eq!(saved.id, original.id);
        assert_eq!(saved.executable, "new");
        assert_eq!(saved.revision, original.revision + 1);
        assert!(database.get_alias(incoming.id).unwrap().is_none());
        assert_eq!(database.list_aliases().unwrap().len(), 1);
        let zsh = database.managed_name_set(&ShellKind::Zsh).unwrap();
        assert_eq!(zsh.retired, vec!["gs".to_string()]);
    }
}
