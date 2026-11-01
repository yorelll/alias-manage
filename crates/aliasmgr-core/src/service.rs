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
        self.database.get_alias(alias.id)?.ok_or_else(|| AliasError::Config("alias not found".into()))
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

    #[test]
    fn update_keeps_created_at_when_caller_supplies_now() {
        let database = Database::open_in_memory().unwrap();
        let service = AliasService::new(&database);
        let original = AliasRecord {
            name: "gui".into(),
            executable: "tool".into(),
            ..Default::default()
        };
        database.insert_alias(&original).unwrap();
        let stored = database.get_alias(original.id).unwrap().unwrap();
        // Simulates the GUI DTO path: same id, created_at from AliasRecord::default()
        // ("now"), an empty checksum, and a changed description.
        let replacement = AliasRecord {
            id: original.id,
            name: original.name.clone(),
            executable: original.executable.clone(),
            description: "changed by gui".into(),
            created_at: AliasRecord::default().created_at,
            record_checksum: String::new(),
            ..Default::default()
        };
        let saved = service.update(replacement).unwrap();
        assert_eq!(saved.created_at, stored.created_at);
        assert_eq!(saved.record_checksum, record_checksum(&saved).unwrap());
        let listed = service.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].created_at, stored.created_at);
        assert_eq!(listed[0].description, "changed by gui");
    }
}
