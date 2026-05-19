//! Qdrant HTTP client primitives for the KB vector backend.
//!
//! This module exposes the smallest surface needed to upsert + search
//! against a Qdrant cluster from the host. It does **not** implement
//! [`crate::vector_store::VectorStore`] directly — that requires the
//! caller to map plugin-specific knowledge IDs into Qdrant points, which
//! is orchestration-specific. The client below covers the wire format so
//! integrators can build whatever wrapper they need.
//!
//! Reference: https://qdrant.tech/documentation/concepts/points/

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Vector distance metric supported by Qdrant. Adding a variant means
/// adding to `as_qdrant_distance()` below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QdrantDistance {
    Cosine,
    Dot,
    Euclid,
}

impl QdrantDistance {
    pub fn as_qdrant_distance(self) -> &'static str {
        match self {
            QdrantDistance::Cosine => "Cosine",
            QdrantDistance::Dot => "Dot",
            QdrantDistance::Euclid => "Euclid",
        }
    }
}

/// HTTP client for the Qdrant REST API.
#[derive(Clone, Debug)]
pub struct QdrantClient {
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl QdrantClient {
    /// Build a client pointing at a Qdrant instance.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim().trim_end_matches('/').to_string(),
            api_key: None,
            client: reqwest::Client::new(),
        }
    }

    /// Provide the API key Qdrant Cloud and protected self-hosted instances
    /// require. Sent as the `api-key` header on every request.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        let key = api_key.into();
        self.api_key = (!key.trim().is_empty()).then_some(key);
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// `PUT /collections/{name}` — creates a collection if it does not yet
    /// exist. Idempotent (re-issuing returns 200).
    pub async fn ensure_collection(
        &self,
        name: &str,
        vector_size: u32,
        distance: QdrantDistance,
    ) -> Result<()> {
        let body = json!({
            "vectors": {
                "size": vector_size,
                "distance": distance.as_qdrant_distance(),
            },
        });
        let response = self
            .request(reqwest::Method::PUT, &format!("/collections/{name}"), Some(body))
            .await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AstrbotError::Pipeline(format!(
                "Qdrant ensure_collection returned {status}: {text}"
            )));
        }
        Ok(())
    }

    /// `PUT /collections/{name}/points` — upsert one or more
    /// [`QdrantPoint`]s. The `wait=true` query param ensures the response
    /// only returns after the points are durable.
    pub async fn upsert_points(&self, collection: &str, points: &[QdrantPoint]) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }
        let body = json!({
            "points": points,
        });
        let response = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{collection}/points?wait=true"),
                Some(body),
            )
            .await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AstrbotError::Pipeline(format!(
                "Qdrant upsert_points returned {status}: {text}"
            )));
        }
        Ok(())
    }

    /// `POST /collections/{name}/points/search` — k-NN similarity search.
    pub async fn search(
        &self,
        collection: &str,
        query: &QdrantSearchRequest,
    ) -> Result<Vec<QdrantSearchHit>> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{collection}/points/search"),
                Some(serde_json::to_value(query).map_err(|err| {
                    AstrbotError::Pipeline(format!("Qdrant search serialise: {err}"))
                })?),
            )
            .await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AstrbotError::Pipeline(format!(
                "Qdrant search returned {status}: {body}"
            )));
        }
        parse_search_response(&body)
    }

    /// `POST /collections/{name}/points/delete` — delete by point IDs.
    pub async fn delete_points(&self, collection: &str, point_ids: &[String]) -> Result<()> {
        if point_ids.is_empty() {
            return Ok(());
        }
        let body = json!({ "points": point_ids });
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{collection}/points/delete?wait=true"),
                Some(body),
            )
            .await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AstrbotError::Pipeline(format!(
                "Qdrant delete_points returned {status}: {text}"
            )));
        }
        Ok(())
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<reqwest::Response> {
        let mut builder = self.client.request(method, self.endpoint(path));
        if let Some(api_key) = &self.api_key {
            builder = builder.header("api-key", api_key);
        }
        if let Some(body) = body {
            builder = builder.json(&body);
        }
        builder
            .send()
            .await
            .map_err(|err| AstrbotError::Pipeline(format!("Qdrant HTTP failed: {err}")))
    }
}

