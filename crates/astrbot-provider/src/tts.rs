use astrbot_core::Result;
use async_trait::async_trait;

use crate::chat::non_empty_option;

pub mod audio_queue;
pub mod stream;

pub use audio_queue::{TextToSpeechAudioChunk, TextToSpeechAudioQueueItem};
pub use stream::{
    FileSynthesisTextToSpeechStreamProvider, QueuedTextToSpeechAudioStream,
    TextToSpeechAudioStream, TextToSpeechStreamProvider, TextToSpeechStreamRequest,
    TextToSpeechStreamText,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextToSpeechRequest {
    pub provider_id: Option<String>,
    pub text: String,
}

impl TextToSpeechRequest {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            provider_id: None,
            text: text.into(),
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextToSpeechResponse {
    pub audio_path: String,
}

impl TextToSpeechResponse {
    pub fn new(audio_path: impl Into<String>) -> Self {
        Self {
            audio_path: audio_path.into(),
        }
    }
}

#[async_trait]
pub trait TextToSpeechProvider: Send + Sync {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse>;

    fn supports_streaming(&self) -> bool {
        false
    }

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MockTextToSpeechProvider {
    audio_path: String,
    supports_streaming: bool,
}

impl MockTextToSpeechProvider {
    pub fn new(audio_path: impl Into<String>) -> Self {
        Self {
            audio_path: audio_path.into(),
            supports_streaming: false,
        }
    }

    pub fn with_streaming(mut self, supports_streaming: bool) -> Self {
        self.supports_streaming = supports_streaming;
        self
    }
}

#[async_trait]
impl TextToSpeechProvider for MockTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        if request.text.trim().is_empty() {
            return Err(astrbot_core::AstrbotError::Provider(
                "text-to-speech request must contain text".to_string(),
            ));
        }

        Ok(TextToSpeechResponse::new(self.audio_path.clone()))
    }

    fn supports_streaming(&self) -> bool {
        self.supports_streaming
    }
}
