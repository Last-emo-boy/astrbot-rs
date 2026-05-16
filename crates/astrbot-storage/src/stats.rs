use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformStatsRecord {
    pub timestamp: String,
    pub platform_id: String,
    pub platform_type: String,
    pub count: i64,
}

impl PlatformStatsRecord {
    pub fn new(
        timestamp: impl Into<String>,
        platform_id: impl Into<String>,
        platform_type: impl Into<String>,
        count: i64,
    ) -> Self {
        Self {
            timestamp: timestamp.into(),
            platform_id: platform_id.into(),
            platform_type: platform_type.into(),
            count,
        }
    }
}

#[async_trait]
pub trait PlatformStatsRepository: Send + Sync {
    async fn increment_platform_stats(&self, record: PlatformStatsRecord) -> Result<()>;

    async fn platform_stats_since(&self, timestamp: &str) -> Result<Vec<PlatformStatsRecord>>;

    async fn total_message_count(&self) -> Result<i64>;
}

#[derive(Default)]
pub struct InMemoryPlatformStatsRepository {
    stats: RwLock<HashMap<(String, String, String), PlatformStatsRecord>>,
}

impl InMemoryPlatformStatsRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PlatformStatsRepository for InMemoryPlatformStatsRepository {
    async fn increment_platform_stats(&self, record: PlatformStatsRecord) -> Result<()> {
        let key = (
            record.timestamp.clone(),
            record.platform_id.clone(),
            record.platform_type.clone(),
        );
        let mut stats = self
            .stats
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("platform stats lock: {err}")))?;
        stats
            .entry(key)
            .and_modify(|current| current.count += record.count)
            .or_insert(record);
        Ok(())
    }

    async fn platform_stats_since(&self, timestamp: &str) -> Result<Vec<PlatformStatsRecord>> {
        let mut records = self
            .stats
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("platform stats lock: {err}")))?
            .values()
            .filter(|record| record.timestamp.as_str() >= timestamp)
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.timestamp
                .cmp(&right.timestamp)
                .then_with(|| left.platform_id.cmp(&right.platform_id))
                .then_with(|| left.platform_type.cmp(&right.platform_type))
        });
        Ok(records)
    }

    async fn total_message_count(&self) -> Result<i64> {
        Ok(self
            .stats
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("platform stats lock: {err}")))?
            .values()
            .map(|record| record.count)
            .sum())
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemoryPlatformStatsRepository, PlatformStatsRecord, PlatformStatsRepository};

    #[tokio::test]
    async fn in_memory_stats_merges_duplicate_platform_hour_keys() {
        let repository = InMemoryPlatformStatsRepository::new();
        repository
            .increment_platform_stats(PlatformStatsRecord::new(
                "2026-05-16T08:00:00Z",
                "webchat",
                "webchat",
                2,
            ))
            .await
            .expect("stats should store");
        repository
            .increment_platform_stats(PlatformStatsRecord::new(
                "2026-05-16T08:00:00Z",
                "webchat",
                "webchat",
                3,
            ))
            .await
            .expect("stats should store");

        let stats = repository
            .platform_stats_since("2026-05-16T00:00:00Z")
            .await
            .expect("stats should load");

        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].count, 5);
        assert_eq!(
            repository
                .total_message_count()
                .await
                .expect("count should load"),
            5
        );
    }
}
