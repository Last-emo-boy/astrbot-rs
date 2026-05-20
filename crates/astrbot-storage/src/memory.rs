use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use astrbot_memory::{
    LongTermMemoryRepository, MemoryRetentionPolicy, MemorySessionKey, MemoryTranscriptRecord,
};
use async_trait::async_trait;

use crate::SqliteJsonStore;

const LONG_TERM_MEMORY_NAMESPACE: &str = "long_term_memory_sessions";

#[async_trait]
pub trait MemoryRepository: Send + Sync {
    async fn append_memory_record(
        &self,
        record: MemoryTranscriptRecord,
        retention: &MemoryRetentionPolicy,
    ) -> Result<()>;

    async fn memory_records(
        &self,
        session: &MemorySessionKey,
    ) -> Result<Vec<MemoryTranscriptRecord>>;

    async fn clear_memory_session(&self, session: &MemorySessionKey) -> Result<usize>;
}

#[derive(Clone)]
pub struct SqliteLongTermMemoryRepository {
    store: SqliteJsonStore,
}

impl SqliteLongTermMemoryRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl LongTermMemoryRepository for SqliteLongTermMemoryRepository {
    async fn load_session(
        &self,
        session: &MemorySessionKey,
    ) -> Result<Vec<MemoryTranscriptRecord>> {
        self.store
            .get_json(LONG_TERM_MEMORY_NAMESPACE, &session_store_key(session)?)
            .map(|records| records.unwrap_or_default())
    }

    async fn save_session(
        &self,
        session: MemorySessionKey,
        records: Vec<MemoryTranscriptRecord>,
    ) -> Result<()> {
        self.store.put_json(
            LONG_TERM_MEMORY_NAMESPACE,
            &session_store_key(&session)?,
            &records,
        )
    }

    async fn remove_session(&self, session: &MemorySessionKey) -> Result<usize> {
        let key = session_store_key(session)?;
        let count = self
            .store
            .get_json::<Vec<MemoryTranscriptRecord>>(LONG_TERM_MEMORY_NAMESPACE, &key)?
            .map(|records| records.len())
            .unwrap_or_default();
        self.store.delete_json(LONG_TERM_MEMORY_NAMESPACE, &key)?;
        Ok(count)
    }
}

#[derive(Default)]
pub struct InMemoryMemoryRepository {
    records: RwLock<HashMap<MemorySessionKey, Vec<MemoryTranscriptRecord>>>,
}

impl InMemoryMemoryRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MemoryRepository for InMemoryMemoryRepository {
    async fn append_memory_record(
        &self,
        record: MemoryTranscriptRecord,
        retention: &MemoryRetentionPolicy,
    ) -> Result<()> {
        let mut records = self
            .records
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("memory repository lock: {err}")))?;
        let session_records = records.entry(record.session.clone()).or_default();
        session_records.push(record);
        retention.apply(session_records);
        Ok(())
    }

    async fn memory_records(
        &self,
        session: &MemorySessionKey,
    ) -> Result<Vec<MemoryTranscriptRecord>> {
        Ok(self
            .records
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("memory repository lock: {err}")))?
            .get(session)
            .cloned()
            .unwrap_or_default())
    }

    async fn clear_memory_session(&self, session: &MemorySessionKey) -> Result<usize> {
        Ok(self
            .records
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("memory repository lock: {err}")))?
            .remove(session)
            .map(|records| records.len())
            .unwrap_or_default())
    }
}

fn session_store_key(session: &MemorySessionKey) -> Result<String> {
    serde_json::to_string(session)
        .map_err(|err| AstrbotError::Pipeline(format!("serialize memory session key: {err}")))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_memory::{
        LongTermMemoryCompressionPolicy, LongTermMemoryConfig, LongTermMemoryManager,
        LongTermMemoryRepository, MemoryRetentionPolicy, MemorySessionKey, MemoryTranscriptRecord,
    };

    use super::{InMemoryMemoryRepository, MemoryRepository, SqliteLongTermMemoryRepository};
    use crate::SqliteJsonStore;

    #[tokio::test]
    async fn memory_repository_appends_trims_and_clears_session_records() {
        let repository = InMemoryMemoryRepository::new();
        let session = MemorySessionKey::new("webchat", "room-1");
        let retention = MemoryRetentionPolicy::new(2);

        for content in ["old", "middle", "new"] {
            repository
                .append_memory_record(
                    MemoryTranscriptRecord::new(session.clone(), "Alice", content),
                    &retention,
                )
                .await
                .expect("memory record should append");
        }

        let records = repository
            .memory_records(&session)
            .await
            .expect("memory records should load");
        assert_eq!(
            records
                .iter()
                .map(|record| record.content.as_str())
                .collect::<Vec<_>>(),
            vec!["middle", "new"]
        );
        assert_eq!(
            repository
                .clear_memory_session(&session)
                .await
                .expect("memory session should clear"),
            2
        );
    }

    #[tokio::test]
    async fn sqlite_long_term_memory_repository_persists_session_records() {
        let store = SqliteJsonStore::open_in_memory().expect("sqlite store should open");
        let repository = Arc::new(SqliteLongTermMemoryRepository::new(store.clone()));
        let session = MemorySessionKey::new("webchat", "room-1");
        let manager = LongTermMemoryManager::new(repository.clone()).with_config(
            LongTermMemoryConfig::new(3)
                .with_compression(LongTermMemoryCompressionPolicy::disabled()),
        );

        for index in 0..4 {
            manager
                .append_record(MemoryTranscriptRecord::new(
                    session.clone(),
                    "Alice",
                    format!("message {index}"),
                ))
                .await
                .expect("record should append");
        }

        let reloaded_repository = SqliteLongTermMemoryRepository::new(store);
        let records = reloaded_repository
            .load_session(&session)
            .await
            .expect("records should reload");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].content, "message 1");
        assert_eq!(
            reloaded_repository
                .remove_session(&session)
                .await
                .expect("session should remove"),
            3
        );
    }
}
