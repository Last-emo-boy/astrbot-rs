use serde::{Deserialize, Serialize};

use super::chain::MessageChain;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventResultType {
    Continue,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultContentType {
    General,
    Llm,
    AgentRunnerError,
    Streaming,
    StreamingFinish,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageStream {
    chunks: Vec<MessageChain>,
}

impl MessageStream {
    pub fn new(chunks: Vec<MessageChain>) -> Self {
        Self { chunks }
    }

    pub fn from_chunk(chunk: impl Into<MessageChain>) -> Self {
        Self {
            chunks: vec![chunk.into()],
        }
    }

    pub fn push(&mut self, chunk: impl Into<MessageChain>) {
        self.chunks.push(chunk.into());
    }

    pub fn chunks(&self) -> &[MessageChain] {
        &self.chunks
    }

    pub fn into_chunks(self) -> Vec<MessageChain> {
        self.chunks
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty() || self.chunks.iter().all(MessageChain::is_empty)
    }
}

impl From<Vec<MessageChain>> for MessageStream {
    fn from(value: Vec<MessageChain>) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEventResult {
    pub chain: MessageChain,
    pub result_type: EventResultType,
    pub content_type: ResultContentType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<MessageStream>,
}

impl MessageEventResult {
    pub fn general(chain: impl Into<MessageChain>) -> Self {
        Self {
            chain: chain.into(),
            result_type: EventResultType::Continue,
            content_type: ResultContentType::General,
            stream: None,
        }
    }

    pub fn llm(chain: impl Into<MessageChain>) -> Self {
        Self {
            chain: chain.into(),
            result_type: EventResultType::Continue,
            content_type: ResultContentType::Llm,
            stream: None,
        }
    }

    pub fn streaming(stream: impl Into<MessageStream>) -> Self {
        Self {
            chain: MessageChain::default(),
            result_type: EventResultType::Continue,
            content_type: ResultContentType::Streaming,
            stream: Some(stream.into()),
        }
    }

    pub fn streaming_finish(chain: impl Into<MessageChain>) -> Self {
        Self {
            chain: chain.into(),
            result_type: EventResultType::Continue,
            content_type: ResultContentType::StreamingFinish,
            stream: None,
        }
    }

    pub fn with_stream(mut self, stream: impl Into<MessageStream>) -> Self {
        self.stream = Some(stream.into());
        self
    }

    pub fn stop(mut self) -> Self {
        self.result_type = EventResultType::Stop;
        self
    }

    pub fn is_stopped(&self) -> bool {
        self.result_type == EventResultType::Stop
    }

    pub fn is_streaming(&self) -> bool {
        self.content_type == ResultContentType::Streaming
    }

    pub fn is_streaming_finish(&self) -> bool {
        self.content_type == ResultContentType::StreamingFinish
    }
}
