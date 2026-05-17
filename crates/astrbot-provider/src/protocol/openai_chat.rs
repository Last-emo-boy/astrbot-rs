use astrbot_core::{AstrbotError, MessageChain, ProviderContentPart, Result};
use serde::Serialize;
use serde_json::Value;

use crate::ChatRequest;
use crate::streaming::{normalize_stream_text_delta, sse_data_lines};
use crate::{
    ProviderRawResponse, ProviderReasoningMetadata, ProviderResponse, ProviderResponseMetadata,
    ProviderTokenUsage, ProviderToolCall, ProviderToolCallArguments,
};

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiChatCompletionRequest {
    model: String,
    messages: Vec<OpenAiChatMessage>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OpenAiChatMessage {
    role: String,
    content: OpenAiChatContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenAiChatContent {
    Text(String),
    Parts(Vec<OpenAiContentPart>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum OpenAiContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: OpenAiImageUrl },
}

#[derive(Debug, Serialize)]
struct OpenAiImageUrl {
    url: String,
}

pub(crate) fn build_openai_chat_completion_request(
    request: &ChatRequest,
    default_model: &str,
) -> OpenAiChatCompletionRequest {
    let mut messages = Vec::new();
    if let Some(system_prompt) = request
        .system_prompt
        .as_deref()
        .filter(|system_prompt| !system_prompt.trim().is_empty())
    {
        messages.push(OpenAiChatMessage {
            role: "system".to_string(),
            content: OpenAiChatContent::Text(system_prompt.to_string()),
        });
    }
    messages.extend(request.contexts.iter().map(|context| OpenAiChatMessage {
        role: context.role.clone(),
        content: build_content_from_parts(&context.parts),
    }));
    messages.push(OpenAiChatMessage {
        role: "user".to_string(),
        content: build_user_content(request),
    });

    OpenAiChatCompletionRequest {
        model: request
            .model
            .clone()
            .unwrap_or_else(|| default_model.to_string()),
        messages,
        stream: request.stream,
    }
}

pub(crate) fn extract_openai_response(body: &str) -> Result<ProviderResponse> {
    let payload: Value = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    let (content, tagged_reasoning) = strip_reasoning_tags(extract_message_content(&payload));
    let mut metadata = extract_response_metadata(&payload, "openai");
    if metadata.reasoning.is_none()
        && let Some(reasoning) = tagged_reasoning
    {
        metadata = metadata.with_reasoning(reasoning);
    }
    Ok(ProviderResponse::new(
        MessageChain::plain(content),
        metadata,
    ))
}

pub(crate) fn collect_openai_streaming_response(body: &str) -> Result<ProviderResponse> {
    let mut content = String::new();
    let mut metadata = ProviderResponseMetadata::default();
    for data in sse_data_lines(body) {
        if data == "[DONE]" {
            continue;
        }
        let parsed = parse_stream_data(data)?;
        if let Some(delta) = parsed.delta {
            content.push_str(&delta);
        }
        metadata = metadata.merge_missing(parsed.metadata);
    }
    Ok(ProviderResponse::new(
        MessageChain::plain(content),
        metadata,
    ))
}

fn build_user_content(request: &ChatRequest) -> OpenAiChatContent {
    let image_urls = request
        .image_urls
        .iter()
        .filter(|url| !url.trim().is_empty())
        .collect::<Vec<_>>();

    if image_urls.is_empty() && request.extra_user_content_parts.is_empty() {
        return OpenAiChatContent::Text(request.prompt.clone());
    }

    let mut parts =
        Vec::with_capacity(image_urls.len() + request.extra_user_content_parts.len() + 1);
    if !request.prompt.trim().is_empty() || !image_urls.is_empty() {
        parts.push(OpenAiContentPart::Text {
            text: if request.prompt.trim().is_empty() {
                "[图片]".to_string()
            } else {
                request.prompt.clone()
            },
        });
    }

    parts.extend(
        request
            .extra_user_content_parts
            .iter()
            .map(openai_part_from_provider_part),
    );

    for image_url in image_urls {
        parts.push(OpenAiContentPart::ImageUrl {
            image_url: OpenAiImageUrl {
                url: image_url.to_string(),
            },
        });
    }

    OpenAiChatContent::Parts(parts)
}

fn build_content_from_parts(parts: &[ProviderContentPart]) -> OpenAiChatContent {
    if let [ProviderContentPart::Text { text }] = parts {
        return OpenAiChatContent::Text(text.clone());
    }

    OpenAiChatContent::Parts(parts.iter().map(openai_part_from_provider_part).collect())
}

fn openai_part_from_provider_part(part: &ProviderContentPart) -> OpenAiContentPart {
    match part {
        ProviderContentPart::Text { text } => OpenAiContentPart::Text { text: text.clone() },
        ProviderContentPart::ImageUrl { url } => OpenAiContentPart::ImageUrl {
            image_url: OpenAiImageUrl { url: url.clone() },
        },
    }
}

fn normalize_content(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Array(items) => {
            let text_parts = items
                .iter()
                .filter_map(|item| {
                    if item.get("type").and_then(Value::as_str) == Some("text") {
                        item.get("text").and_then(Value::as_str)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if text_parts.is_empty() {
                value.to_string()
            } else {
                text_parts.join("")
            }
        }
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn extract_message_content(payload: &Value) -> String {
    payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .map(normalize_content)
        .unwrap_or_default()
}

struct OpenAiStreamData {
    delta: Option<String>,
    metadata: ProviderResponseMetadata,
}

fn parse_stream_data(data: &str) -> Result<OpenAiStreamData> {
    let payload: Value = serde_json::from_str(data).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider stream JSON: {err}"))
    })?;
    let delta = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
        .map(normalize_stream_text_delta)
        .unwrap_or_default();

    Ok(OpenAiStreamData {
        delta: (!delta.is_empty()).then_some(delta),
        metadata: extract_response_metadata(&payload, "openai_stream"),
    })
}

fn extract_response_metadata(payload: &Value, provider: &str) -> ProviderResponseMetadata {
    let mut metadata = ProviderResponseMetadata::default()
        .with_raw_response(ProviderRawResponse::new(provider, payload.clone()));

    if let Some(id) = payload.get("id").and_then(Value::as_str) {
        metadata = metadata.with_response_id(id);
    }
    if let Some(model) = payload.get("model").and_then(Value::as_str) {
        metadata = metadata.with_model(model);
    }
    if let Some(usage) = payload.get("usage").and_then(extract_usage) {
        metadata = metadata.with_usage(usage);
    }

    let Some(choice) = first_choice(payload) else {
        return metadata;
    };

    if let Some(finish_reason) = choice.get("finish_reason").and_then(Value::as_str) {
        metadata = metadata.with_finish_reason(finish_reason);
    }

    if let Some(reasoning) = choice
        .get("message")
        .and_then(extract_reasoning)
        .or_else(|| choice.get("delta").and_then(extract_reasoning))
    {
        metadata = metadata.with_reasoning(reasoning);
    }

    if let Some(tool_calls) = choice
        .get("message")
        .and_then(|message| message.get("tool_calls"))
        .or_else(|| {
            choice
                .get("delta")
                .and_then(|delta| delta.get("tool_calls"))
        })
        .and_then(Value::as_array)
    {
        for tool_call in tool_calls.iter().filter_map(extract_tool_call) {
            metadata = metadata.with_tool_call(tool_call);
        }
    }

    metadata
}

fn first_choice(payload: &Value) -> Option<&Value> {
    payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
}

fn extract_usage(value: &Value) -> Option<ProviderTokenUsage> {
    let prompt_tokens = value
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = value
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cached_tokens = value
        .get("prompt_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let usage = ProviderTokenUsage::openai(prompt_tokens, cached_tokens, completion_tokens);
    (!usage.is_empty()).then_some(usage)
}

fn extract_reasoning(value: &Value) -> Option<ProviderReasoningMetadata> {
    let content = value
        .get("reasoning_content")
        .or_else(|| value.get("reasoning"))
        .or_else(|| value.get("thinking"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let signature = value
        .get("reasoning_signature")
        .or_else(|| value.get("signature"))
        .and_then(Value::as_str);

    let mut reasoning = ProviderReasoningMetadata::new(content);
    if let Some(signature) = signature {
        reasoning = reasoning.with_signature(signature);
    }
    (!reasoning.is_empty()).then_some(reasoning)
}

fn extract_tool_call(value: &Value) -> Option<ProviderToolCall> {
    let id = value.get("id").and_then(Value::as_str).unwrap_or_default();
    let function = value.get("function")?;
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if id.trim().is_empty() || name.trim().is_empty() {
        return None;
    }
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .map(ProviderToolCallArguments::from_raw)
        .unwrap_or(ProviderToolCallArguments::Empty);
    let mut tool_call = ProviderToolCall::new(id, name, arguments);
    if let Some(extra_content) = value.get("extra_content") {
        tool_call = tool_call.with_extra_content(extra_content.clone());
    }
    Some(tool_call)
}

trait MergeMetadata {
    fn merge_missing(self, other: ProviderResponseMetadata) -> ProviderResponseMetadata;
}

impl MergeMetadata for ProviderResponseMetadata {
    fn merge_missing(mut self, other: ProviderResponseMetadata) -> ProviderResponseMetadata {
        if self.response_id.is_none() {
            self.response_id = other.response_id;
        }
        if self.model.is_none() {
            self.model = other.model;
        }
        if self.finish_reason.is_none() {
            self.finish_reason = other.finish_reason;
        }
        if self.stop_reason.is_none() {
            self.stop_reason = other.stop_reason;
        }
        if self.usage.is_none() {
            self.usage = other.usage;
        }
        if self.reasoning.is_none() {
            self.reasoning = other.reasoning;
        }
        if self.tool_calls.is_empty() {
            self.tool_calls = other.tool_calls;
        }
        if self.raw_response.is_none() {
            self.raw_response = other.raw_response;
        }
        self
    }
}

fn strip_reasoning_tags(content: String) -> (String, Option<ProviderReasoningMetadata>) {
    let mut visible = String::new();
    let mut reasoning = Vec::new();
    let mut rest = content.as_str();

    while let Some(start) = rest.find("<think>") {
        visible.push_str(&rest[..start]);
        let after_start = &rest[start + "<think>".len()..];
        let Some(end) = after_start.find("</think>") else {
            visible.push_str(&rest[start..]);
            return (visible.trim().to_string(), None);
        };
        reasoning.push(after_start[..end].trim().to_string());
        rest = &after_start[end + "</think>".len()..];
    }
    visible.push_str(rest);

    let reasoning = reasoning
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if reasoning.is_empty() {
        (visible.trim().to_string(), None)
    } else {
        (
            visible.trim().to_string(),
            Some(ProviderReasoningMetadata::new(reasoning)),
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{collect_openai_streaming_response, normalize_content};
    use crate::http::extract_error_message;

    #[test]
    fn normalize_openai_content_shapes() {
        assert_eq!(normalize_content(&json!(" hello ")), "hello");
        assert_eq!(
            normalize_content(&json!({"type": "text", "text": "hello"})),
            "hello"
        );
        assert_eq!(
            normalize_content(&json!([
                {"type": "text", "text": "hello"},
                {"type": "text", "text": " world"},
                {"type": "image_url", "image_url": {"url": "ignored"}}
            ])),
            "hello world"
        );
    }

    #[test]
    fn extracts_structured_error_message() {
        assert_eq!(
            extract_error_message(r#"{"error":{"message":"bad request"}}"#),
            "bad request"
        );
    }

    #[test]
    fn collects_openai_streaming_content() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
            "data: [DONE]\n\n"
        );

        let content = collect_openai_streaming_response(body)
            .expect("stream should parse")
            .chain
            .plain_text();

        assert_eq!(content, "hello world");
    }
}
