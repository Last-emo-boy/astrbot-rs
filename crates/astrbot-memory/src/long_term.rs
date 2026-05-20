use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use crate::{
    MemoryImageCaptionConfig, MemoryImageCaptioner, MemoryMessageInput, MemorySessionKey,
    MemoryTranscriptBuilder, MemoryTranscriptRecord,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LongTermMemoryConfig {
    pub max_records: usize,
    pub compression: LongTermMemoryCompressionPolicy,
}

impl LongTermMemoryConfig {
    pub fn new(max_records: usize) -> Self {
        Self {
            max_records: max_records.max(1),
            compression: LongTermMemoryCompressionPolicy::default(),
        }
    }

    pub fn with_compression(mut self, compression: LongTermMemoryCompressionPolicy) -> Self {
        self.compression = compression;
        self
    }
}

impl Default for LongTermMemoryConfig {
    fn default() -> Self {
        Self::new(300)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LongTermMemoryCompressionPolicy {
    pub enabled: bool,
    pub max_estimated_tokens: usize,
    pub summary_speaker_label: String,
}

impl LongTermMemoryCompressionPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_estimated_tokens: usize::MAX,
            summary_speaker_label: "Memory Summary".to_string(),
        }
    }

    pub fn enabled(max_estimated_tokens: usize) -> Self {
        Self {
            enabled: true,
            max_estimated_tokens: max_estimated_tokens.max(1),
            summary_speaker_label: "Memory Summary".to_string(),
        }
    }

    pub fn with_summary_speaker_label(mut self, speaker_label: impl Into<String>) -> Self {
        let speaker_label = speaker_label.into();
        if !speaker_label.trim().is_empty() {
            self.summary_speaker_label = speaker_label.trim().to_string();
        }
        self
    }
}

impl Default for LongTermMemoryCompressionPolicy {
    fn default() -> Self {
        Self::enabled(4_000)
    }
}

#[async_trait]
pub trait LongTermMemoryRepository: Send + Sync {
    async fn load_session(&self, session: &MemorySessionKey)
    -> Result<Vec<MemoryTranscriptRecord>>;

    async fn save_session(
        &self,
        session: MemorySessionKey,
        records: Vec<MemoryTranscriptRecord>,
    ) -> Result<()>;

    async fn remove_session(&self, session: &MemorySessionKey) -> Result<usize>;
}

#[derive(Default)]
pub struct InMemoryLongTermMemoryRepository {
    records: Mutex<HashMap<MemorySessionKey, Vec<MemoryTranscriptRecord>>>,
}

impl InMemoryLongTermMemoryRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_records(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<MemorySessionKey, Vec<MemoryTranscriptRecord>>>>
    {
        self.records
            .lock()
            .map_err(|_| AstrbotError::Pipeline("long-term memory store lock poisoned".to_string()))
    }
}

#[async_trait]
impl LongTermMemoryRepository for InMemoryLongTermMemoryRepository {
    async fn load_session(
        &self,
        session: &MemorySessionKey,
    ) -> Result<Vec<MemoryTranscriptRecord>> {
        let records = self.lock_records()?;
        Ok(records.get(session).cloned().unwrap_or_default())
    }

    async fn save_session(
        &self,
        session: MemorySessionKey,
        records: Vec<MemoryTranscriptRecord>,
    ) -> Result<()> {
        let mut store = self.lock_records()?;
        store.insert(session, records);
        Ok(())
    }

    async fn remove_session(&self, session: &MemorySessionKey) -> Result<usize> {
        let mut store = self.lock_records()?;
        Ok(store
            .remove(session)
            .map(|records| records.len())
            .unwrap_or(0))
    }
}

pub struct LongTermMemoryManager {
    repository: Arc<dyn LongTermMemoryRepository>,
    builder: MemoryTranscriptBuilder,
    config: LongTermMemoryConfig,
}

impl LongTermMemoryManager {
    pub fn new(repository: Arc<dyn LongTermMemoryRepository>) -> Self {
        Self {
            repository,
            builder: MemoryTranscriptBuilder::new(),
            config: LongTermMemoryConfig::default(),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryLongTermMemoryRepository::new()))
    }

    pub fn with_config(mut self, config: LongTermMemoryConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_captioner(
        mut self,
        captioner: Arc<dyn MemoryImageCaptioner>,
        caption_config: MemoryImageCaptionConfig,
    ) -> Self {
        self.builder = MemoryTranscriptBuilder::new().with_captioner(captioner, caption_config);
        self
    }

    pub async fn record_message(
        &self,
        input: MemoryMessageInput,
    ) -> Result<Option<MemoryTranscriptRecord>> {
        let Some(record) = self.builder.build(input).await? else {
            return Ok(None);
        };
        self.append_record(record.clone()).await?;
        Ok(Some(record))
    }

    pub async fn append_record(&self, record: MemoryTranscriptRecord) -> Result<()> {
        let session = record.session.clone();
        let mut records = self.repository.load_session(&session).await?;
        records.push(record);
        self.apply_budget(&mut records, &session);
        self.repository.save_session(session, records).await
    }

    pub async fn records(&self, session: &MemorySessionKey) -> Result<Vec<MemoryTranscriptRecord>> {
        self.repository.load_session(session).await
    }

    pub async fn remove_session(&self, session: &MemorySessionKey) -> Result<usize> {
        self.repository.remove_session(session).await
    }

    fn apply_budget(&self, records: &mut Vec<MemoryTranscriptRecord>, session: &MemorySessionKey) {
        let over_record_budget = records.len() > self.config.max_records;
        let over_token_budget =
            estimated_tokens(records) > self.config.compression.max_estimated_tokens;
        if !self.config.compression.enabled || (!over_record_budget && !over_token_budget) {
            trim_to_recent(records, self.config.max_records);
            return;
        }

        let keep_recent = self.config.max_records.saturating_sub(1).max(1);
        let split_at = records.len().saturating_sub(keep_recent);
        let compressed = records.drain(0..split_at).collect::<Vec<_>>();
        let summary = build_summary_record(
            session.clone(),
            &self.config.compression.summary_speaker_label,
            &compressed,
        );
        records.insert(0, summary);
        trim_to_recent(records, self.config.max_records);
    }
}

