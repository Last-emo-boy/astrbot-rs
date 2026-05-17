use astrbot_core::AstrbotError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextToSpeechAudioChunk {
    pub source_text: Option<String>,
    pub audio: Vec<u8>,
    pub mime_type: Option<String>,
}

impl TextToSpeechAudioChunk {
    pub fn new(audio: impl Into<Vec<u8>>) -> Self {
        Self {
            source_text: None,
            audio: audio.into(),
            mime_type: None,
        }
    }

    pub fn with_source_text(mut self, source_text: impl Into<String>) -> Self {
        self.source_text = non_empty_option(source_text);
        self
    }

    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = non_empty_option(mime_type);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.audio.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextToSpeechAudioQueueItem {
    Chunk(TextToSpeechAudioChunk),
    Terminal,
    Error(String),
}

impl TextToSpeechAudioQueueItem {
    pub fn chunk(audio: impl Into<Vec<u8>>) -> Self {
        Self::Chunk(TextToSpeechAudioChunk::new(audio))
    }

    pub fn text_chunk(source_text: impl Into<String>, audio: impl Into<Vec<u8>>) -> Self {
        Self::Chunk(TextToSpeechAudioChunk::new(audio).with_source_text(source_text))
    }

    pub fn terminal() -> Self {
        Self::Terminal
    }

    pub fn error(message: impl Into<String>) -> Self {
        let message = message.into();
        Self::Error(if message.trim().is_empty() {
            "text-to-speech stream failed".to_string()
        } else {
            message
        })
    }

    pub fn into_result(self) -> Result<Option<TextToSpeechAudioChunk>, AstrbotError> {
        match self {
            Self::Chunk(chunk) => Ok(Some(chunk)),
            Self::Terminal => Ok(None),
            Self::Error(message) => Err(AstrbotError::Provider(message)),
        }
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use astrbot_core::AstrbotError;

    use super::{TextToSpeechAudioChunk, TextToSpeechAudioQueueItem};

    #[test]
    fn audio_chunk_can_carry_optional_source_text_and_mime_type() {
        let chunk = TextToSpeechAudioChunk::new([1, 2, 3])
            .with_source_text(" hello ")
            .with_mime_type(" audio/wav ");

        assert_eq!(chunk.source_text.as_deref(), Some("hello"));
        assert_eq!(chunk.mime_type.as_deref(), Some("audio/wav"));
        assert_eq!(chunk.audio, vec![1, 2, 3]);
        assert!(!chunk.is_empty());
    }

    #[test]
    fn queue_item_models_terminal_and_errors_without_provider_protocol() {
        assert!(
            TextToSpeechAudioQueueItem::terminal()
                .into_result()
                .expect("terminal should not fail")
                .is_none()
        );

        let error = TextToSpeechAudioQueueItem::error("stream broken")
            .into_result()
            .expect_err("error item should map to provider error");
        assert!(matches!(error, AstrbotError::Provider(message) if message == "stream broken"));
    }
}
