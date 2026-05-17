use std::collections::HashSet;
use std::sync::Arc;

use astrbot_core::{MessageEvent, Result};
use astrbot_provider::{SpeechToTextProvider, SpeechToTextRequest};
use async_trait::async_trait;

#[derive(Clone)]
pub struct PreprocessConfig {
    pub pre_ack: PreAckConfig,
    pub path_mapping_enabled: bool,
    pub speech_to_text: SpeechToTextPreprocessConfig,
    pre_ack_sink: Arc<dyn PreAckReactionSink>,
    path_mapper: Arc<dyn PreprocessPathMapper>,
    speech_to_text_provider: Option<Arc<dyn SpeechToTextProvider>>,
}

impl Default for PreprocessConfig {
    fn default() -> Self {
        Self {
            pre_ack: PreAckConfig::default(),
            path_mapping_enabled: false,
            speech_to_text: SpeechToTextPreprocessConfig::default(),
            pre_ack_sink: Arc::new(NoopPreAckReactionSink),
            path_mapper: Arc::new(NoopPreprocessPathMapper),
            speech_to_text_provider: None,
        }
    }
}

impl PreprocessConfig {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn with_pre_ack(mut self, pre_ack: PreAckConfig) -> Self {
        self.pre_ack = pre_ack;
        self
    }

    pub fn with_pre_ack_sink(mut self, sink: Arc<dyn PreAckReactionSink>) -> Self {
        self.pre_ack_sink = sink;
        self
    }

    pub fn with_path_mapper(mut self, mapper: Arc<dyn PreprocessPathMapper>) -> Self {
        self.path_mapping_enabled = true;
        self.path_mapper = mapper;
        self
    }

    pub fn with_speech_to_text_provider(mut self, provider: Arc<dyn SpeechToTextProvider>) -> Self {
        self.speech_to_text.enabled = true;
        self.speech_to_text_provider = Some(provider);
        self
    }

    pub fn with_speech_to_text(mut self, speech_to_text: SpeechToTextPreprocessConfig) -> Self {
        self.speech_to_text = speech_to_text;
        self
    }

    pub fn pre_ack_sink(&self) -> Arc<dyn PreAckReactionSink> {
        self.pre_ack_sink.clone()
    }

    pub fn path_mapper(&self) -> Arc<dyn PreprocessPathMapper> {
        self.path_mapper.clone()
    }

    pub fn speech_to_text_provider(&self) -> Option<Arc<dyn SpeechToTextProvider>> {
        self.speech_to_text_provider.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreAckConfig {
    pub enabled: bool,
    supported_platforms: HashSet<String>,
    reactions: Vec<String>,
}

impl Default for PreAckConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            supported_platforms: HashSet::from([
                "telegram".to_string(),
                "lark".to_string(),
                "discord".to_string(),
            ]),
            reactions: Vec::new(),
        }
    }
}

impl PreAckConfig {
    pub fn enabled<I, S>(reactions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            enabled: true,
            reactions: normalize_non_empty(reactions),
            ..Self::default()
        }
    }

    pub fn with_supported_platforms<I, S>(mut self, platforms: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.supported_platforms = normalize_non_empty(platforms).into_iter().collect();
        self
    }

    pub fn reactions(&self) -> &[String] {
        &self.reactions
    }

    pub fn first_reaction(&self) -> Option<&str> {
        self.reactions.first().map(String::as_str)
    }

    pub fn supports_platform(&self, platform_id: &str, platform_name: &str) -> bool {
        self.supported_platforms.contains(platform_id)
            || self
                .supported_platforms
                .contains(&platform_name.to_ascii_lowercase())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpeechToTextPreprocessConfig {
    pub enabled: bool,
    pub provider_id: Option<String>,
    pub retry_attempts: usize,
}

impl SpeechToTextPreprocessConfig {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            retry_attempts: 1,
            ..Self::default()
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        let provider_id = provider_id.into();
        self.provider_id = (!provider_id.trim().is_empty()).then_some(provider_id);
        self
    }

    pub fn with_retry_attempts(mut self, retry_attempts: usize) -> Self {
        self.retry_attempts = retry_attempts.max(1);
        self
    }

    pub fn request_for(&self, audio_url: impl Into<String>) -> SpeechToTextRequest {
        let request = SpeechToTextRequest::new(audio_url);
        match self.provider_id.as_deref() {
            Some(provider_id) => request.with_provider_id(provider_id),
            None => request,
        }
    }
}

#[async_trait]
pub trait PreAckReactionSink: Send + Sync {
    async fn react(&self, event: &MessageEvent, reaction: &str) -> Result<()>;
}

pub struct NoopPreAckReactionSink;

#[async_trait]
impl PreAckReactionSink for NoopPreAckReactionSink {
    async fn react(&self, _event: &MessageEvent, _reaction: &str) -> Result<()> {
        Ok(())
    }
}

pub trait PreprocessPathMapper: Send + Sync {
    fn map_path(&self, path: &str) -> Option<String>;
}

pub struct NoopPreprocessPathMapper;

impl PreprocessPathMapper for NoopPreprocessPathMapper {
    fn map_path(&self, _path: &str) -> Option<String> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefixPathMapper {
    mappings: Vec<PrefixPathMapping>,
}

impl PrefixPathMapper {
    pub fn new<I>(mappings: I) -> Self
    where
        I: IntoIterator<Item = PrefixPathMapping>,
    {
        Self {
            mappings: mappings.into_iter().collect(),
        }
    }

    pub fn mappings(&self) -> &[PrefixPathMapping] {
        &self.mappings
    }
}

impl PreprocessPathMapper for PrefixPathMapper {
    fn map_path(&self, path: &str) -> Option<String> {
        let normalized = strip_file_scheme(path);
        self.mappings
            .iter()
            .find_map(|mapping| mapping.apply(normalized))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefixPathMapping {
    from: String,
    to: String,
}

impl PrefixPathMapping {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: normalize_prefix(from),
            to: normalize_prefix(to),
        }
    }

    pub fn parse(mapping: &str) -> Option<Self> {
        let (from, to) = mapping.split_once(':')?;
        let from = from.trim();
        let to = to.trim();
        (!from.is_empty()).then(|| Self::new(from, to))
    }

    pub fn apply(&self, path: &str) -> Option<String> {
        path.starts_with(&self.from)
            .then(|| format!("{}{}", self.to, &path[self.from.len()..]))
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn to(&self) -> &str {
        &self.to
    }
}

fn normalize_prefix(value: impl Into<String>) -> String {
    value
        .into()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

fn normalize_non_empty<I, S>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    items
        .into_iter()
        .map(Into::into)
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

pub fn strip_file_scheme(path: &str) -> &str {
    path.strip_prefix("file://").unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::{PrefixPathMapper, PrefixPathMapping, PreprocessPathMapper};

    #[test]
    fn prefix_path_mapping_strips_file_scheme_and_rewrites_prefix() {
        let mapper = PrefixPathMapper::new([PrefixPathMapping::new("/host/media", "/mnt/media")]);

        assert_eq!(
            mapper.map_path("file:///host/media/a.ogg").as_deref(),
            Some("/mnt/media/a.ogg")
        );
        assert_eq!(mapper.map_path("/other/a.ogg"), None);
    }

    #[test]
    fn prefix_path_mapping_parses_colon_mapping() {
        let mapping = PrefixPathMapping::parse("/host:/container").expect("mapping should parse");

        assert_eq!(mapping.from(), "/host");
        assert_eq!(mapping.to(), "/container");
        assert_eq!(
            mapping.apply("/host/a.png").as_deref(),
            Some("/container/a.png")
        );
    }
}
