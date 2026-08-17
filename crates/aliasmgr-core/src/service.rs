use crate::{
    error::AliasError,
    model::{record_checksum, AliasRecord},
    search::{search, SearchQuery, SearchResult},
    storage::Database,
    validation::validate_alias,
};
use std::collections::BTreeMap;

pub struct AliasService<'a> {
    pub database: &'a Database,
}

impl<'a> AliasService<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    pub fn list(&self) -> Result<Vec<AliasRecord>, AliasError> {
        self.database.list_aliases()
    }

    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AliasError> {
        Ok(search(&self.database.list_aliases()?, query))
    }

    pub fn tag_counts(&self) -> Result<BTreeMap<String, usize>, AliasError> {
        let mut counts = BTreeMap::new();
        for alias in self.database.list_aliases()? {
            for tag in alias.tags {
                *counts.entry(tag).or_insert(0) += 1;
            }
        }
        Ok(counts)
    }

    pub fn create(&self, mut alias: AliasRecord) -> Result<AliasRecord, AliasError> {
        validate_alias(&alias)?;
        alias.record_checksum = record_checksum(&alias)?;
        self.database.insert_alias(&alias)?;
        Ok(alias)
    }

    pub fn update(&self, mut alias: AliasRecord) -> Result<AliasRecord, AliasError> {
        validate_alias(&alias)?;
        alias.record_checksum = record_checksum(&alias)?;
        if !self.database.update_alias(&alias)? {
            return Err(AliasError::Config("alias not found".into()));
        }
        Ok(alias)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_lists_searches_and_counts_tags() {
        let database = Database::open_in_memory().unwrap();
        let mut first = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        };
        first.tags = vec!["git".into(), "work".into()];
        first.record_checksum = record_checksum(&first).unwrap();
        database.insert_alias(&first).unwrap();

        let service = AliasService::new(&database);
        assert_eq!(service.list().unwrap()[0].name, "gs");
        let query = SearchQuery {
            query: "gs".into(),
            tag_filter: vec!["git".into(), "work".into()],
            ..Default::default()
        };
        assert_eq!(service.search(&query).unwrap()[0].alias.name, "gs");
        assert_eq!(service.tag_counts().unwrap().get("git"), Some(&1));
    }

    #[test]
    fn service_create_rejects_invalid_records() {
        let database = Database::open_in_memory().unwrap();
        let service = AliasService::new(&database);
        let invalid = AliasRecord {
            name: "bad name".into(),
            executable: "tool".into(),
            ..Default::default()
        };
        assert!(matches!(
            service.create(invalid),
            Err(AliasError::InvalidAliasName)
        ));
    }

    #[test]
    fn service_update_recalculates_checksum() {
        let database = Database::open_in_memory().unwrap();
        let service = AliasService::new(&database);
        let mut alias = AliasRecord {
            name: "one".into(),
            executable: "tool".into(),
            ..Default::default()
        };
        alias.record_checksum = record_checksum(&alias).unwrap();
        database.insert_alias(&alias).unwrap();
        alias.description = "updated".into();
        let saved = service.update(alias).unwrap();
        assert_eq!(saved.record_checksum, record_checksum(&saved).unwrap());
    }
}
