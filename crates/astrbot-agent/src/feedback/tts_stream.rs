use std::sync::Arc;

use astrbot_core::{MessageChain, Result};
use astrbot_provider::{
    TextToSpeechAudioChunk, TextToSpeechAudioStream, TextToSpeechStreamProvider,
    TextToSpeechStreamRequest,
};

use super::voice::LiveVoiceFeedbackConfig;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveTtsStreamFeedbackChunk {
    pub source_text: Option<String>,
    pub audio: Vec<u8>,
    pub mime_type: Option<String>,
    pub terminal: bool,
}

impl LiveTtsStreamFeedbackChunk {
    pub fn terminal() -> Self {
        Self {
            source_text: None,
            audio: Vec::new(),
            mime_type: None,
            terminal: true,
        }
    }
}

impl From<TextToSpeechAudioChunk> for LiveTtsStreamFeedbackChunk {
    fn from(chunk: TextToSpeechAudioChunk) -> Self {
        Self {
            source_text: chunk.source_text,
            audio: chunk.audio,
            mime_type: chunk.mime_type,
            terminal: false,
        }
    }
}

pub struct LiveTtsStreamFeedbackBridge {
    config: LiveVoiceFeedbackConfig,
    provider: Option<Arc<dyn TextToSpeechStreamProvider>>,
}

impl LiveTtsStreamFeedbackBridge {
    pub fn disabled() -> Self {
        Self {
            config: LiveVoiceFeedbackConfig::disabled(),
            provider: None,
        }
    }

    pub fn new(
        provider: Arc<dyn TextToSpeechStreamProvider>,
        config: LiveVoiceFeedbackConfig,
    ) -> Self {
        Self {
            config,
            provider: Some(provider),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled() && self.config.prefer_streaming && self.provider.is_some()
    }

    pub async fn stream_chain(
        &self,
        chain: &MessageChain,
    ) -> Result<Option<Box<dyn TextToSpeechAudioStream>>> {
        if !self.is_enabled() {
            return Ok(None);
        }

        let text = chain.plain_text();
        let text = text.trim();
        if text.chars().count() < self.config.min_text_chars {
            return Ok(None);
        }

        let provider = self.provider.as_ref().expect("provider checked above");
        let mut request = TextToSpeechStreamRequest::single(text);
        if let Some(provider_id) = &self.config.provider_id {
            request = request.with_provider_id(provider_id.clone());
        }

        provider.stream_audio(request).await.map(Some)
    }

    pub async fn collect_chain(
        &self,
        chain: &MessageChain,
    ) -> Result<Vec<LiveTtsStreamFeedbackChunk>> {
        let Some(mut stream) = self.stream_chain(chain).await? else {
            return Ok(Vec::new());
        };

        let mut chunks = Vec::new();
        while let Some(chunk) = stream.next_audio().await? {
            chunks.push(LiveTtsStreamFeedbackChunk::from(chunk));
        }
        chunks.push(LiveTtsStreamFeedbackChunk::terminal());
        Ok(chunks)
    }
}

impl Default for LiveTtsStreamFeedbackBridge {
    fn default() -> Self {
        Self::disabled()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use astrbot_core::{MessageChain, Result};
    use astrbot_provider::{
        QueuedTextToSpeechAudioStream, TextToSpeechAudioQueueItem, TextToSpeechAudioStream,
        TextToSpeechStreamProvider, TextToSpeechStreamRequest,
    };
    use async_trait::async_trait;

    use super::LiveTtsStreamFeedbackBridge;
    use crate::LiveVoiceFeedbackConfig;

    #[derive(Default)]
    struct RecordingStreamProvider {
        requests: Mutex<Vec<TextToSpeechStreamRequest>>,
    }

    #[async_trait]
    impl TextToSpeechStreamProvider for RecordingStreamProvider {
        async fn stream_audio(
            &self,
            request: TextToSpeechStreamRequest,
        ) -> Result<Box<dyn TextToSpeechAudioStream>> {
            self.requests
                .lock()
                .expect("tts stream requests lock")
                .push(request);
            Ok(Box::new(QueuedTextToSpeechAudioStream::new([
                TextToSpeechAudioQueueItem::text_chunk("hello", [1, 2, 3]),
                TextToSpeechAudioQueueItem::terminal(),
            ])))
        }
    }

    #[tokio::test]
    async fn stream_feedback_is_disabled_without_provider_or_streaming_preference() {
        let bridge = LiveTtsStreamFeedbackBridge::disabled();

        let chunks = bridge
            .collect_chain(&MessageChain::plain("hello"))
            .await
            .expect("disabled bridge should not fail");

        assert!(chunks.is_empty());
    }

    #[tokio::test]
    async fn stream_feedback_consumes_provider_neutral_audio_stream() {
        let provider = Arc::new(RecordingStreamProvider::default());
        let bridge = LiveTtsStreamFeedbackBridge::new(
            provider.clone(),
            LiveVoiceFeedbackConfig::enabled().with_provider_id("stream-tts"),
        );

        let chunks = bridge
            .collect_chain(&MessageChain::plain(" hello "))
            .await
            .expect("stream feedback should collect");

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].source_text.as_deref(), Some("hello"));
        assert_eq!(chunks[0].audio, vec![1, 2, 3]);
        assert!(!chunks[0].terminal);
        assert!(chunks[1].terminal);

        let requests = provider.requests.lock().expect("tts stream requests lock");
        assert_eq!(requests[0].provider_id.as_deref(), Some("stream-tts"));
        assert_eq!(requests[0].accumulated_text(), "hello");
    }
}
