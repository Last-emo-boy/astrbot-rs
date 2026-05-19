use std::fmt;

use astrbot_core::{MessageChain, MessageComponent};
use astrbot_storage::ApiKeyRecord;
use serde::{Deserialize, Serialize};

use crate::management::OpenApiScope;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenApiChatAuthContext {
    pub key_id: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
}

impl OpenApiChatAuthContext {
    pub fn new(
        key_id: impl Into<String>,
        key_prefix: impl Into<String>,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            key_prefix: key_prefix.into(),
            scopes: scopes.into_iter().map(Into::into).collect(),
        }
    }

    pub fn from_api_key_record(record: &ApiKeyRecord) -> Self {
        Self::new(
            record.key_id.clone(),
            record.key_prefix.clone(),
            record.scopes.clone(),
        )
    }

    pub fn has_chat_scope(&self) -> bool {
        self.scopes.iter().any(|scope| {
            let scope = scope.trim();
            scope == OpenApiScope::Chat.as_str() || scope == "openapi.chat" || scope == "*"
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiChatMessageRequest {
    pub conversation_id: String,
    #[serde(default)]
    pub sender_id: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub message_parts: Vec<OpenApiChatMessagePart>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub stream: bool,
}

impl OpenApiChatMessageRequest {
    pub fn response_mode(&self) -> OpenApiChatResponseMode {
        if self.stream {
            OpenApiChatResponseMode::Streaming
        } else {
            OpenApiChatResponseMode::Blocking
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpenApiChatMessagePart {
    #[serde(alias = "text")]
    Plain { text: String },
    Image {
        #[serde(alias = "image_url")]
        url: String,
    },
    Reply {
        message_id: String,
        #[serde(default)]
        selected_text: String,
    },
    #[serde(alias = "audio")]
    Record {
        #[serde(alias = "record_url", alias = "audio_url")]
        url: String,
    },
    Video {
        #[serde(alias = "video_url")]
        url: String,
    },
    File {
        #[serde(default, alias = "filename")]
        name: String,
        #[serde(alias = "file_url")]
        url: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenApiChatResponseMode {
    Blocking,
    Streaming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenApiChatGatewayRequest {
    pub auth: OpenApiChatAuthContext,
    pub conversation_id: String,
    pub sender_id: String,
    pub request_id: Option<String>,
    pub message: MessageChain,
    pub response_mode: OpenApiChatResponseMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenApiChatSubscriptionPlan {
    pub conversation_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenApiChatEnqueuePlan {
    pub request: OpenApiChatGatewayRequest,
    pub subscription: Option<OpenApiChatSubscriptionPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenApiChatGatewayError {
    MissingChatScope,
    EmptyConversationId,
    EmptyMessage,
}

impl fmt::Display for OpenApiChatGatewayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChatScope => formatter.write_str("openapi chat scope is required"),
            Self::EmptyConversationId => formatter.write_str("conversation id is required"),
            Self::EmptyMessage => formatter.write_str("chat message is empty"),
        }
    }
}

impl std::error::Error for OpenApiChatGatewayError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenApiChatGateway {
    default_sender_id: String,
}

impl OpenApiChatGateway {
    pub fn new(default_sender_id: impl Into<String>) -> Self {
        let default_sender_id =
            non_empty_string(default_sender_id).unwrap_or_else(|| "openapi".to_string());
        Self { default_sender_id }
    }

    pub fn default_sender_id(&self) -> &str {
        &self.default_sender_id
    }

    pub fn prepare_enqueue(
        &self,
        auth: OpenApiChatAuthContext,
        request: OpenApiChatMessageRequest,
    ) -> Result<OpenApiChatEnqueuePlan, OpenApiChatGatewayError> {
        if !auth.has_chat_scope() {
            return Err(OpenApiChatGatewayError::MissingChatScope);
        }

        let OpenApiChatMessageRequest {
            conversation_id,
            sender_id,
            text,
            message_parts,
            request_id,
            stream,
        } = request;
        let response_mode = if stream {
            OpenApiChatResponseMode::Streaming
        } else {
            OpenApiChatResponseMode::Blocking
        };

        let conversation_id = non_empty_string(conversation_id)
            .ok_or(OpenApiChatGatewayError::EmptyConversationId)?;
        let sender_id = sender_id
            .and_then(non_empty_string)
            .unwrap_or_else(|| self.default_sender_id.clone());
        let request_id = request_id.and_then(non_empty_string);
        let message = message_chain_from_openapi_payload(text, message_parts);
        if message.is_empty() {
            return Err(OpenApiChatGatewayError::EmptyMessage);
        }

        let subscription = (response_mode == OpenApiChatResponseMode::Streaming).then(|| {
            OpenApiChatSubscriptionPlan {
                conversation_id: conversation_id.clone(),
                request_id: request_id
                    .clone()
                    .unwrap_or_else(|| conversation_id.clone()),
            }
        });

        Ok(OpenApiChatEnqueuePlan {
            request: OpenApiChatGatewayRequest {
                auth,
                conversation_id,
                sender_id,
                request_id,
                message,
                response_mode,
            },
            subscription,
        })
    }
}

impl Default for OpenApiChatGateway {
    fn default() -> Self {
        Self::new("openapi")
    }
}

pub fn required_openapi_chat_scopes() -> [OpenApiScope; 1] {
    [OpenApiScope::Chat]
}

fn message_chain_from_openapi_payload(
    text: String,
    message_parts: Vec<OpenApiChatMessagePart>,
) -> MessageChain {
    let mut message = MessageChain::default();
    if !text.trim().is_empty() {
        message.push(MessageComponent::plain(text));
    }

    for part in message_parts {
        match part {
            OpenApiChatMessagePart::Plain { text } => {
                if !text.trim().is_empty() {
                    message.push(MessageComponent::plain(text));
                }
            }
            OpenApiChatMessagePart::Image { url } => {
                let url = url.trim();
                if !url.is_empty() {
                    message.push(MessageComponent::image(url.to_string()));
                }
            }
            OpenApiChatMessagePart::Reply {
                message_id,
                selected_text,
            } => {
                let message_id = message_id.trim();
                if !message_id.is_empty() {
                    message.push(MessageComponent::reply(
                        message_id.to_string(),
                        selected_text,
                    ));
                }
            }
            OpenApiChatMessagePart::Record { url } => {
                let url = url.trim();
                if !url.is_empty() {
                    message.push(MessageComponent::record(url.to_string()));
                }
            }
            OpenApiChatMessagePart::Video { url } => {
                let url = url.trim();
                if !url.is_empty() {
                    message.push(MessageComponent::video(url.to_string()));
                }
            }
            OpenApiChatMessagePart::File { name, url } => {
                let url = url.trim();
                if !url.is_empty() {
                    message.push(MessageComponent::file(name, url.to_string()));
                }
            }
        }
    }

    message
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
