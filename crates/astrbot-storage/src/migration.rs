use std::collections::HashSet;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationOperation {
    CreateTable(String),
    AddColumn { table: String, column: String },
    CreateIndex { name: String, table: String },
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationRecord {
    pub migration_id: String,
    pub checksum: Option<String>,
}

impl MigrationRecord {
    pub fn new(migration_id: impl Into<String>) -> Self {
        Self {
            migration_id: migration_id.into(),
            checksum: None,
        }
    }

    pub fn with_checksum(mut self, checksum: impl Into<String>) -> Self {
        self.checksum = Some(checksum.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationOutcome {
    pub applied: Vec<MigrationRecord>,
    pub skipped: Vec<String>,
}

impl MigrationOutcome {
    pub fn is_empty(&self) -> bool {
        self.applied.is_empty() && self.skipped.is_empty()
    }
}

#[async_trait]
pub trait MigrationStateRepository: Send + Sync {
    async fn applied_migrations(&self) -> Result<Vec<MigrationRecord>>;

    async fn record_migration(&self, record: MigrationRecord) -> Result<()>;
}

#[derive(Default)]
pub struct InMemoryMigrationStateRepository {
    records: RwLock<Vec<MigrationRecord>>,
}

impl InMemoryMigrationStateRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MigrationStateRepository for InMemoryMigrationStateRepository {
    async fn applied_migrations(&self) -> Result<Vec<MigrationRecord>> {
        self.records
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("migration state lock: {err}")))
            .map(|records| records.clone())
    }

    async fn record_migration(&self, record: MigrationRecord) -> Result<()> {
        let mut records = self
            .records
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("migration state lock: {err}")))?;
        if !records
            .iter()
            .any(|current| current.migration_id == record.migration_id)
        {
            records.push(record);
        }
        Ok(())
    }
}

#[async_trait]
pub trait StorageMigration: Send + Sync {
    fn id(&self) -> &str;

    fn operations(&self) -> &[MigrationOperation];

    fn checksum(&self) -> Option<&str> {
        None
    }

    async fn apply(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarativeMigration {
    id: String,
    checksum: Option<String>,
    operations: Vec<MigrationOperation>,
}

impl DeclarativeMigration {
    pub fn new(id: impl Into<String>, operations: Vec<MigrationOperation>) -> Self {
        Self {
            id: id.into(),
            checksum: None,
            operations,
        }
    }

    pub fn with_checksum(mut self, checksum: impl Into<String>) -> Self {
        self.checksum = Some(checksum.into());
        self
    }
}

#[async_trait]
impl StorageMigration for DeclarativeMigration {
    fn id(&self) -> &str {
        &self.id
    }

    fn operations(&self) -> &[MigrationOperation] {
        &self.operations
    }

    fn checksum(&self) -> Option<&str> {
        self.checksum.as_deref()
    }
}

pub struct MigrationRunner<'a> {
    state: &'a dyn MigrationStateRepository,
    migrations: Vec<Box<dyn StorageMigration>>,
}

impl<'a> MigrationRunner<'a> {
    pub fn new(state: &'a dyn MigrationStateRepository) -> Self {
        Self {
            state,
            migrations: Vec::new(),
        }
    }

    pub fn with_migration(mut self, migration: impl StorageMigration + 'static) -> Self {
        self.migrations.push(Box::new(migration));
        self
    }

    pub async fn run(mut self) -> Result<MigrationOutcome> {
        let applied_ids = self
            .state
            .applied_migrations()
            .await?
            .into_iter()
            .map(|record| record.migration_id)
            .collect::<HashSet<_>>();
        let mut outcome = MigrationOutcome::default();

        self.migrations
            .sort_by(|left, right| left.id().cmp(right.id()));

        for migration in self.migrations {
            if applied_ids.contains(migration.id()) {
                outcome.skipped.push(migration.id().to_string());
                continue;
            }

            migration.apply().await?;
            let mut record = MigrationRecord::new(migration.id());
            if let Some(checksum) = migration.checksum() {
                record = record.with_checksum(checksum);
            }
            self.state.record_migration(record.clone()).await?;
            outcome.applied.push(record);
        }

        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DeclarativeMigration, InMemoryMigrationStateRepository, MigrationOperation,
        MigrationRunner, MigrationStateRepository,
    };

    #[tokio::test]
    async fn migration_runner_applies_each_pending_migration_once() {
        let state = InMemoryMigrationStateRepository::new();
        let migration = DeclarativeMigration::new(
            "001-create-conversations",
            vec![MigrationOperation::CreateTable(
                "conversation_messages".to_string(),
            )],
        );

        let first = MigrationRunner::new(&state)
            .with_migration(migration.clone())
            .run()
            .await
            .expect("migration should run");
        let second = MigrationRunner::new(&state)
            .with_migration(migration)
            .run()
            .await
            .expect("migration should skip");

        assert_eq!(first.applied.len(), 1);
        assert_eq!(second.skipped, vec!["001-create-conversations"]);
        assert_eq!(
            state
                .applied_migrations()
                .await
                .expect("state should load")
                .len(),
            1
        );
    }
}
