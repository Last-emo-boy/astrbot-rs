use astrbot_core::{AstrbotError, ProviderContentPart, Result};
use serde::Serialize;
use serde_json::Value;

use crate::ChatRequest;

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

pub(crate) fn extract_anthropic_message_content(body: &str) -> Result<String> {
    let payload: Value = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    Ok(payload
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
        .unwrap_or_default())
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
