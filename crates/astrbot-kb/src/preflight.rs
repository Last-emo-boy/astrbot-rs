use std::sync::Arc;

use astrbot_core::Result;
use astrbot_provider::{EmbeddingProvider, EmbeddingRequest, RerankProvider, RerankRequest};
use serde::{Deserialize, Serialize};

use crate::types::kb_error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeProviderPreflightRequest {
    pub embedding_provider_id: String,
    pub expected_embedding_dimension: Option<usize>,
    pub rerank_provider_id: Option<String>,
}

impl KnowledgeProviderPreflightRequest {
    pub fn new(embedding_provider_id: impl Into<String>) -> Self {
        Self {
            embedding_provider_id: embedding_provider_id.into(),
            expected_embedding_dimension: None,
            rerank_provider_id: None,
        }
    }

    pub fn with_expected_embedding_dimension(mut self, dimension: usize) -> Self {
        self.expected_embedding_dimension = Some(dimension);
        self
    }

    pub fn with_rerank_provider_id(mut self, rerank_provider_id: Option<String>) -> Self {
        self.rerank_provider_id = rerank_provider_id.and_then(|id| {
            let id = id.trim().to_string();
            (!id.is_empty()).then_some(id)
        });
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeEmbeddingPreflight {
    pub provider_id: String,
    pub available: bool,
    pub expected_dimension: Option<usize>,
    pub actual_dimension: Option<usize>,
    pub dimension_matches: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRerankPreflight {
    pub provider_id: String,
    pub available: bool,
    pub smoke_test_passed: bool,
    pub result_count: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeProviderPreflightReport {
    pub embedding: KnowledgeEmbeddingPreflight,
    pub rerank: Option<KnowledgeRerankPreflight>,
}

impl KnowledgeProviderPreflightReport {
    pub fn is_usable(&self) -> bool {
        self.embedding.available
            && self.embedding.dimension_matches
            && self
                .rerank
                .as_ref()
                .is_none_or(|rerank| rerank.available && rerank.smoke_test_passed)
    }
}

#[derive(Clone)]
pub struct KnowledgeProviderPreflightService {
    embedding_provider: Arc<dyn EmbeddingProvider>,
    rerank_provider: Option<Arc<dyn RerankProvider>>,
}

impl std::fmt::Debug for KnowledgeProviderPreflightService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KnowledgeProviderPreflightService")
            .field("has_rerank_provider", &self.rerank_provider.is_some())
            .finish_non_exhaustive()
    }
}

impl KnowledgeProviderPreflightService {
    pub fn new(
        embedding_provider: Arc<dyn EmbeddingProvider>,
        rerank_provider: Option<Arc<dyn RerankProvider>>,
    ) -> Self {
        Self {
            embedding_provider,
            rerank_provider,
        }
    }

    pub async fn preflight(
        &self,
        request: KnowledgeProviderPreflightRequest,
    ) -> Result<KnowledgeProviderPreflightReport> {
        let embedding_provider_id = normalize_required_id(
            request.embedding_provider_id,
            "embedding provider id cannot be empty",
        )?;
        let embedding = self
            .check_embedding(embedding_provider_id, request.expected_embedding_dimension)
            .await;
        let rerank = match request.rerank_provider_id {
            Some(rerank_provider_id) => Some(self.check_rerank(rerank_provider_id).await),
            None => None,
        };

        Ok(KnowledgeProviderPreflightReport { embedding, rerank })
    }

    async fn check_embedding(
        &self,
        provider_id: String,
        expected_dimension: Option<usize>,
    ) -> KnowledgeEmbeddingPreflight {
        let response = self
            .embedding_provider
            .embed(EmbeddingRequest::new("astrbot").with_provider_id(provider_id.clone()))
            .await;
        match response {
            Ok(response) => {
                let actual_dimension = response
                    .dimension()
                    .or_else(|| self.embedding_provider.dimensions());
                let dimension_matches = expected_dimension.is_none_or(|expected| {
                    actual_dimension.is_some_and(|actual| actual == expected)
                });
                KnowledgeEmbeddingPreflight {
                    provider_id,
                    available: true,
                    expected_dimension,
                    actual_dimension,
                    dimension_matches,
                    error: None,
                }
            }
            Err(error) => KnowledgeEmbeddingPreflight {
                provider_id,
                available: false,
                expected_dimension,
                actual_dimension: None,
                dimension_matches: false,
                error: Some(error.to_string()),
            },
        }
    }

    async fn check_rerank(&self, provider_id: String) -> KnowledgeRerankPreflight {
        let Some(rerank_provider) = &self.rerank_provider else {
            return KnowledgeRerankPreflight {
                provider_id,
                available: false,
                smoke_test_passed: false,
                result_count: 0,
                error: Some("rerank provider manager is not configured".to_string()),
            };
        };
        let response = rerank_provider
            .rerank(
                RerankRequest::new("astrbot", ["astrbot knowledge base"])
                    .with_provider_id(provider_id.clone())
                    .with_top_n(1),
            )
            .await;
        match response {
            Ok(response) => KnowledgeRerankPreflight {
                provider_id,
                available: true,
                smoke_test_passed: !response.results.is_empty(),
                result_count: response.results.len(),
                error: None,
            },
            Err(error) => KnowledgeRerankPreflight {
                provider_id,
                available: false,
                smoke_test_passed: false,
                result_count: 0,
                error: Some(error.to_string()),
            },
        }
    }
}

fn normalize_required_id(value: String, error: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(kb_error(error))
    } else {
        Ok(value)
    }
}
