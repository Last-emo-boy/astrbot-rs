use std::collections::BTreeMap;

use astrbot_core::Result;
use astrbot_runtime::RuntimeConfigMigrationPlan;
use astrbot_storage::{MigrationOutcome, MigrationRecord};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::operation::{
    MaintenanceOperationId, MaintenanceOperationKind, MaintenanceOperationProgress,
    MaintenanceOperationStore, MaintenanceOperationSummary,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfigMigrationDescriptor {
    pub missing_default_keys: Vec<String>,
}

impl RuntimeConfigMigrationDescriptor {
    pub fn is_empty(&self) -> bool {
        self.missing_default_keys.is_empty()
    }
}

impl From<RuntimeConfigMigrationPlan> for RuntimeConfigMigrationDescriptor {
    fn from(plan: RuntimeConfigMigrationPlan) -> Self {
        Self {
            missing_default_keys: plan.missing_default_keys,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceMigrationCheck {
    pub runtime_config: RuntimeConfigMigrationDescriptor,
    pub pending_storage_migrations: Vec<String>,
    pub legacy_data_migration_needed: bool,
}

impl MaintenanceMigrationCheck {
    pub fn is_needed(&self) -> bool {
        !self.runtime_config.is_empty()
            || !self.pending_storage_migrations.is_empty()
            || self.legacy_data_migration_needed
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceMigrationRequest {
    pub platform_id_map: BTreeMap<String, BTreeMap<String, String>>,
    pub confirmed: bool,
}

impl MaintenanceMigrationRequest {
    pub fn confirmed(platform_id_map: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        Self {
            platform_id_map,
            confirmed: true,
        }
    }
}

#[async_trait]
pub trait MaintenanceMigrationService: Send + Sync {
    async fn check_migration(&self) -> Result<MaintenanceMigrationCheck>;

    async fn run_migration(
        &self,
        request: MaintenanceMigrationRequest,
    ) -> Result<MaintenanceOperationSummary>;
}

pub struct PlannedMigrationService<'a> {
    operations: &'a dyn MaintenanceOperationStore,
    runtime_config: RuntimeConfigMigrationDescriptor,
    pending_storage_migrations: Vec<String>,
    legacy_data_migration_needed: bool,
}

impl<'a> PlannedMigrationService<'a> {
    pub fn new(operations: &'a dyn MaintenanceOperationStore) -> Self {
        Self {
            operations,
            runtime_config: RuntimeConfigMigrationPlan::default().into(),
            pending_storage_migrations: Vec::new(),
            legacy_data_migration_needed: false,
        }
    }

    pub fn with_runtime_config_plan(mut self, plan: RuntimeConfigMigrationPlan) -> Self {
        self.runtime_config = plan.into();
        self
    }

    pub fn with_pending_storage_migrations<I, S>(mut self, migrations: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.pending_storage_migrations = migrations.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_legacy_data_migration_needed(mut self, needed: bool) -> Self {
        self.legacy_data_migration_needed = needed;
        self
    }
}

#[async_trait]
impl MaintenanceMigrationService for PlannedMigrationService<'_> {
    async fn check_migration(&self) -> Result<MaintenanceMigrationCheck> {
        Ok(MaintenanceMigrationCheck {
            runtime_config: self.runtime_config.clone(),
            pending_storage_migrations: self.pending_storage_migrations.clone(),
            legacy_data_migration_needed: self.legacy_data_migration_needed,
        })
    }

    async fn run_migration(
        &self,
        request: MaintenanceMigrationRequest,
    ) -> Result<MaintenanceOperationSummary> {
        let operation_id = MaintenanceOperationId::new("migration")
            .expect("static migration operation id should be valid");
        let mut progress = MaintenanceOperationProgress::queued();
        if !request.confirmed {
            progress = progress.failed("migration requires explicit confirmation");
        } else {
            progress = progress.running("migration planned").completed("migration completed");
        }
        let summary =
            MaintenanceOperationSummary::new(operation_id, MaintenanceOperationKind::Migration)
                .with_progress(progress);
        self.operations.put_operation(summary.clone()).await?;
        Ok(summary)
    }
}

pub fn migration_records_to_outcome(records: Vec<MigrationRecord>) -> MigrationOutcome {
    MigrationOutcome {
        applied: records,
        skipped: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use astrbot_runtime::RuntimeConfigMigrationPlan;

    use super::{MaintenanceMigrationRequest, MaintenanceMigrationService, PlannedMigrationService};
    use crate::operation::{InMemoryMaintenanceOperationStore, MaintenanceOperationStatus};

    #[tokio::test]
    async fn migration_check_combines_runtime_storage_and_legacy_signals() {
        let store = InMemoryMaintenanceOperationStore::new();
        let service = PlannedMigrationService::new(&store)
            .with_runtime_config_plan(RuntimeConfigMigrationPlan {
                missing_default_keys: vec!["webchat_server.port".to_string()],
            })
            .with_pending_storage_migrations(["001"])
            .with_legacy_data_migration_needed(false);

        let check = service.check_migration().await.expect("check");

        assert!(check.is_needed());
        assert_eq!(check.pending_storage_migrations, vec!["001"]);
    }

    #[tokio::test]
    async fn migration_run_requires_explicit_confirmation() {
        let store = InMemoryMaintenanceOperationStore::new();
        let service = PlannedMigrationService::new(&store);

        let summary = service
            .run_migration(MaintenanceMigrationRequest::default())
            .await
            .expect("summary");

        assert_eq!(summary.progress.status, MaintenanceOperationStatus::Failed);
    }
}
