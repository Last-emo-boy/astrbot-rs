use astrbot_core::{AstrbotError, MessageChain, ProviderContentPart, Result};
use serde::Serialize;
use serde_json::Value;

use crate::ChatRequest;
use crate::{
    ProviderRawResponse, ProviderReasoningMetadata, ProviderResponse, ProviderResponseMetadata,
    ProviderTokenUsage, ProviderToolCall, ProviderToolCallArguments,
};

#[derive(Debug, Serialize)]
pub(crate) struct AnthropicMessageRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: AnthropicImageSource },
}

#[derive(Debug, Serialize)]
struct AnthropicImageSource {
    #[serde(rename = "type")]
    source_type: &'static str,
    media_type: String,
    data: String,
}

pub(crate) fn build_anthropic_message_request(
    request: &ChatRequest,
    default_model: &str,
    max_tokens: u32,
) -> Result<AnthropicMessageRequest> {
    let mut messages = Vec::new();
    for context in &request.contexts {
        if context.role == "system" {
            continue;
        }
        messages.push(AnthropicMessage {
            role: normalize_anthropic_role(&context.role),
            content: content_from_parts(&context.parts)?,
        });
    }

    messages.push(AnthropicMessage {
        role: "user".to_string(),
        content: user_content(request)?,
    });

    Ok(AnthropicMessageRequest {
        model: request
            .model
            .clone()
            .unwrap_or_else(|| default_model.to_string()),
        system: request
            .system_prompt
            .clone()
            .filter(|system_prompt| !system_prompt.trim().is_empty()),
        messages,
        max_tokens,
    })
}

