use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::SqliteJsonStore;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigSnapshotRecord {
    pub snapshot_id: String,
    pub config: Value,
    pub note: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SqliteConfigSnapshotRepository {
    store: SqliteJsonStore,
}

impl SqliteConfigSnapshotRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

impl ConfigSnapshotRecord {
    pub fn new(snapshot_id: impl Into<String>, config: Value) -> Self {
        Self {
            snapshot_id: snapshot_id.into(),
            config,
            note: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

#[async_trait]
pub trait ConfigSnapshotRepository: Send + Sync {
    async fn put_snapshot(&self, record: ConfigSnapshotRecord) -> Result<()>;

    async fn snapshot(&self, snapshot_id: &str) -> Result<Option<ConfigSnapshotRecord>>;

    async fn latest_snapshot(&self) -> Result<Option<ConfigSnapshotRecord>>;
}

#[derive(Default)]
pub struct InMemoryConfigSnapshotRepository {
    order: RwLock<Vec<String>>,
    snapshots: RwLock<HashMap<String, ConfigSnapshotRecord>>,
}

impl InMemoryConfigSnapshotRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ConfigSnapshotRepository for InMemoryConfigSnapshotRepository {
    async fn put_snapshot(&self, record: ConfigSnapshotRecord) -> Result<()> {
        let snapshot_id = record.snapshot_id.clone();
        self.snapshots
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("config snapshot lock: {err}")))?
            .insert(snapshot_id.clone(), record);
        let mut order = self
            .order
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("config snapshot order lock: {err}")))?;
        if !order.contains(&snapshot_id) {
            order.push(snapshot_id);
        }
        Ok(())
    }

    async fn snapshot(&self, snapshot_id: &str) -> Result<Option<ConfigSnapshotRecord>> {
        Ok(self
            .snapshots
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("config snapshot lock: {err}")))?
            .get(snapshot_id)
            .cloned())
    }

    async fn latest_snapshot(&self) -> Result<Option<ConfigSnapshotRecord>> {
        let latest_id = self
            .order
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("config snapshot order lock: {err}")))?
            .last()
            .cloned();
        match latest_id {
            Some(snapshot_id) => self.snapshot(&snapshot_id).await,
            None => Ok(None),
        }
    }
}

#[async_trait]
impl ConfigSnapshotRepository for SqliteConfigSnapshotRepository {
    async fn put_snapshot(&self, record: ConfigSnapshotRecord) -> Result<()> {
        self.store
            .put_json("config_snapshots", &record.snapshot_id, &record)
    }

    async fn snapshot(&self, snapshot_id: &str) -> Result<Option<ConfigSnapshotRecord>> {
        self.store.get_json("config_snapshots", snapshot_id)
    }

    async fn latest_snapshot(&self) -> Result<Option<ConfigSnapshotRecord>> {
        Ok(self
            .store
            .list_json::<ConfigSnapshotRecord>("config_snapshots")?
            .into_iter()
            .last())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigSnapshotRecord, ConfigSnapshotRepository, InMemoryConfigSnapshotRepository,
        SqliteConfigSnapshotRepository,
    };
    use crate::SqliteJsonStore;
    use serde_json::json;

    #[tokio::test]
    async fn latest_snapshot_returns_last_inserted_record() {
        let repository = InMemoryConfigSnapshotRepository::new();
        repository
            .put_snapshot(ConfigSnapshotRecord::new("snap-1", json!({"version": 1})))
            .await
            .expect("snapshot should store");
        repository
            .put_snapshot(ConfigSnapshotRecord::new("snap-2", json!({"version": 2})))
            .await
            .expect("snapshot should store");

        let latest = repository
            .latest_snapshot()
            .await
            .expect("snapshot should load")
            .expect("snapshot should exist");

        assert_eq!(latest.snapshot_id, "snap-2");
        assert_eq!(latest.config, json!({"version": 2}));
    }

    #[tokio::test]
    async fn sqlite_latest_snapshot_returns_last_inserted_record() {
        let repository = SqliteConfigSnapshotRepository::new(
            SqliteJsonStore::open_in_memory().expect("sqlite store should open"),
        );
        repository
            .put_snapshot(ConfigSnapshotRecord::new("snap-1", json!({"version": 1})))
            .await
            .expect("snapshot should store");
        repository
            .put_snapshot(ConfigSnapshotRecord::new("snap-2", json!({"version": 2})))
            .await
            .expect("snapshot should store");

        assert_eq!(
            repository
                .latest_snapshot()
                .await
                .expect("snapshot should load")
                .expect("snapshot should exist")
                .snapshot_id,
            "snap-2"
        );
    }
}
