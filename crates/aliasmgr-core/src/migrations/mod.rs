use crate::error::AliasError;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

pub const CURRENT_VERSION: i64 = 2;
pub const INITIAL: &str = include_str!("0001_initial.sql");
pub const RESTRICTION: &str = include_str!("0002_restriction.sql");

/// Ordered migrations. Each entry is applied once, in order, when the database
/// is older than the entry's version. A v1 database therefore runs only the
/// restriction migration instead of replaying `0001_initial.sql`.
const MIGRATIONS: &[(i64, &str)] = &[(1, INITIAL), (2, RESTRICTION)];

pub fn migrate(connection: &Connection) -> Result<(), AliasError> {
    let current: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > CURRENT_VERSION {
        return Err(AliasError::SchemaTooNew);
    }
    if current == CURRENT_VERSION {
        return Ok(());
    }
    let transaction = connection.unchecked_transaction()?;
    for (version, sql) in MIGRATIONS.iter().filter(|(version, _)| *version > current) {
        transaction.execute_batch(sql)?;
        let mut digest = Sha256::new();
        digest.update(sql.as_bytes());
        let checksum = format!("{:x}", digest.finalize());
        transaction.execute(
            "INSERT INTO schema_migrations(version, applied_at, checksum) VALUES (?1, datetime('now'), ?2)",
            rusqlite::params![version, checksum],
        )?;
    }
    transaction.execute_batch(&format!("PRAGMA user_version = {CURRENT_VERSION}"))?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_schema_is_rejected_without_creating_tables() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA user_version = 999")
            .unwrap();
        assert!(matches!(
            migrate(&connection),
            Err(AliasError::SchemaTooNew)
        ));
        assert!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='aliases'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap()
                == 0
        );
    }

    fn applied_versions(connection: &Connection) -> Vec<i64> {
        let mut statement = connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn fresh_database_records_every_migration() {
        let connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        assert_eq!(applied_versions(&connection), vec![1, 2]);
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            CURRENT_VERSION
        );
    }

    #[test]
    fn v1_database_upgrades_without_replaying_the_initial_migration() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(INITIAL).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations(version, applied_at, checksum) VALUES (1, datetime('now'), 'legacy')",
                [],
            )
            .unwrap();
        connection.execute_batch("PRAGMA user_version = 1").unwrap();

        migrate(&connection).unwrap();

        assert_eq!(applied_versions(&connection), vec![1, 2]);
        connection
            .execute(
                "INSERT INTO aliases (id,name,name_folded,target_type,executable,fixed_args_json,shells_json,created_at,updated_at) VALUES ('x','legacy','legacy','native_executable','tool','[]','[\"bash\"]',datetime('now'),datetime('now'))",
                [],
            )
            .unwrap();
        let restriction: String = connection
            .query_row(
                "SELECT restriction_json FROM aliases WHERE id='x'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(restriction, "null");
    }
}
