use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use super::audio_queue::{TextToSpeechAudioChunk, TextToSpeechAudioQueueItem};
use super::{TextToSpeechProvider, TextToSpeechRequest};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextToSpeechStreamText {
    pub text: String,
    pub terminal: bool,
}

impl TextToSpeechStreamText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            terminal: false,
        }
    }

    pub fn terminal() -> Self {
        Self {
            text: String::new(),
            terminal: true,
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextToSpeechStreamRequest {
    pub provider_id: Option<String>,
    pub text: Vec<TextToSpeechStreamText>,
}

impl TextToSpeechStreamRequest {
    pub fn new(text: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            provider_id: None,
            text: text
                .into_iter()
                .map(|text| TextToSpeechStreamText::new(text.into()))
                .collect(),
        }
    }

    pub fn single(text: impl Into<String>) -> Self {
        Self::new([text.into()])
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }

    pub fn push_text(&mut self, text: impl Into<String>) {
        self.text.push(TextToSpeechStreamText::new(text));
    }

    pub fn push_terminal(&mut self) {
        self.text.push(TextToSpeechStreamText::terminal());
    }

    pub fn accumulated_text(&self) -> String {
        self.text
            .iter()
            .filter(|part| !part.is_terminal())
            .map(|part| part.text.as_str())
            .collect()
    }
}

#[async_trait]
pub trait TextToSpeechAudioStream: Send {
    async fn next_audio(&mut self) -> Result<Option<TextToSpeechAudioChunk>>;
}

#[async_trait]
pub trait TextToSpeechStreamProvider: Send + Sync {
    async fn stream_audio(
        &self,
        request: TextToSpeechStreamRequest,
    ) -> Result<Box<dyn TextToSpeechAudioStream>>;
}

pub struct QueuedTextToSpeechAudioStream {
    items: Vec<TextToSpeechAudioQueueItem>,
    index: usize,
    terminal_seen: bool,
}

impl QueuedTextToSpeechAudioStream {
    pub fn new(items: impl IntoIterator<Item = TextToSpeechAudioQueueItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            index: 0,
            terminal_seen: false,
        }
    }
}

#[async_trait]
impl TextToSpeechAudioStream for QueuedTextToSpeechAudioStream {
    async fn next_audio(&mut self) -> Result<Option<TextToSpeechAudioChunk>> {
        if self.terminal_seen {
            return Ok(None);
        }

        let Some(item) = self.items.get(self.index).cloned() else {
            self.terminal_seen = true;
            return Ok(None);
        };
        self.index += 1;

        let item = item.into_result()?;
        if item.is_none() {
            self.terminal_seen = true;
        }
        Ok(item)
    }
}

pub struct FileSynthesisTextToSpeechStreamProvider<T> {
    provider: T,
}

impl<T> FileSynthesisTextToSpeechStreamProvider<T> {
    pub fn new(provider: T) -> Self {
        Self { provider }
    }

    pub fn provider(&self) -> &T {
        &self.provider
    }
}

