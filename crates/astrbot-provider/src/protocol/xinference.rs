use std::collections::HashMap;

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};

use crate::RerankDocumentScore;

#[derive(Debug, Serialize)]
pub(crate) struct XinferenceLaunchModelRequest {
    pub model_name: String,
    pub model_type: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct XinferenceRerankRequest {
    pub model: String,
    pub documents: Vec<String>,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_n: Option<usize>,
}

pub(crate) fn parse_running_model_uid(body: &str, requested_model: &str) -> Result<Option<String>> {
    let payload: XinferenceListModelsResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;

    Ok(payload
        .into_models()
        .into_iter()
        .find(|model| {
            model.id == requested_model
                || model
                    .model_name
                    .as_deref()
                    .is_some_and(|name| name == requested_model)
        })
        .map(|model| model.id))
}

pub(crate) fn parse_launch_model_uid(body: &str) -> Result<String> {
    let payload: XinferenceLaunchModelResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    let Some(model_uid) = payload.into_model_uid() else {
        return Err(AstrbotError::Provider(
            "Xinference launch model response did not contain model_uid".to_string(),
        ));
    };

    Ok(model_uid)
}

pub(crate) fn parse_xinference_stt_text(body: &str) -> Result<String> {
    let payload: XinferenceSpeechToTextResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    if payload.text.trim().is_empty() {
        return Err(AstrbotError::Provider(
            "provider response did not contain transcription text".to_string(),
        ));
    }

    Ok(payload.text)
}

pub(crate) fn parse_xinference_rerank_response(body: &str) -> Result<Vec<RerankDocumentScore>> {
    let payload: XinferenceRerankResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;

    Ok(payload
        .results
        .into_iter()
        .map(|result| RerankDocumentScore::new(result.index, result.relevance_score))
        .collect())
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum XinferenceListModelsResponse {
    UidMap(HashMap<String, XinferenceModelSpec>),
    Data {
        #[serde(default)]
        data: Vec<XinferenceModelSpec>,
    },
}

impl XinferenceListModelsResponse {
    fn into_models(self) -> Vec<XinferenceModelSpec> {
        match self {
            Self::Data { data } => data,
            Self::UidMap(models) => models
                .into_iter()
                .map(|(uid, mut model)| {
                    if model.id.trim().is_empty() {
                        model.id = uid;
                    }
                    model
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct XinferenceModelSpec {
    #[serde(default)]
    id: String,
    model_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum XinferenceLaunchModelResponse {
    Object {
        model_uid: Option<String>,
        id: Option<String>,
    },
    String(String),
}

impl XinferenceLaunchModelResponse {
    fn into_model_uid(self) -> Option<String> {
        match self {
            Self::Object { model_uid, id } => model_uid.or(id),
            Self::String(model_uid) => Some(model_uid),
        }
        .map(|model_uid| model_uid.trim().to_string())
        .filter(|model_uid| !model_uid.is_empty())
    }
}

#[derive(Debug, Deserialize)]
struct XinferenceSpeechToTextResponse {
    text: String,
}

#[derive(Debug, Deserialize)]
struct XinferenceRerankResponse {
    #[serde(default)]
    results: Vec<XinferenceRerankResult>,
}

#[derive(Debug, Deserialize)]
struct XinferenceRerankResult {
    index: usize,
    relevance_score: f32,
}
