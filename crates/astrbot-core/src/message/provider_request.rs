use serde::{Deserialize, Serialize};

use super::event::MessageEvent;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub image_urls: Vec<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_prefix: Option<String>,
    #[serde(default)]
    pub contexts: Vec<ProviderContextMessage>,
    #[serde(default)]
    pub extra_user_content_parts: Vec<ProviderContentPart>,
    #[serde(default)]
    pub tool_placeholders: Vec<ProviderToolPlaceholder>,
    #[serde(default)]
    pub tool_call_results: Vec<ProviderToolCallResult>,
}

impl ProviderRequest {
    pub fn new(prompt: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            prompt: Some(prompt.into()),
            session_id: Some(session_id.into()),
            ..Self::default()
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }

    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = non_empty_option(session_id);
        self
    }

    pub fn with_image_url(mut self, image_url: impl Into<String>) -> Self {
        let image_url = image_url.into();
        if !image_url.trim().is_empty() {
            self.image_urls.push(image_url);
        }
        self
    }

    pub fn with_image_urls<I, S>(mut self, image_urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.image_urls.extend(
            image_urls
                .into_iter()
                .map(Into::into)
                .filter(|url| !url.trim().is_empty()),
        );
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
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

    pub fn has_user_content(&self) -> bool {
        self.prompt
            .as_deref()
            .is_some_and(|prompt| !prompt.trim().is_empty())
            || !self.image_urls.is_empty()
            || !self.extra_user_content_parts.is_empty()
    }

    pub fn from_event(event: &MessageEvent) -> Self {
        Self {
            prompt: Some(event.message.plain_text()),
            session_id: Some(event.session.conversation_id.clone()),
            image_urls: event.message.image_urls(),
            ..Self::default()
        }
    }

    pub fn with_event_defaults(mut self, event: &MessageEvent) -> Self {
        if self
            .session_id
            .as_deref()
            .is_none_or(|session_id| session_id.trim().is_empty())
        {
            self.session_id = Some(event.session.conversation_id.clone());
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderContextMessage {
    pub role: String,
    pub parts: Vec<ProviderContentPart>,
}

impl ProviderContextMessage {
    pub fn new(role: impl Into<String>, parts: Vec<ProviderContentPart>) -> Self {
        Self {
            role: role.into(),
            parts,
        }
    }

    pub fn text(role: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(role, vec![ProviderContentPart::text(text)])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderContentPart {
    Text { text: String },
    ImageUrl { url: String },
}

impl ProviderContentPart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn image_url(url: impl Into<String>) -> Self {
        Self::ImageUrl { url: url.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderToolPlaceholder {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ProviderToolPlaceholder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = non_empty_option(description);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderToolCallResult {
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
}

impl ProviderToolCallResult {
    pub fn new(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            name: name.into(),
            content: content.into(),
        }
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}
