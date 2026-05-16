use astrbot_core::{AstrbotError, ProviderContentPart, Result};
use serde::Serialize;
use serde_json::Value;

use crate::ChatRequest;
use crate::streaming::{normalize_stream_text_delta, sse_data_lines};

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

pub(crate) fn extract_openai_message_content(body: &str) -> Result<String> {
    let payload: Value = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    Ok(extract_message_content(&payload))
}

pub(crate) fn collect_openai_streaming_content(body: &str) -> Result<String> {
    let mut content = String::new();
    for data in sse_data_lines(body) {
        if data == "[DONE]" {
            continue;
        }
        if let Some(delta) = parse_stream_data(data)? {
            content.push_str(&delta);
        }
    }
    Ok(content)
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

fn parse_stream_data(data: &str) -> Result<Option<String>> {
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

    if delta.is_empty() {
        Ok(None)
    } else {
        Ok(Some(delta))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{collect_openai_streaming_content, normalize_content};
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

        let content = collect_openai_streaming_content(body).expect("stream should parse");

        assert_eq!(content, "hello world");
    }
}
