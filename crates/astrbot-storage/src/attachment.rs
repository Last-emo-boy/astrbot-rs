use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentRecord {
    pub attachment_id: String,
    pub source_url: String,
    pub stored_url: Option<String>,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

impl AttachmentRecord {
    pub fn new(attachment_id: impl Into<String>, source_url: impl Into<String>) -> Self {
        Self {
            attachment_id: attachment_id.into(),
            source_url: source_url.into(),
            stored_url: None,
            filename: None,
            content_type: None,
        }
    }

    pub fn with_stored_url(mut self, stored_url: impl Into<String>) -> Self {
        self.stored_url = Some(stored_url.into());
        self
    }
}

#[async_trait]
pub trait AttachmentRepository: Send + Sync {
    async fn put_attachment(&self, record: AttachmentRecord) -> Result<()>;

    async fn attachment(&self, attachment_id: &str) -> Result<Option<AttachmentRecord>>;
}

#[derive(Default)]
pub struct InMemoryAttachmentRepository {
    attachments: RwLock<HashMap<String, AttachmentRecord>>,
}

impl InMemoryAttachmentRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AttachmentRepository for InMemoryAttachmentRepository {
    async fn put_attachment(&self, record: AttachmentRecord) -> Result<()> {
        self.attachments
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("attachment repository lock: {err}")))?
            .insert(record.attachment_id.clone(), record);
        Ok(())
    }

    async fn attachment(&self, attachment_id: &str) -> Result<Option<AttachmentRecord>> {
        Ok(self
            .attachments
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("attachment repository lock: {err}")))?
            .get(attachment_id)
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::{AttachmentRecord, AttachmentRepository, InMemoryAttachmentRepository};

    #[tokio::test]
    async fn in_memory_attachment_repository_round_trips_metadata() {
        let repository = InMemoryAttachmentRepository::new();
        repository
            .put_attachment(
                AttachmentRecord::new("att-1", "https://example.test/source.png")
                    .with_stored_url("attachments/att-1.png"),
            )
            .await
            .expect("attachment should store");

        let attachment = repository
            .attachment("att-1")
            .await
            .expect("attachment should load")
            .expect("attachment should exist");

        assert_eq!(attachment.source_url, "https://example.test/source.png");
        assert_eq!(
            attachment.stored_url.as_deref(),
            Some("attachments/att-1.png")
        );
    }
}
