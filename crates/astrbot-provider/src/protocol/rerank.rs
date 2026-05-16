use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{RerankDocumentScore, RerankRequest};

const ERROR_TEXT_MAX_CHARS: usize = 4096;
const MAX_BAILIAN_DOCUMENTS: usize = 500;

#[derive(Debug, Serialize)]
pub(crate) struct BailianRerankRequest {
    model: String,
    input: BailianRerankInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<BailianRerankParameters>,
}

#[derive(Debug, Serialize)]
struct BailianRerankInput {
    query: String,
    documents: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BailianRerankParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    top_n: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_documents: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instruct: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BailianRerankResponse {
    code: Option<String>,
    message: Option<String>,
    output: Option<BailianRerankOutput>,
}

#[derive(Debug, Deserialize)]
struct BailianRerankOutput {
    #[serde(default)]
    results: Vec<BailianRerankResult>,
}

#[derive(Debug, Deserialize)]
struct BailianRerankResult {
    index: Option<usize>,
    relevance_score: Option<f32>,
}

pub(crate) fn build_bailian_rerank_request(
    request: &RerankRequest,
    default_model: &str,
    return_documents: bool,
    instruct: Option<&str>,
) -> Result<BailianRerankRequest> {
    if request.documents.is_empty() {
        return Err(AstrbotError::Provider(
            "rerank request must contain at least one document".to_string(),
        ));
    }

    let model = request
        .model
        .clone()
        .unwrap_or_else(|| default_model.to_string());
    let documents = request
        .documents
        .iter()
        .take(MAX_BAILIAN_DOCUMENTS)
        .cloned()
        .collect::<Vec<_>>();
    let parameters = bailian_parameters(&model, request.top_n, return_documents, instruct);

    Ok(BailianRerankRequest {
        model,
        input: BailianRerankInput {
            query: request.query.clone(),
            documents,
        },
        parameters,
    })
}

pub(crate) fn parse_bailian_rerank_response(body: &str) -> Result<Vec<RerankDocumentScore>> {
    let payload: BailianRerankResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;

    if payload.code.as_deref().is_some_and(|code| code != "200") {
        return Err(AstrbotError::Provider(format!(
            "Bailian rerank provider returned code {}: {}",
            payload.code.unwrap_or_default(),
            payload.message.unwrap_or_default()
        )));
    }

    Ok(payload
        .output
        .map(|output| output.results)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(position, result)| {
            RerankDocumentScore::new(
                result.index.unwrap_or(position),
                result.relevance_score.unwrap_or(0.0),
            )
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub(crate) struct VllmRerankRequest {
    query: String,
    documents: Vec<String>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_n: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct VllmRerankResponse {
    #[serde(default)]
    results: Vec<VllmRerankResult>,
}

#[derive(Debug, Deserialize)]
struct VllmRerankResult {
    index: usize,
    relevance_score: f32,
}

pub(crate) fn build_vllm_rerank_request(
    request: &RerankRequest,
    default_model: &str,
) -> Result<VllmRerankRequest> {
    if request.documents.is_empty() {
        return Err(AstrbotError::Provider(
            "rerank request must contain at least one document".to_string(),
        ));
    }

    Ok(VllmRerankRequest {
        query: request.query.clone(),
        documents: request.documents.clone(),
        model: request
            .model
            .clone()
            .unwrap_or_else(|| default_model.to_string()),
        top_n: request.top_n,
    })
}

pub(crate) fn parse_vllm_rerank_response(body: &str) -> Result<Vec<RerankDocumentScore>> {
    let payload: VllmRerankResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;

    Ok(payload
        .results
        .into_iter()
        .map(|result| RerankDocumentScore::new(result.index, result.relevance_score))
        .collect())
}

pub(crate) fn extract_bailian_rerank_error_message(body: &str) -> String {
    extract_error_message(body)
}

fn bailian_parameters(
    model: &str,
    top_n: Option<usize>,
    return_documents: bool,
    instruct: Option<&str>,
) -> Option<BailianRerankParameters> {
    let top_n = top_n.filter(|top_n| *top_n > 0);
    let return_documents = return_documents.then_some(true);
    let instruct = instruct
        .filter(|instruct| !instruct.trim().is_empty() && model == "qwen3-rerank")
        .map(str::to_string);

    if top_n.is_none() && return_documents.is_none() && instruct.is_none() {
        return None;
    }

    Some(BailianRerankParameters {
        top_n,
        return_documents,
        instruct,
    })
}

fn extract_error_message(body: &str) -> String {
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

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use crate::RerankRequest;

    use super::{
        build_bailian_rerank_request, build_vllm_rerank_request, parse_bailian_rerank_response,
        parse_vllm_rerank_response,
    };

    #[test]
    fn rerank_request_builders_match_provider_wire_shapes() {
        let request =
            RerankRequest::new("Apple", ["apple document", "banana document"]).with_top_n(2);

        let bailian = build_bailian_rerank_request(
            &request,
            "qwen3-rerank",
            true,
            Some("Rank by semantic relevance."),
        )
        .expect("Bailian request should build");
        let vllm = build_vllm_rerank_request(&request, "BAAI/bge-reranker-base")
            .expect("VLLM request should build");

        assert_eq!(
            serde_json::to_value(bailian).expect("request should serialize"),
            serde_json::json!({
                "model":"qwen3-rerank",
                "input":{"query":"Apple","documents":["apple document","banana document"]},
                "parameters":{"top_n":2,"return_documents":true,"instruct":"Rank by semantic relevance."}
            })
        );
        assert_eq!(
            serde_json::to_value(vllm).expect("request should serialize"),
            serde_json::json!({
                "query":"Apple",
                "documents":["apple document","banana document"],
                "model":"BAAI/bge-reranker-base",
                "top_n":2
            })
        );
    }

    #[test]
    fn rerank_response_parsers_extract_scores() {
        let bailian = parse_bailian_rerank_response(
            r#"{"output":{"results":[{"index":1,"relevance_score":0.92},{"relevance_score":0.25}]}}"#,
        )
        .expect("Bailian response should parse");
        let vllm = parse_vllm_rerank_response(r#"{"results":[{"index":0,"relevance_score":0.4}]}"#)
            .expect("VLLM response should parse");

        assert_eq!(bailian[0].index, 1);
        assert_eq!(bailian[1].index, 1);
        assert_eq!(vllm[0].relevance_score, 0.4);
    }
}
