use astrbot_core::{
    MessageChain, ProviderContentPart, ProviderContextMessage,
    ProviderRequest as EventProviderRequest, ProviderToolCallResult, ProviderToolPlaceholder,
    Result,
};
use async_trait::async_trait;

use crate::ProviderResponseMetadata;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRequest {
    pub provider_id: Option<String>,
    pub prompt: String,
    pub session_id: String,
    pub stream: bool,
    pub image_urls: Vec<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub wake_prefix: Option<String>,
    pub contexts: Vec<ProviderContextMessage>,
    pub extra_user_content_parts: Vec<ProviderContentPart>,
    pub tool_placeholders: Vec<ProviderToolPlaceholder>,
    pub tool_call_results: Vec<ProviderToolCallResult>,
}

impl ChatRequest {
    pub fn new(prompt: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            provider_id: None,
            prompt: prompt.into(),
            session_id: session_id.into(),
            stream: false,
            image_urls: Vec::new(),
            system_prompt: None,
            model: None,
            wake_prefix: None,
            contexts: Vec::new(),
            extra_user_content_parts: Vec::new(),
            tool_placeholders: Vec::new(),
            tool_call_results: Vec::new(),
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    pub fn with_image_url(mut self, image_url: impl Into<String>) -> Self {
        self.image_urls.push(image_url.into());
        self
    }

    pub fn with_image_urls<I, S>(mut self, image_urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.image_urls
            .extend(image_urls.into_iter().map(Into::into));
        self
    }

    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = non_empty_option(system_prompt);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = non_empty_option(model);
        self
    }

    pub fn with_wake_prefix(mut self, wake_prefix: impl Into<String>) -> Self {
        self.wake_prefix = non_empty_option(wake_prefix);
        self
    }

    pub fn with_context(mut self, context: ProviderContextMessage) -> Self {
        self.contexts.push(context);
        self
    }

    pub fn with_extra_user_content_part(mut self, part: ProviderContentPart) -> Self {
        self.extra_user_content_parts.push(part);
        self
    }

    pub fn with_tool_placeholder(mut self, tool: ProviderToolPlaceholder) -> Self {
        self.tool_placeholders.push(tool);
        self
    }

    pub fn with_tool_call_result(mut self, result: ProviderToolCallResult) -> Self {
        self.tool_call_results.push(result);
        self
    }
}

impl From<EventProviderRequest> for ChatRequest {
    fn from(request: EventProviderRequest) -> Self {
        Self {
            provider_id: request.provider_id,
            prompt: request.prompt.unwrap_or_default(),
            session_id: request.session_id.unwrap_or_default(),
            stream: request.stream,
            image_urls: request.image_urls,
            system_prompt: request.system_prompt,
            model: request.model,
            wake_prefix: request.wake_prefix,
            contexts: request.contexts,
            extra_user_content_parts: request.extra_user_content_parts,
            tool_placeholders: request.tool_placeholders,
            tool_call_results: request.tool_call_results,
        }
    }
}

pub(crate) fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatResponse {
    pub chain: MessageChain,
    pub metadata: ProviderResponseMetadata,
}

impl ChatResponse {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            chain: MessageChain::plain(text),
            metadata: ProviderResponseMetadata::default(),
        }
    }

    pub fn with_metadata(mut self, metadata: ProviderResponseMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MockChatProvider {
    response: String,
}

impl MockChatProvider {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

#[async_trait]
impl ChatProvider for MockChatProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Ok(ChatResponse::text(self.response.clone()))
    }
}
