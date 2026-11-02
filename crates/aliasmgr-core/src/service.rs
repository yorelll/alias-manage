use crate::{
    error::AliasError,
    model::{normalize_restriction, record_checksum, AliasRecord, ShellKind},
    search::{search, SearchQuery, SearchResult},
    storage::Database,
    validation::validate_alias,
};
use std::collections::BTreeMap;
use uuid::Uuid;

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
        normalize_restriction(&mut alias.restriction);
        validate_alias(&alias)?;
        alias.record_checksum = record_checksum(&alias)?;
        self.database.insert_alias(&alias)?;
        Ok(alias)
    }

    pub fn update(&self, mut alias: AliasRecord) -> Result<AliasRecord, AliasError> {
        normalize_restriction(&mut alias.restriction);
        validate_alias(&alias)?;
        alias.record_checksum = record_checksum(&alias)?;
        if !self.database.update_alias(&alias)? {
            return Err(AliasError::Config("alias not found".into()));
        }
        self.database
            .get_alias(alias.id)?
            .ok_or_else(|| AliasError::Config("alias not found".into()))
    }

    pub fn get(&self, id: Uuid) -> Result<Option<AliasRecord>, AliasError> {
        self.database.get_alias(id)
    }

    pub fn get_by_name(&self, name: &str) -> Result<Option<AliasRecord>, AliasError> {
        self.database.get_alias_by_name(name)
    }

    /// Update with an optimistic-concurrency guard. The stored revision must
    /// still equal `expected_revision`; otherwise the write is refused with
    /// `AliasConflict` and nothing is changed.
    ///
    /// A rename (or a shell removed from the record) retires the old name for
    /// the affected shells, matching the CLI's `rename`/`update` semantics, so
    /// the next sync emits the cleanup block.
    pub fn update_checked(
        &self,
        mut alias: AliasRecord,
        expected_revision: i64,
    ) -> Result<AliasRecord, AliasError> {
        normalize_restriction(&mut alias.restriction);
        validate_alias(&alias)?;
        alias.record_checksum = record_checksum(&alias)?;
        let current = self
            .database
            .get_alias(alias.id)?
            .ok_or_else(|| AliasError::NotFound(alias.name.clone()))?;
        if current.revision != expected_revision {
            return Err(AliasError::AliasConflict(format!(
                "alias {} changed since revision {} (found {})",
                current.name, expected_revision, current.revision
            )));
        }
        let retire_shells: Vec<ShellKind> = if current.name != alias.name {
            current.shells.clone()
        } else {
            current
                .shells
                .iter()
                .filter(|shell| !alias.shells.contains(shell))
                .cloned()
                .collect()
        };
        let tombstones = retire_shells
            .iter()
            .map(|shell| {
                (
                    current.name.clone(),
                    shell.clone(),
                    current.revision.saturating_add(1),
                )
            })
            .collect::<Vec<_>>();
        // The revision guard, the row update and the tombstones commit (or roll
        // back) together, so a refused rename cannot leave a stale retired name.
        if !self.database.update_alias_checked_with_tombstones(
            &alias,
            expected_revision,
            &tombstones,
        )? {
            return Err(AliasError::NotFound(alias.name.clone()));
        }
        self.database
            .get_alias(alias.id)?
            .ok_or_else(|| AliasError::NotFound(alias.name.clone()))
    }

    /// Delete a record and retire its name for every bound shell.
    pub fn delete(&self, name: &str) -> Result<bool, AliasError> {
        let alias = self
            .get_by_name(name)?
            .ok_or_else(|| AliasError::NotFound(name.into()))?;
        self.database.delete_alias_with_tombstones(alias.id, None)
    }

    /// Delete with an optimistic-concurrency guard. The tombstone insertion
    /// and the delete happen in one transaction.
    pub fn delete_checked(
        &self,
        alias: &AliasRecord,
        expected_revision: i64,
    ) -> Result<bool, AliasError> {
        self.database
            .delete_alias_with_tombstones(alias.id, Some(expected_revision))
    }

    pub fn rename(&self, old: &str, new: &str) -> Result<AliasRecord, AliasError> {
        let alias = self
            .get_by_name(old)?
            .ok_or_else(|| AliasError::NotFound(old.into()))?;
        let revision = alias.revision;
        self.rename_checked(&alias, new, revision)
    }

    pub fn rename_checked(
        &self,
        alias: &AliasRecord,
        new_name: &str,
        expected_revision: i64,
    ) -> Result<AliasRecord, AliasError> {
        let mut updated = alias.clone();
        updated.name = new_name.to_string();
        self.update_checked(updated, expected_revision)
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<AliasRecord, AliasError> {
        let alias = self
            .get_by_name(name)?
            .ok_or_else(|| AliasError::NotFound(name.into()))?;
        let revision = alias.revision;
        self.set_enabled_checked(&alias, enabled, revision)
    }

    pub fn set_enabled_checked(
        &self,
        alias: &AliasRecord,
        enabled: bool,
        expected_revision: i64,
    ) -> Result<AliasRecord, AliasError> {
        let mut updated = alias.clone();
        updated.enabled = enabled;
        self.update_checked(updated, expected_revision)
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

    #[test]
    fn service_normalizes_restriction_paths_on_create_and_update() {
        let database = Database::open_in_memory().unwrap();
        let service = AliasService::new(&database);
        let path = std::env::temp_dir()
            .join("aliasmgr-service-restriction")
            .to_string_lossy()
            .into_owned();
        let created = service
            .create(AliasRecord {
                name: "rma".into(),
                executable: "rm".into(),
                restriction: Some(crate::model::PathRestriction {
                    mode: crate::model::RestrictionMode::Whitelist,
                    paths: vec![format!(" {path} "), path.clone()],
                }),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            created.restriction.as_ref().unwrap().paths,
            vec![path.clone()]
        );

        let mut update = created;
        update.restriction = Some(crate::model::PathRestriction {
            mode: crate::model::RestrictionMode::Blacklist,
            paths: vec![format!("  {path}  ")],
        });
        let saved = service.update(update).unwrap();
        assert_eq!(
            saved.restriction.as_ref().unwrap().paths,
            vec![path.clone()]
        );
        assert_eq!(
            saved.restriction.as_ref().unwrap().mode,
            crate::model::RestrictionMode::Blacklist
        );
    }

    fn seeded(database: &Database, name: &str) -> AliasRecord {
        AliasService::new(database)
            .create(AliasRecord {
                name: name.into(),
                executable: "git".into(),
                fixed_args: vec!["status".into()],
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap()
    }

    fn temp_database(label: &str) -> (std::path::PathBuf, Database) {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-service-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        (root, database)
    }

    #[test]
    fn get_by_name_returns_the_record_or_none() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        let fetched = service.get_by_name("gs").unwrap().unwrap();
        assert_eq!(fetched.id, created.id);
        assert!(service.get_by_name("missing").unwrap().is_none());
        assert_eq!(service.get(created.id).unwrap().unwrap().name, "gs");
    }

    #[test]
    fn delete_retires_every_shell_and_removes_the_row() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        assert!(service.delete("gs").unwrap());
        assert!(service.get(created.id).unwrap().is_none());
        for shell in [ShellKind::Bash, ShellKind::Zsh] {
            let names = database.managed_name_set(&shell).unwrap();
            assert_eq!(names.retired, vec!["gs".to_string()]);
            assert!(names.current.is_empty());
        }
        assert_eq!(database.max_retired_revision().unwrap(), 2);
    }

    #[test]
    fn delete_reports_missing_names_as_not_found() {
        let database = Database::open_in_memory().unwrap();
        let service = AliasService::new(&database);
        assert!(matches!(
            service.delete("missing"),
            Err(AliasError::NotFound(name)) if name == "missing"
        ));
    }

    #[test]
    fn rename_retires_the_old_name_and_updates_the_record() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        let renamed = service.rename("gs", "git-status").unwrap();
        assert_eq!(renamed.name, "git-status");
        assert_eq!(renamed.id, created.id);
        assert_eq!(renamed.revision, created.revision + 1);
        assert!(service.get_by_name("gs").unwrap().is_none());
        let names = database.managed_name_set(&ShellKind::Bash).unwrap();
        assert_eq!(names.retired, vec!["gs".to_string()]);
        assert_eq!(names.current, vec!["git-status".to_string()]);
    }

    #[test]
    fn rename_to_an_existing_name_is_a_conflict_and_keeps_both_records() {
        let database = Database::open_in_memory().unwrap();
        let service = AliasService::new(&database);
        seeded(&database, "gs");
        seeded(&database, "other");
        assert!(matches!(
            service.rename("gs", "other"),
            Err(AliasError::ExactNameConflict(name)) if name == "other"
        ));
        assert!(service.get_by_name("gs").unwrap().is_some());
        assert!(service.get_by_name("other").unwrap().is_some());
    }

    #[test]
    fn set_enabled_toggles_state_and_bumps_revision() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        let disabled = service.set_enabled("gs", false).unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.revision, created.revision + 1);
        let enabled = service.set_enabled("gs", true).unwrap();
        assert!(enabled.enabled);
        assert_eq!(enabled.revision, created.revision + 2);
        assert_eq!(
            enabled.record_checksum,
            crate::model::record_checksum(&enabled).unwrap()
        );
    }

    #[test]
    fn update_checked_rejects_a_stale_revision_without_writing() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        let mut stale = created.clone();
        stale.description = "stale edit".into();
        assert!(matches!(
            service.update_checked(stale, created.revision + 5),
            Err(AliasError::AliasConflict(message)) if message.contains("changed since revision")
        ));
        let stored = service.get(created.id).unwrap().unwrap();
        assert_eq!(stored.description, "");
        assert_eq!(stored.revision, created.revision);
    }

    #[test]
    fn update_checked_writes_once_and_preserves_created_at() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        let mut edit = created.clone();
        edit.description = "edited".into();
        edit.fixed_args = vec!["log".into(), "--oneline".into()];
        let saved = service.update_checked(edit, created.revision).unwrap();
        assert_eq!(saved.description, "edited");
        assert_eq!(saved.fixed_args, vec!["log", "--oneline"]);
        assert_eq!(saved.created_at, created.created_at);
        assert_eq!(saved.revision, created.revision + 1);
        assert_eq!(
            saved.record_checksum,
            crate::model::record_checksum(&saved).unwrap()
        );
    }

    #[test]
    fn update_checked_retires_a_renamed_record_and_removed_shells() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        let mut edit = created.clone();
        edit.name = "git-status".into();
        edit.shells = vec![ShellKind::Bash];
        let saved = service.update_checked(edit, created.revision).unwrap();
        assert_eq!(saved.name, "git-status");
        let bash = database.managed_name_set(&ShellKind::Bash).unwrap();
        assert_eq!(bash.current, vec!["git-status".to_string()]);
        assert_eq!(bash.retired, vec!["gs".to_string()]);
        let zsh = database.managed_name_set(&ShellKind::Zsh).unwrap();
        assert!(zsh.current.is_empty());
        assert_eq!(zsh.retired, vec!["gs".to_string()]);
    }

    #[test]
    fn update_checked_validates_before_retiring_names() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        let mut edit = created.clone();
        edit.name = "bad name".into();
        assert!(matches!(
            service.update_checked(edit, created.revision),
            Err(AliasError::InvalidAliasName)
        ));
        assert!(database
            .managed_name_set(&ShellKind::Bash)
            .unwrap()
            .retired
            .is_empty());
        assert_eq!(service.get(created.id).unwrap().unwrap().name, "gs");
    }

    #[test]
    fn a_conflicting_rename_does_not_leave_a_tombstone() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        // Another writer wins first.
        let mut external = created.clone();
        external.description = "external".into();
        service.update_checked(external, created.revision).unwrap();
        // A stale rename must roll back both the row and the tombstone.
        let mut stale = created.clone();
        stale.name = "renamed".into();
        assert!(matches!(
            service.update_checked(stale, created.revision),
            Err(AliasError::AliasConflict(_))
        ));
        assert!(database
            .managed_name_set(&ShellKind::Bash)
            .unwrap()
            .retired
            .is_empty());
        assert_eq!(service.get(created.id).unwrap().unwrap().name, "gs");
    }

    #[test]
    fn delete_checked_rejects_a_stale_revision_without_tombstones() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        assert!(matches!(
            service.delete_checked(&created, created.revision + 3),
            Err(AliasError::AliasConflict(_))
        ));
        assert!(service.get(created.id).unwrap().is_some());
        assert!(database
            .managed_name_set(&ShellKind::Bash)
            .unwrap()
            .retired
            .is_empty());
        assert_eq!(database.max_retired_revision().unwrap(), 0);
    }

    #[test]
    fn delete_checked_removes_the_record_and_writes_tombstones() {
        let database = Database::open_in_memory().unwrap();
        let created = seeded(&database, "gs");
        let service = AliasService::new(&database);
        assert!(service.delete_checked(&created, created.revision).unwrap());
        assert!(service.get(created.id).unwrap().is_none());
        assert_eq!(
            database.managed_name_set(&ShellKind::Bash).unwrap().retired,
            vec!["gs".to_string()]
        );
    }

    #[test]
    fn two_connections_cannot_overwrite_each_other() {
        let (root, database) = temp_database("two-connections");
        let created = seeded(&database, "gs");
        let second = Database::open(root.join("aliases.db")).unwrap();
        let mut first_edit = created.clone();
        first_edit.description = "from first".into();
        AliasService::new(&database)
            .update_checked(first_edit, created.revision)
            .unwrap();
        let mut second_edit = created.clone();
        second_edit.description = "from second".into();
        assert!(matches!(
            AliasService::new(&second).update_checked(second_edit, created.revision),
            Err(AliasError::AliasConflict(_))
        ));
        assert_eq!(
            AliasService::new(&database)
                .get(created.id)
                .unwrap()
                .unwrap()
                .description,
            "from first"
        );
        drop(second);
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }
}
