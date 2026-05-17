use std::hash::{Hash, Hasher};
use std::sync::Arc;

use astrbot_core::{MessageSession, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Serialize, Deserialize)]
pub struct MemorySessionKey {
    pub platform_id: String,
    pub conversation_id: String,
}

impl MemorySessionKey {
    pub fn new(platform_id: impl Into<String>, conversation_id: impl Into<String>) -> Self {
        Self {
            platform_id: platform_id.into(),
            conversation_id: conversation_id.into(),
        }
    }

    pub fn from_session(session: &MessageSession) -> Self {
        Self::new(session.platform_id.clone(), session.conversation_id.clone())
    }

    pub fn origin(&self) -> String {
        format!("{}:{}", self.platform_id, self.conversation_id)
    }
}

impl PartialEq for MemorySessionKey {
    fn eq(&self, other: &Self) -> bool {
        self.platform_id == other.platform_id && self.conversation_id == other.conversation_id
    }
}

impl Hash for MemorySessionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.platform_id.hash(state);
        self.conversation_id.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryTranscriptRecord {
    pub session: MemorySessionKey,
    pub speaker_label: String,
    pub content: String,
    pub timestamp: Option<String>,
}

impl MemoryTranscriptRecord {
    pub fn new(
        session: MemorySessionKey,
        speaker_label: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            session,
            speaker_label: speaker_label.into(),
            content: content.into(),
            timestamp: None,
        }
    }

    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = non_empty_string(timestamp);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryRetentionPolicy {
    pub max_records: usize,
}

impl MemoryRetentionPolicy {
    pub fn new(max_records: usize) -> Self {
        Self {
            max_records: max_records.max(1),
        }
    }

    pub fn apply(&self, records: &mut Vec<MemoryTranscriptRecord>) {
        let overflow = records.len().saturating_sub(self.max_records);
        if overflow > 0 {
            records.drain(0..overflow);
        }
    }
}

impl Default for MemoryRetentionPolicy {
    fn default() -> Self {
        Self::new(300)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryImageCaptionConfig {
    pub enabled: bool,
    pub prompt: String,
    pub provider_id: Option<String>,
}

impl MemoryImageCaptionConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            prompt: "Describe the image content for the chat model.".to_string(),
            provider_id: None,
        }
    }

    pub fn enabled(prompt: impl Into<String>) -> Self {
        Self {
            enabled: true,
            prompt: prompt.into(),
            provider_id: None,
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_string(provider_id);
        self
    }
}

impl Default for MemoryImageCaptionConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryImageCaptionRequest {
    pub prompt: String,
    pub provider_id: Option<String>,
    pub session_id: String,
    pub image_url: String,
}

#[async_trait]
pub trait MemoryImageCaptioner: Send + Sync {
    async fn caption_image(&self, request: MemoryImageCaptionRequest) -> Result<Option<String>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryMessageInput {
    pub session: MemorySessionKey,
    pub speaker_label: String,
    pub text_parts: Vec<String>,
    pub image_urls: Vec<String>,
    pub mentions: Vec<String>,
    pub timestamp: Option<String>,
}

impl MemoryMessageInput {
    pub fn new(session: MemorySessionKey, speaker_label: impl Into<String>) -> Self {
        Self {
            session,
            speaker_label: speaker_label.into(),
            text_parts: Vec::new(),
            image_urls: Vec::new(),
            mentions: Vec::new(),
            timestamp: None,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        if let Some(text) = non_empty_string(text) {
            self.text_parts.push(text);
        }
        self
    }

    pub fn with_image_url(mut self, image_url: impl Into<String>) -> Self {
        if let Some(image_url) = non_empty_string(image_url) {
            self.image_urls.push(image_url);
        }
        self
    }

    pub fn with_mention(mut self, mention: impl Into<String>) -> Self {
        if let Some(mention) = non_empty_string(mention) {
            self.mentions.push(mention);
        }
        self
    }

    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = non_empty_string(timestamp);
        self
    }
}

pub struct MemoryTranscriptBuilder {
    caption_config: MemoryImageCaptionConfig,
    captioner: Option<Arc<dyn MemoryImageCaptioner>>,
}

impl MemoryTranscriptBuilder {
    pub fn new() -> Self {
        Self {
            caption_config: MemoryImageCaptionConfig::default(),
            captioner: None,
        }
    }

