use crate::error::AliasError;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

pub const CURRENT_VERSION: i64 = 1;
pub const INITIAL: &str = include_str!("0001_initial.sql");

pub fn migrate(connection: &Connection) -> Result<(), AliasError> {
    let current: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > CURRENT_VERSION { return Err(AliasError::SchemaTooNew); }
    if current == CURRENT_VERSION { return Ok(()); }
    let mut digest = Sha256::new();
    digest.update(INITIAL.as_bytes());
    let checksum = format!("{:x}", digest.finalize());
    let transaction = connection.unchecked_transaction()?;
    transaction.execute_batch(INITIAL)?;
    transaction.execute("INSERT INTO schema_migrations(version, applied_at, checksum) VALUES (?1, datetime('now'), ?2)", rusqlite::params![CURRENT_VERSION, checksum])?;
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
        connection.execute_batch("PRAGMA user_version = 999").unwrap();
        assert!(matches!(migrate(&connection), Err(AliasError::SchemaTooNew)));
        assert!(connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='aliases'", [], |row| row.get::<_, i64>(0)).unwrap() == 0);
    }
}