#[async_trait]
impl<T> TextToSpeechStreamProvider for FileSynthesisTextToSpeechStreamProvider<T>
where
    T: TextToSpeechProvider,
{
    async fn stream_audio(
        &self,
        request: TextToSpeechStreamRequest,
    ) -> Result<Box<dyn TextToSpeechAudioStream>> {
        let text = request.accumulated_text();
        if text.trim().is_empty() {
            return Ok(Box::new(QueuedTextToSpeechAudioStream::new([
                TextToSpeechAudioQueueItem::terminal(),
            ])));
        }

        let mut file_request = TextToSpeechRequest::new(text.trim());
        if let Some(provider_id) = request.provider_id {
            file_request = file_request.with_provider_id(provider_id);
        }

        let response = self.provider.synthesize(file_request).await?;
        let audio = std::fs::read(&response.audio_path).map_err(|err| {
            AstrbotError::Provider(format!(
                "failed to read synthesized TTS audio artifact {}: {err}",
                response.audio_path
            ))
        })?;

        Ok(Box::new(QueuedTextToSpeechAudioStream::new([
            TextToSpeechAudioQueueItem::Chunk(
                TextToSpeechAudioChunk::new(audio).with_source_text(text.trim()),
            ),
            TextToSpeechAudioQueueItem::terminal(),
        ])))
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;

    use astrbot_core::{AstrbotError, Result};
    use async_trait::async_trait;

    use super::{
        FileSynthesisTextToSpeechStreamProvider, QueuedTextToSpeechAudioStream,
        TextToSpeechAudioStream, TextToSpeechStreamProvider, TextToSpeechStreamRequest,
    };
    use crate::tts::audio_queue::{TextToSpeechAudioChunk, TextToSpeechAudioQueueItem};
    use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

    struct FileProvider {
        audio_path: PathBuf,
        requests: Mutex<Vec<TextToSpeechRequest>>,
    }

    #[async_trait]
    impl TextToSpeechProvider for FileProvider {
        async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
            self.requests
                .lock()
                .expect("tts requests lock")
                .push(request);
            Ok(TextToSpeechResponse::new(
                self.audio_path.to_string_lossy().to_string(),
            ))
        }
    }

    #[tokio::test]
    async fn queued_stream_yields_chunks_until_terminal() {
        let mut stream = QueuedTextToSpeechAudioStream::new([
            TextToSpeechAudioQueueItem::Chunk(
                TextToSpeechAudioChunk::new([1, 2, 3]).with_source_text("hello"),
            ),
            TextToSpeechAudioQueueItem::terminal(),
            TextToSpeechAudioQueueItem::chunk([9]),
        ]);

        let first = stream
            .next_audio()
            .await
            .expect("chunk should be readable")
            .expect("first item should be chunk");
        assert_eq!(first.source_text.as_deref(), Some("hello"));
        assert_eq!(first.audio, vec![1, 2, 3]);

        assert!(
            stream
                .next_audio()
                .await
                .expect("terminal should not fail")
                .is_none()
        );
        assert!(
            stream
                .next_audio()
                .await
                .expect("terminal should stick")
                .is_none()
        );
    }

    #[tokio::test]
    async fn queued_stream_surfaces_stream_errors() {
        let mut stream =
            QueuedTextToSpeechAudioStream::new([TextToSpeechAudioQueueItem::error("bad stream")]);

        let error = stream
            .next_audio()
            .await
            .expect_err("error item should fail stream pull");
        assert!(matches!(error, AstrbotError::Provider(message) if message == "bad stream"));
    }

    #[tokio::test]
    async fn file_synthesis_adapter_keeps_file_provider_independently_testable() {
        let audio_path =
            std::env::temp_dir().join(format!("astrbot-rs-tts-stream-{}.wav", std::process::id()));
        fs::write(&audio_path, [7, 8, 9]).expect("write fixture audio");

        let provider = FileProvider {
            audio_path: audio_path.clone(),
            requests: Mutex::default(),
        };
        let adapter = FileSynthesisTextToSpeechStreamProvider::new(provider);
        let request = TextToSpeechStreamRequest::single("hello ").with_provider_id("file-tts");

        let mut stream = adapter
            .stream_audio(request)
            .await
            .expect("file synthesis should adapt to stream");
        let chunk = stream
            .next_audio()
            .await
            .expect("stream pull should work")
            .expect("adapter should emit audio chunk");

        assert_eq!(chunk.source_text.as_deref(), Some("hello"));
        assert_eq!(chunk.audio, vec![7, 8, 9]);
        assert!(
            stream
                .next_audio()
                .await
                .expect("terminal should work")
                .is_none()
        );

        let requests = adapter
            .provider()
            .requests
            .lock()
            .expect("tts requests lock");
        assert_eq!(requests[0].provider_id.as_deref(), Some("file-tts"));
        assert_eq!(requests[0].text, "hello");

        let _ = fs::remove_file(audio_path);
    }
}