/// One point: `(id, vector, payload)`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QdrantPoint {
    pub id: String,
    pub vector: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

impl QdrantPoint {
    pub fn new(id: impl Into<String>, vector: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            vector,
            payload: None,
        }
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = Some(payload);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QdrantSearchRequest {
    pub vector: Vec<f32>,
    pub limit: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Value>,
    #[serde(default = "default_true")]
    pub with_payload: bool,
}

fn default_true() -> bool {
    true
}

impl QdrantSearchRequest {
    pub fn new(vector: Vec<f32>, limit: u32) -> Self {
        Self {
            vector,
            limit,
            filter: None,
            with_payload: true,
        }
    }

    /// Filter on a `kb_id`-like payload field. Mirrors Qdrant's `must` clause
    /// shape:
    ///
    /// ```json
    /// { "must": [ { "key": "kb_id", "match": { "value": "X" } } ] }
    /// ```
    pub fn with_kb_filter(mut self, kb_ids: &[String]) -> Self {
        if kb_ids.is_empty() {
            return self;
        }
        let any_of: Vec<Value> = kb_ids
            .iter()
            .map(|id| json!({ "key": "kb_id", "match": { "value": id } }))
            .collect();
        self.filter = Some(json!({ "should": any_of }));
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QdrantSearchHit {
    pub id: String,
    pub score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

fn parse_search_response(body: &str) -> Result<Vec<QdrantSearchHit>> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| AstrbotError::Pipeline(format!("Qdrant response not JSON: {err}")))?;
    let result = value
        .get("result")
        .and_then(Value::as_array)
        .ok_or_else(|| AstrbotError::Pipeline("Qdrant response missing result array".into()))?;
    let mut hits = Vec::new();
    for item in result {
        let id = item
            .get("id")
            .map(|value| match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
            .unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let score = item
            .get("score")
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .unwrap_or(0.0);
        let payload = item.get("payload").cloned();
        hits.push(QdrantSearchHit { id, score, payload });
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_strips_trailing_slash() {
        let client = QdrantClient::new("http://localhost:6333/");
        assert_eq!(client.base_url(), "http://localhost:6333");
        assert_eq!(
            client.endpoint("/collections/foo"),
            "http://localhost:6333/collections/foo"
        );
    }

    #[test]
    fn search_request_serialises_kb_filter() {
        let request = QdrantSearchRequest::new(vec![0.1, 0.2], 5)
            .with_kb_filter(&["kb-a".into(), "kb-b".into()]);
        let json = serde_json::to_value(&request).unwrap();
        let should = json
            .get("filter")
            .and_then(|v| v.get("should"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(should.len(), 2);
        assert_eq!(should[0]["key"], "kb_id");
        assert_eq!(should[0]["match"]["value"], "kb-a");
    }

    #[test]
    fn search_request_without_filter_omits_filter_field() {
        let request = QdrantSearchRequest::new(vec![0.1], 5);
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("filter").is_none());
    }

    #[test]
    fn parse_search_response_extracts_hits() {
        let body = r#"{
            "result": [
                { "id": "chunk-1", "score": 0.92, "payload": { "kb_id": "kb-a" } },
                { "id": 42, "score": 0.7 }
            ]
        }"#;
        let hits = parse_search_response(body).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "chunk-1");
        assert!(hits[0].score > 0.9);
        assert_eq!(hits[1].id, "42");
    }

    #[test]
    fn parse_search_response_missing_result_errors() {
        let body = r#"{ "status": "ok" }"#;
        assert!(parse_search_response(body).is_err());
    }

    #[test]
    fn distance_serialises_to_qdrant_value() {
        assert_eq!(QdrantDistance::Cosine.as_qdrant_distance(), "Cosine");
        assert_eq!(QdrantDistance::Dot.as_qdrant_distance(), "Dot");
        assert_eq!(QdrantDistance::Euclid.as_qdrant_distance(), "Euclid");
    }
}
