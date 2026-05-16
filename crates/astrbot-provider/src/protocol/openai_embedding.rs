use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};

use crate::EmbeddingRequest;

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiEmbeddingRequest {
    model: String,
    input: OpenAiEmbeddingInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenAiEmbeddingInput {
    Text(String),
    Batch(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
}

pub(crate) fn build_openai_embedding_request(
    request: &EmbeddingRequest,
    default_model: &str,
    dimensions: Option<usize>,
) -> Result<OpenAiEmbeddingRequest> {
    if request.texts.is_empty() {
        return Err(AstrbotError::Provider(
            "embedding request must contain at least one text".to_string(),
        ));
    }

    let input = if let [text] = request.texts.as_slice() {
        OpenAiEmbeddingInput::Text(text.clone())
    } else {
        OpenAiEmbeddingInput::Batch(request.texts.clone())
    };

    Ok(OpenAiEmbeddingRequest {
        model: request
            .model
            .clone()
            .unwrap_or_else(|| default_model.to_string()),
        input,
        dimensions,
    })
}

pub(crate) fn parse_openai_embedding_response(body: &str) -> Result<Vec<Vec<f32>>> {
    let mut payload: OpenAiEmbeddingResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    payload.data.sort_by_key(|item| item.index);
    let embeddings = payload
        .data
        .into_iter()
        .map(|item| item.embedding)
        .collect::<Vec<_>>();

    if embeddings.is_empty() {
        return Err(AstrbotError::Provider(
            "provider response did not contain embeddings".to_string(),
        ));
    }

    Ok(embeddings)
}

#[cfg(test)]
mod tests {
    use crate::EmbeddingRequest;

    use super::{build_openai_embedding_request, parse_openai_embedding_response};

    #[test]
    fn request_uses_string_for_single_input_and_array_for_batch() {
        let single =
            build_openai_embedding_request(&EmbeddingRequest::new("hello"), "embed", Some(2))
                .expect("single request should build");
        let batch = build_openai_embedding_request(
            &EmbeddingRequest::batch(["first", "second"]),
            "embed",
            Some(2),
        )
        .expect("batch request should build");

        assert_eq!(
            serde_json::to_value(single).expect("request should serialize"),
            serde_json::json!({"model":"embed","input":"hello","dimensions":2})
        );
        assert_eq!(
            serde_json::to_value(batch).expect("request should serialize"),
            serde_json::json!({"model":"embed","input":["first","second"],"dimensions":2})
        );
    }

    #[test]
    fn response_parser_orders_embeddings_by_index() {
        let embeddings = parse_openai_embedding_response(
            r#"{"data":[{"index":1,"embedding":[0.3]},{"index":0,"embedding":[0.1]}]}"#,
        )
        .expect("response should parse");

        assert_eq!(embeddings, vec![vec![0.1], vec![0.3]]);
    }
}
