use std::sync::Arc;

use astrbot_core::{MessageChain, Result};
use astrbot_provider::{TextToSpeechProvider, TextToSpeechRequest};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceFeedbackMode {
    Disabled,
    SynthesizedAudio,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveVoiceFeedbackConfig {
    pub mode: VoiceFeedbackMode,
    pub provider_id: Option<String>,
    pub prefer_streaming: bool,
    pub min_text_chars: usize,
}

impl Default for LiveVoiceFeedbackConfig {
    fn default() -> Self {
        Self {
            mode: VoiceFeedbackMode::Disabled,
            provider_id: None,
            prefer_streaming: true,
            min_text_chars: 1,
        }
    }
}

impl LiveVoiceFeedbackConfig {
    pub fn enabled() -> Self {
        Self {
            mode: VoiceFeedbackMode::SynthesizedAudio,
            ..Self::default()
        }
    }

    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }

    pub fn prefer_streaming(mut self, prefer_streaming: bool) -> Self {
        self.prefer_streaming = prefer_streaming;
        self
    }

    pub fn with_min_text_chars(mut self, min_text_chars: usize) -> Self {
        self.min_text_chars = min_text_chars.max(1);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.mode == VoiceFeedbackMode::SynthesizedAudio
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceFeedbackEvent {
    pub text: String,
    pub audio_path: String,
    pub streaming_supported: bool,
}

pub struct LiveVoiceFeedbackBridge {
    config: LiveVoiceFeedbackConfig,
    provider: Option<Arc<dyn TextToSpeechProvider>>,
}

impl LiveVoiceFeedbackBridge {
    pub fn disabled() -> Self {
        Self {
            config: LiveVoiceFeedbackConfig::disabled(),
            provider: None,
        }
    }

    pub fn new(provider: Arc<dyn TextToSpeechProvider>, config: LiveVoiceFeedbackConfig) -> Self {
        Self {
            config,
            provider: Some(provider),
        }
    }

    pub fn config(&self) -> &LiveVoiceFeedbackConfig {
        &self.config
    }

    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled() && self.provider.is_some()
    }

    pub fn supports_streaming(&self) -> bool {
        self.config.prefer_streaming
            && self
                .provider
                .as_ref()
                .is_some_and(|provider| provider.supports_streaming())
    }

    pub async fn synthesize_chain(
        &self,
        chain: &MessageChain,
    ) -> Result<Option<VoiceFeedbackEvent>> {
        if !self.is_enabled() {
            return Ok(None);
        }

        let text = chain.plain_text();
        if text.trim().chars().count() < self.config.min_text_chars {
            return Ok(None);
        }

        let provider = self.provider.as_ref().expect("provider checked above");
        let mut request = TextToSpeechRequest::new(text.trim());
        if let Some(provider_id) = &self.config.provider_id {
            request = request.with_provider_id(provider_id.clone());
        }

        let response = provider.synthesize(request).await?;
        Ok(Some(VoiceFeedbackEvent {
            text: text.trim().to_string(),
            audio_path: response.audio_path,
            streaming_supported: self.supports_streaming(),
        }))
    }
}

impl Default for LiveVoiceFeedbackBridge {
    fn default() -> Self {
        Self::disabled()
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use astrbot_core::{MessageChain, Result};
    use astrbot_provider::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};
    use async_trait::async_trait;

    use super::{LiveVoiceFeedbackBridge, LiveVoiceFeedbackConfig};

    #[derive(Default)]
    struct RecordingTtsProvider {
        requests: Mutex<Vec<TextToSpeechRequest>>,
        streaming: bool,
    }

    #[async_trait]
    impl TextToSpeechProvider for RecordingTtsProvider {
        async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
            self.requests
                .lock()
                .expect("tts request lock")
                .push(request);
            Ok(TextToSpeechResponse::new("voice.wav"))
        }

        fn supports_streaming(&self) -> bool {
            self.streaming
        }
    }

    #[tokio::test]
    async fn voice_feedback_can_be_disabled_without_core_runner_changes() {
        let bridge = LiveVoiceFeedbackBridge::disabled();

        let event = bridge
            .synthesize_chain(&MessageChain::plain("hello"))
            .await
            .expect("disabled bridge should not fail");

        assert_eq!(event, None);
    }

    #[tokio::test]
    async fn voice_feedback_synthesizes_plain_text_when_enabled() {
        let provider = Arc::new(RecordingTtsProvider {
            streaming: true,
            ..RecordingTtsProvider::default()
        });
        let bridge = LiveVoiceFeedbackBridge::new(
            provider.clone(),
            LiveVoiceFeedbackConfig::enabled().with_provider_id("tts-1"),
        );

        let event = bridge
            .synthesize_chain(&MessageChain::plain("hello"))
            .await
            .expect("tts should synthesize")
            .expect("enabled bridge should emit voice feedback");

        assert_eq!(event.text, "hello");
        assert_eq!(event.audio_path, "voice.wav");
        assert!(event.streaming_supported);
        let requests = provider.requests.lock().expect("tts request lock");
        assert_eq!(requests[0].provider_id.as_deref(), Some("tts-1"));
        assert_eq!(requests[0].text, "hello");
    }
}