pub(crate) fn extract_anthropic_response(body: &str) -> Result<ProviderResponse> {
    let payload: Value = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    let content = payload
        .get("content")
        .and_then(Value::as_array)
        .map(|content| {
            content
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    let metadata = extract_response_metadata(&payload);
    Ok(ProviderResponse::new(
        MessageChain::plain(content),
        metadata,
    ))
}

fn normalize_anthropic_role(role: &str) -> String {
    if role == "assistant" {
        "assistant".to_string()
    } else {
        "user".to_string()
    }
}

fn user_content(request: &ChatRequest) -> Result<AnthropicContent> {
    let image_urls = request
        .image_urls
        .iter()
        .filter(|url| !url.trim().is_empty())
        .collect::<Vec<_>>();
    if image_urls.is_empty() && request.extra_user_content_parts.is_empty() {
        return Ok(AnthropicContent::Text(request.prompt.clone()));
    }

    let mut blocks =
        Vec::with_capacity(image_urls.len() + request.extra_user_content_parts.len() + 1);
    if !request.prompt.trim().is_empty() || !image_urls.is_empty() {
        blocks.push(AnthropicContentBlock::Text {
            text: if request.prompt.trim().is_empty() {
                "[image]".to_string()
            } else {
                request.prompt.clone()
            },
        });
    }

    for part in &request.extra_user_content_parts {
        push_part_block(&mut blocks, part)?;
    }
    for image_url in image_urls {
        blocks.push(image_block_from_data_url(image_url)?);
    }

    Ok(AnthropicContent::Blocks(blocks))
}

fn content_from_parts(parts: &[ProviderContentPart]) -> Result<AnthropicContent> {
    if let [ProviderContentPart::Text { text }] = parts {
        return Ok(AnthropicContent::Text(text.clone()));
    }

    let mut blocks = Vec::with_capacity(parts.len());
    for part in parts {
        push_part_block(&mut blocks, part)?;
    }
    Ok(AnthropicContent::Blocks(blocks))
}

fn push_part_block(
    blocks: &mut Vec<AnthropicContentBlock>,
    part: &ProviderContentPart,
) -> Result<()> {
    match part {
        ProviderContentPart::Text { text } => {
            blocks.push(AnthropicContentBlock::Text { text: text.clone() });
            Ok(())
        }
        ProviderContentPart::ImageUrl { url } => {
            blocks.push(image_block_from_data_url(url)?);
            Ok(())
        }
    }
}

fn image_block_from_data_url(url: &str) -> Result<AnthropicContentBlock> {
    let url = url.trim();
    let Some(data_url) = url.strip_prefix("data:") else {
        return Err(AstrbotError::Provider(
            "Anthropic image inputs currently require data URLs".to_string(),
        ));
    };
    let Some((metadata, data)) = data_url.split_once(',') else {
        return Err(AstrbotError::Provider(
            "invalid Anthropic image data URL".to_string(),
        ));
    };
    let media_type = metadata
        .split(';')
        .next()
        .filter(|media_type| media_type.starts_with("image/"))
        .ok_or_else(|| AstrbotError::Provider("invalid Anthropic image media type".to_string()))?;

    Ok(AnthropicContentBlock::Image {
        source: AnthropicImageSource {
            source_type: "base64",
            media_type: media_type.to_string(),
            data: data.to_string(),
        },
    })
}

fn extract_response_metadata(payload: &Value) -> ProviderResponseMetadata {
    let mut metadata = ProviderResponseMetadata::default()
        .with_raw_response(ProviderRawResponse::new("anthropic", payload.clone()));

    if let Some(id) = payload.get("id").and_then(Value::as_str) {
        metadata = metadata.with_response_id(id);
    }
    if let Some(model) = payload.get("model").and_then(Value::as_str) {
        metadata = metadata.with_model(model);
    }
    if let Some(stop_reason) = payload.get("stop_reason").and_then(Value::as_str) {
        metadata = metadata.with_stop_reason(stop_reason);
    }
    if let Some(usage) = payload.get("usage").and_then(extract_usage) {
        metadata = metadata.with_usage(usage);
    }
    if let Some(content) = payload.get("content").and_then(Value::as_array) {
        if let Some(reasoning) = extract_reasoning(content) {
            metadata = metadata.with_reasoning(reasoning);
        }
        for tool_call in content.iter().filter_map(extract_tool_call) {
            metadata = metadata.with_tool_call(tool_call);
        }
    }

    metadata
}

fn extract_usage(value: &Value) -> Option<ProviderTokenUsage> {
    let usage = ProviderTokenUsage::new(
        value
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        value
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        value
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    );
    (!usage.is_empty()).then_some(usage)
}

fn extract_reasoning(content: &[Value]) -> Option<ProviderReasoningMetadata> {
    let reasoning_text = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("thinking"))
        .filter_map(|block| block.get("thinking").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    let signature = content
        .iter()
        .find_map(|block| block.get("signature").and_then(Value::as_str));

    let mut reasoning = ProviderReasoningMetadata::new(reasoning_text.trim());
    if let Some(signature) = signature {
        reasoning = reasoning.with_signature(signature);
    }
    (!reasoning.is_empty()).then_some(reasoning)
}

fn extract_tool_call(block: &Value) -> Option<ProviderToolCall> {
    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
        return None;
    }
    let id = block.get("id").and_then(Value::as_str).unwrap_or_default();
    let name = block
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if id.trim().is_empty() || name.trim().is_empty() {
        return None;
    }
    let arguments = block
        .get("input")
        .cloned()
        .map(ProviderToolCallArguments::Json)
        .unwrap_or(ProviderToolCallArguments::Empty);
    Some(ProviderToolCall::new(id, name, arguments))
}

#[cfg(test)]
mod tests {
    use astrbot_core::ProviderContentPart;

    use super::{content_from_parts, image_block_from_data_url};

    #[test]
    fn rejects_remote_image_urls_until_transport_download_exists() {
        let error = image_block_from_data_url("https://example.test/image.png")
            .expect_err("remote URL should not be accepted yet");

        assert!(error.to_string().contains("data URLs"));
    }

    #[test]
    fn preserves_single_text_context_as_string_content() {
        let content = content_from_parts(&[ProviderContentPart::text("previous")])
            .expect("text content should build");

        let value = serde_json::to_value(content).expect("content should serialize");
        assert_eq!(value, serde_json::json!("previous"));
    }
}
