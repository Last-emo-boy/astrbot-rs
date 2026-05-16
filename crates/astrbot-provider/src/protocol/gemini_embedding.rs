use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ERROR_TEXT_MAX_CHARS: usize = 4096;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiEmbedContentRequest {
    model: String,
    content: GeminiContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_dimensionality: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GeminiBatchEmbedContentsRequest {
    requests: Vec<GeminiEmbedContentRequest>,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiEmbedContentResponse {
    embedding: GeminiContentEmbedding,
}

#[derive(Debug, Deserialize)]
struct GeminiBatchEmbedContentsResponse {
    embeddings: Vec<GeminiContentEmbedding>,
}

#[derive(Debug, Deserialize)]
struct GeminiContentEmbedding {
    values: Vec<f32>,
}

pub(crate) fn gemini_embedding_model_resource(model: &str) -> String {
    if model.starts_with("models/") {
        model.to_string()
    } else {
        format!("models/{model}")
    }
}

pub(crate) fn gemini_embedding_method_url(api_base: &str, model: &str, method: &str) -> String {
    let api_base = api_base.trim_end_matches('/');
    let model = gemini_embedding_model_resource(model);
    if api_base.ends_with("/v1beta") {
        format!("{api_base}/{model}:{method}")
    } else {
        format!("{api_base}/v1beta/{model}:{method}")
    }
}

pub(crate) fn build_gemini_embed_content_request(
    model: &str,
    text: &str,
    dimensions: Option<usize>,
) -> GeminiEmbedContentRequest {
    GeminiEmbedContentRequest {
        model: gemini_embedding_model_resource(model),
        content: gemini_text_content(text),
        output_dimensionality: dimensions,
    }
}

pub(crate) fn build_gemini_batch_embed_contents_request(
    model: &str,
    texts: &[String],
    dimensions: Option<usize>,
) -> GeminiBatchEmbedContentsRequest {
    GeminiBatchEmbedContentsRequest {
        requests: texts
            .iter()
            .map(|text| build_gemini_embed_content_request(model, text, dimensions))
            .collect(),
    }
}

pub(crate) fn parse_gemini_embed_content_response(body: &str) -> Result<Vec<f32>> {
    let payload: GeminiEmbedContentResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;

    if payload.embedding.values.is_empty() {
        return Err(AstrbotError::Provider(
            "provider response did not contain embedding values".to_string(),
        ));
    }

    Ok(payload.embedding.values)
}

pub(crate) fn parse_gemini_batch_embed_contents_response(body: &str) -> Result<Vec<Vec<f32>>> {
    let payload: GeminiBatchEmbedContentsResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    let embeddings = payload
        .embeddings
        .into_iter()
        .map(|embedding| embedding.values)
        .collect::<Vec<_>>();

    if embeddings.is_empty() {
        return Err(AstrbotError::Provider(
            "provider response did not contain embeddings".to_string(),
        ));
    }

    Ok(embeddings)
}

pub(crate) fn extract_gemini_embedding_error_message(body: &str) -> String {
    let fallback = truncate(body.trim());
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return fallback;
    };

    let extracted = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string);

    extracted.unwrap_or(fallback)
}

fn gemini_text_content(text: &str) -> GeminiContent {
    GeminiContent {
        parts: vec![GeminiPart {
            text: text.to_string(),
        }],
    }
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        build_gemini_batch_embed_contents_request, build_gemini_embed_content_request,
        gemini_embedding_method_url, parse_gemini_batch_embed_contents_response,
        parse_gemini_embed_content_response,
    };

    #[test]
    fn request_builders_match_gemini_embedding_wire_shape() {
        let single = build_gemini_embed_content_request("gemini-embedding-001", "hello", Some(3));
        let batch = build_gemini_batch_embed_contents_request(
            "models/custom",
            &["first".to_string(), "second".to_string()],
            Some(2),
        );

        assert_eq!(
            serde_json::to_value(single).expect("request should serialize"),
            serde_json::json!({
                "model":"models/gemini-embedding-001",
                "content":{"parts":[{"text":"hello"}]},
                "outputDimensionality":3
            })
        );
        assert_eq!(
            serde_json::to_value(batch).expect("request should serialize"),
            serde_json::json!({
                "requests":[
                    {"model":"models/custom","content":{"parts":[{"text":"first"}]},"outputDimensionality":2},
                    {"model":"models/custom","content":{"parts":[{"text":"second"}]},"outputDimensionality":2}
                ]
            })
        );
    }

    #[test]
    fn url_builder_preserves_v1beta_model_method_shape() {
        assert_eq!(
            gemini_embedding_method_url(
                "https://generativelanguage.googleapis.com",
                "gemini-embedding-001",
                "embedContent"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-001:embedContent"
        );
    }

    #[test]
    fn response_parsers_extract_single_and_batch_vectors() {
        assert_eq!(
            parse_gemini_embed_content_response(r#"{"embedding":{"values":[0.1,0.2]}}"#)
                .expect("single response should parse"),
            vec![0.1, 0.2]
        );
        assert_eq!(
            parse_gemini_batch_embed_contents_response(
                r#"{"embeddings":[{"values":[1.0]},{"values":[2.0]}]}"#
            )
            .expect("batch response should parse"),
            vec![vec![1.0], vec![2.0]]
        );
    }
}