fn trim_to_recent(records: &mut Vec<MemoryTranscriptRecord>, max_records: usize) {
    let overflow = records.len().saturating_sub(max_records.max(1));
    if overflow > 0 {
        records.drain(0..overflow);
    }
}

fn estimated_tokens(records: &[MemoryTranscriptRecord]) -> usize {
    records
        .iter()
        .map(|record| (record.content.chars().count() / 4).max(1))
        .sum()
}

fn build_summary_record(
    session: MemorySessionKey,
    speaker_label: &str,
    records: &[MemoryTranscriptRecord],
) -> MemoryTranscriptRecord {
    let compressed_count = records.iter().map(compressed_record_count).sum::<usize>();
    let facts = records
        .iter()
        .flat_map(extract_key_facts)
        .take(8)
        .collect::<Vec<_>>();
    let detail = if facts.is_empty() {
        "Earlier conversation compressed.".to_string()
    } else {
        format!("Key facts: {}", facts.join(" | "))
    };
    MemoryTranscriptRecord::new(
        session,
        speaker_label,
        format!(
            "[{}]: Summary of {} earlier records. {}",
            speaker_label, compressed_count, detail
        ),
    )
}

fn compressed_record_count(record: &MemoryTranscriptRecord) -> usize {
    let marker = "Summary of ";
    let Some(start) = record.content.find(marker) else {
        return 1;
    };
    let after_marker = &record.content[start + marker.len()..];
    let Some(end) = after_marker.find(" earlier records") else {
        return 1;
    };
    after_marker[..end].parse::<usize>().unwrap_or(1)
}

fn extract_key_facts(record: &MemoryTranscriptRecord) -> Vec<String> {
    let lower = record.content.to_lowercase();
    let has_marker = lower.contains("remember")
        || lower.contains("key fact")
        || lower.contains("fact:")
        || record.content.contains("记住")
        || record.content.contains("重要");
    if !has_marker {
        return Vec::new();
    }
    vec![record.content.trim().to_string()]
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{
        InMemoryLongTermMemoryRepository, LongTermMemoryCompressionPolicy, LongTermMemoryConfig,
        LongTermMemoryManager,
    };
    use crate::{
        MemoryImageCaptionConfig, MemoryImageCaptionRequest, MemoryImageCaptioner,
        MemoryMessageInput, MemorySessionKey,
    };

    struct StaticCaptioner;

    #[async_trait]
    impl MemoryImageCaptioner for StaticCaptioner {
        async fn caption_image(
            &self,
            request: MemoryImageCaptionRequest,
        ) -> astrbot_core::Result<Option<String>> {
            assert_eq!(request.image_url, "image.png");
            Ok(Some("whiteboard with release checklist".to_string()))
        }
    }

    #[tokio::test]
    async fn long_term_memory_compresses_after_100_records_and_keeps_key_facts() {
        let session = MemorySessionKey::new("webchat", "room-1");
        let manager = LongTermMemoryManager::new(Arc::new(InMemoryLongTermMemoryRepository::new()))
            .with_config(
                LongTermMemoryConfig::new(20)
                    .with_compression(LongTermMemoryCompressionPolicy::enabled(1_000)),
            );

        for index in 0..100 {
            let text = if index == 3 {
                "remember key fact: deploy window is Friday".to_string()
            } else {
                format!("message {index}")
            };
            manager
                .record_message(MemoryMessageInput::new(session.clone(), "Alice").with_text(text))
                .await
                .expect("message should record");
        }

        let records = manager
            .records(&session)
            .await
            .expect("records should load");
        assert_eq!(records.len(), 20);
        assert!(records[0].content.contains("Summary of 81 earlier records"));
        assert!(records[0].content.contains("deploy window is Friday"));
        assert!(
            records
                .last()
                .expect("last record")
                .content
                .contains("message 99")
        );
    }

    #[tokio::test]
    async fn long_term_memory_records_image_caption() {
        let session = MemorySessionKey::new("webchat", "room-1");
        let manager = LongTermMemoryManager::in_memory().with_captioner(
            Arc::new(StaticCaptioner),
            MemoryImageCaptionConfig::enabled("caption"),
        );

        manager
            .record_message(
                MemoryMessageInput::new(session.clone(), "Alice")
                    .with_text("see this")
                    .with_image_url("image.png"),
            )
            .await
            .expect("message should record");

        let records = manager
            .records(&session)
            .await
            .expect("records should load");
        assert_eq!(records.len(), 1);
        assert!(
            records[0]
                .content
                .contains("[Image: whiteboard with release checklist]")
        );
    }
}
