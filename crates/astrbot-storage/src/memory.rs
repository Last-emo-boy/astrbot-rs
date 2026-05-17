use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use astrbot_memory::{MemoryRetentionPolicy, MemorySessionKey, MemoryTranscriptRecord};
use async_trait::async_trait;

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

#[cfg(test)]
mod tests {
    use astrbot_memory::{MemoryRetentionPolicy, MemorySessionKey, MemoryTranscriptRecord};

    use super::{InMemoryMemoryRepository, MemoryRepository};

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
}