    pub fn with_captioner(
        mut self,
        captioner: Arc<dyn MemoryImageCaptioner>,
        caption_config: MemoryImageCaptionConfig,
    ) -> Self {
        self.captioner = Some(captioner);
        self.caption_config = caption_config;
        self
    }

    pub async fn build(&self, input: MemoryMessageInput) -> Result<Option<MemoryTranscriptRecord>> {
        let mut parts = input
            .text_parts
            .iter()
            .filter_map(|part| non_empty_string_ref(part))
            .collect::<Vec<_>>();

        for image_url in input
            .image_urls
            .iter()
            .filter_map(|image_url| non_empty_string_ref(image_url))
        {
            parts.push(self.render_image(&input.session, &image_url).await?);
        }
        for mention in input
            .mentions
            .iter()
            .filter_map(|mention| non_empty_string_ref(mention))
        {
            parts.push(format!("[At: {mention}]"));
        }

        if parts.is_empty() {
            return Ok(None);
        }

        let prefix = if let Some(timestamp) = input.timestamp.as_deref() {
            format!("[{}/{}]:", input.speaker_label, timestamp)
        } else {
            format!("[{}]:", input.speaker_label)
        };
        let content = format!("{prefix} {}", parts.join(" "));

        let mut record = MemoryTranscriptRecord::new(input.session, input.speaker_label, content);
        record.timestamp = input.timestamp;
        Ok(Some(record))
    }

    async fn render_image(&self, session: &MemorySessionKey, image_url: &str) -> Result<String> {
        if !self.caption_config.enabled {
            return Ok("[Image]".to_string());
        }
        let Some(captioner) = &self.captioner else {
            return Ok("[Image]".to_string());
        };
        let caption = captioner
            .caption_image(MemoryImageCaptionRequest {
                prompt: self.caption_config.prompt.clone(),
                provider_id: self.caption_config.provider_id.clone(),
                session_id: session.conversation_id.clone(),
                image_url: image_url.to_string(),
            })
            .await?
            .map(|caption| caption.trim().to_string())
            .filter(|caption| !caption.is_empty());

        Ok(caption
            .map(|caption| format!("[Image: {caption}]"))
            .unwrap_or_else(|| "[Image]".to_string()))
    }
}

impl Default for MemoryTranscriptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn non_empty_string_ref(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{
        MemoryImageCaptionConfig, MemoryImageCaptionRequest, MemoryImageCaptioner,
        MemoryMessageInput, MemoryRetentionPolicy, MemorySessionKey, MemoryTranscriptBuilder,
        MemoryTranscriptRecord,
    };

    struct StaticCaptioner;

    #[async_trait]
    impl MemoryImageCaptioner for StaticCaptioner {
        async fn caption_image(
            &self,
            request: MemoryImageCaptionRequest,
        ) -> astrbot_core::Result<Option<String>> {
            assert_eq!(request.provider_id.as_deref(), Some("vision"));
            assert_eq!(request.image_url, "image.png");
            Ok(Some("a chart".to_string()))
        }
    }

    #[tokio::test]
    async fn transcript_builder_uses_caption_port_without_provider_manager() {
        let builder = MemoryTranscriptBuilder::new().with_captioner(
            Arc::new(StaticCaptioner),
            MemoryImageCaptionConfig::enabled("caption").with_provider_id("vision"),
        );
        let input = MemoryMessageInput::new(MemorySessionKey::new("webchat", "room-1"), "Alice")
            .with_timestamp("12:00:00")
            .with_text("hello")
            .with_image_url("image.png")
            .with_mention("Bob");

        let record = builder
            .build(input)
            .await
            .expect("transcript should build")
            .expect("record should exist");

        assert_eq!(
            record.content,
            "[Alice/12:00:00]: hello [Image: a chart] [At: Bob]"
        );
    }

    #[test]
    fn retention_policy_keeps_newest_records() {
        let session = MemorySessionKey::new("webchat", "room-1");
        let mut records = vec![
            MemoryTranscriptRecord::new(session.clone(), "u", "old"),
            MemoryTranscriptRecord::new(session.clone(), "u", "middle"),
            MemoryTranscriptRecord::new(session, "u", "new"),
        ];

        MemoryRetentionPolicy::new(2).apply(&mut records);

        assert_eq!(records[0].content, "middle");
        assert_eq!(records[1].content, "new");
    }
}
