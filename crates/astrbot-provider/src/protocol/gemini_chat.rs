use astrbot_core::{AstrbotError, ProviderContentPart, Result};
use serde::Serialize;
use serde_json::Value;

use crate::ChatRequest;

#[derive(Debug, Serialize)]
pub(crate) struct GeminiGenerateContentRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
}

#[derive(Debug, Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

pub(crate) fn build_gemini_generate_content_request(
    request: &ChatRequest,
) -> Result<GeminiGenerateContentRequest> {
    let mut contents = Vec::new();
    let mut system_parts = Vec::new();

    if let Some(system_prompt) = request
        .system_prompt
        .as_deref()
        .filter(|system_prompt| !system_prompt.trim().is_empty())
    {
        system_parts.push(gemini_text_part(system_prompt));
    }

    for context in &request.contexts {
        if context.role == "system" {
            system_parts.extend(gemini_parts_from_parts(&context.parts)?);
            continue;
        }

        contents.push(GeminiContent {
            role: normalize_gemini_role(&context.role),
            parts: gemini_parts_from_parts(&context.parts)?,
        });
    }

    contents.push(GeminiContent {
        role: "user".to_string(),
        parts: user_parts(request)?,
    });

    Ok(GeminiGenerateContentRequest {
        contents,
        system_instruction: (!system_parts.is_empty()).then_some(GeminiSystemInstruction {
            parts: system_parts,
        }),
    })
}

pub(crate) fn extract_gemini_message_content(body: &str) -> Result<String> {
    let payload: Value = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    extract_message_content(&payload)
}

fn normalize_gemini_role(role: &str) -> String {
    if role == "assistant" || role == "model" {
        "model".to_string()
    } else {
        "user".to_string()
    }
}

fn user_parts(request: &ChatRequest) -> Result<Vec<GeminiPart>> {
    let image_urls = request
        .image_urls
        .iter()
        .filter(|url| !url.trim().is_empty())
        .collect::<Vec<_>>();

    if image_urls.is_empty() && request.extra_user_content_parts.is_empty() {
        return Ok(vec![gemini_text_part(&request.prompt)]);
    }

    let mut parts =
        Vec::with_capacity(image_urls.len() + request.extra_user_content_parts.len() + 1);
    if !request.prompt.trim().is_empty()
        || !image_urls.is_empty()
        || !request.extra_user_content_parts.is_empty()
    {
        parts.push(gemini_text_part(if request.prompt.trim().is_empty() {
            "[image]"
        } else {
            &request.prompt
        }));
    }

    for part in &request.extra_user_content_parts {
        push_gemini_part(&mut parts, part)?;
    }
    for image_url in image_urls {
        parts.push(image_part_from_data_url(image_url)?);
    }

    Ok(parts)
}

fn gemini_parts_from_parts(parts: &[ProviderContentPart]) -> Result<Vec<GeminiPart>> {
    let mut gemini_parts = Vec::with_capacity(parts.len());
    for part in parts {
        push_gemini_part(&mut gemini_parts, part)?;
    }
    if gemini_parts.is_empty() {
        gemini_parts.push(gemini_text_part(""));
    }
    Ok(gemini_parts)
}

fn push_gemini_part(parts: &mut Vec<GeminiPart>, part: &ProviderContentPart) -> Result<()> {
    match part {
        ProviderContentPart::Text { text } => {
            parts.push(gemini_text_part(text));
            Ok(())
        }
        ProviderContentPart::ImageUrl { url } => {
            parts.push(image_part_from_data_url(url)?);
            Ok(())
        }
    }
}

fn gemini_text_part(text: &str) -> GeminiPart {
    GeminiPart::Text {
        text: if text.is_empty() {
            " ".to_string()
        } else {
            text.to_string()
        },
    }
}

fn image_part_from_data_url(url: &str) -> Result<GeminiPart> {
    let url = url.trim();
    let Some(data_url) = url.strip_prefix("data:") else {
        return Err(AstrbotError::Provider(
            "Gemini image inputs currently require data URLs".to_string(),
        ));
    };
    let Some((metadata, data)) = data_url.split_once(',') else {
        return Err(AstrbotError::Provider(
            "invalid Gemini image data URL".to_string(),
        ));
    };
    if !metadata.split(';').any(|part| part == "base64") {
        return Err(AstrbotError::Provider(
            "Gemini image data URLs must be base64 encoded".to_string(),
        ));
    }
    let mime_type = metadata
        .split(';')
        .next()
        .filter(|mime_type| mime_type.starts_with("image/"))
        .ok_or_else(|| AstrbotError::Provider("invalid Gemini image media type".to_string()))?;

    Ok(GeminiPart::InlineData {
        inline_data: GeminiInlineData {
            mime_type: mime_type.to_string(),
            data: data.to_string(),
        },
    })
}

fn extract_message_content(payload: &Value) -> Result<String> {
    if let Some(block_reason) = payload
        .get("promptFeedback")
        .and_then(|feedback| feedback.get("blockReason"))
        .and_then(Value::as_str)
    {
        return Err(AstrbotError::Provider(format!(
            "Gemini prompt was blocked: {block_reason}"
        )));
    }

    let candidate = payload
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .ok_or_else(|| {
            AstrbotError::Provider("Gemini response contained no candidates".to_string())
        })?;

    if let Some(finish_reason) = candidate
        .get("finishReason")
        .and_then(Value::as_str)
        .filter(|finish_reason| is_blocked_finish_reason(finish_reason))
    {
        return Err(AstrbotError::Provider(format!(
            "Gemini provider blocked response: {finish_reason}"
        )));
    }

    Ok(candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default())
}

fn is_blocked_finish_reason(reason: &str) -> bool {
    matches!(
        reason,
        "SAFETY" | "PROHIBITED_CONTENT" | "SPII" | "BLOCKLIST" | "IMAGE_SAFETY"
    )
}

#[cfg(test)]
mod tests {
    use astrbot_core::ProviderContentPart;

    use super::{gemini_parts_from_parts, image_part_from_data_url};

    #[test]
    fn rejects_remote_image_urls_until_transport_download_exists() {
        let error = image_part_from_data_url("https://example.test/image.png")
            .expect_err("remote URL should not be accepted yet");

        assert!(error.to_string().contains("data URLs"));
    }

    #[test]
    fn preserves_empty_context_as_text_placeholder() {
        let parts = gemini_parts_from_parts(&[]).expect("empty parts should build");

        let value = serde_json::to_value(parts).expect("parts should serialize");
        assert_eq!(value, serde_json::json!([{"text":" "}]));
    }

    #[test]
    fn serializes_text_and_image_parts() {
        let parts = gemini_parts_from_parts(&[
            ProviderContentPart::text("look"),
            ProviderContentPart::image_url("data:image/png;base64,iVBORw0KGgo="),
        ])
        .expect("parts should build");

        let value = serde_json::to_value(parts).expect("parts should serialize");
        assert_eq!(
            value,
            serde_json::json!([
                {"text":"look"},
                {"inlineData":{"mimeType":"image/png","data":"iVBORw0KGgo="}}
            ])
        );
    }
}
