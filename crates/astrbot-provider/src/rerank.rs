use std::cmp::Ordering;

use astrbot_core::Result;
use async_trait::async_trait;

use crate::chat::non_empty_option;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RerankRequest {
    pub provider_id: Option<String>,
    pub query: String,
    pub documents: Vec<String>,
    pub top_n: Option<usize>,
    pub model: Option<String>,
}

impl RerankRequest {
    pub fn new<I, S>(query: impl Into<String>, documents: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            provider_id: None,
            query: query.into(),
            documents: documents.into_iter().map(Into::into).collect(),
            top_n: None,
            model: None,
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }

    pub fn with_top_n(mut self, top_n: usize) -> Self {
        self.top_n = Some(top_n);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = non_empty_option(model);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RerankDocumentScore {
    pub index: usize,
    pub relevance_score: f32,
}

impl RerankDocumentScore {
    pub fn new(index: usize, relevance_score: f32) -> Self {
        Self {
            index,
            relevance_score,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RerankResponse {
    pub results: Vec<RerankDocumentScore>,
}

impl RerankResponse {
    pub fn new(results: Vec<RerankDocumentScore>) -> Self {
        Self { results }
    }
}

#[async_trait]
pub trait RerankProvider: Send + Sync {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MockRerankProvider {
    scores: Vec<f32>,
}

impl MockRerankProvider {
    pub fn new(scores: Vec<f32>) -> Self {
        Self { scores }
    }
}

#[async_trait]
impl RerankProvider for MockRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        if request.documents.is_empty() {
            return Err(astrbot_core::AstrbotError::Provider(
                "rerank request must contain at least one document".to_string(),
            ));
        }

        let mut results = request
            .documents
            .iter()
            .enumerate()
            .map(|(index, _)| {
                RerankDocumentScore::new(index, self.scores.get(index).copied().unwrap_or(0.0))
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .relevance_score
                .partial_cmp(&left.relevance_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.index.cmp(&right.index))
        });
        if let Some(top_n) = request.top_n {
            results.truncate(top_n);
        }

        Ok(RerankResponse::new(results))
    }
}
