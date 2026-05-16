use astrbot_core::Result;
use async_trait::async_trait;

use crate::chat::non_empty_option;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddingRequest {
    pub provider_id: Option<String>,
    pub texts: Vec<String>,
    pub model: Option<String>,
}

impl EmbeddingRequest {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            provider_id: None,
            texts: vec![text.into()],
            model: None,
        }
    }

    pub fn batch<I, S>(texts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            provider_id: None,
            texts: texts.into_iter().map(Into::into).collect(),
            model: None,
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = non_empty_option(model);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
}

impl EmbeddingResponse {
    pub fn new(embeddings: Vec<Vec<f32>>) -> Self {
        Self { embeddings }
    }

    pub fn dimension(&self) -> Option<usize> {
        self.embeddings.first().map(Vec::len)
    }
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse>;

    fn dimensions(&self) -> Option<usize> {
        None
    }

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MockEmbeddingProvider {
    embedding: Vec<f32>,
}

impl MockEmbeddingProvider {
    pub fn new(embedding: Vec<f32>) -> Self {
        let embedding = if embedding.is_empty() {
            vec![0.0]
        } else {
            embedding
        };
        Self { embedding }
    }
}

#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        if request.texts.is_empty() {
            return Err(astrbot_core::AstrbotError::Provider(
                "embedding request must contain at least one text".to_string(),
            ));
        }

        Ok(EmbeddingResponse::new(
            request
                .texts
                .iter()
                .map(|_| self.embedding.clone())
                .collect(),
        ))
    }

    fn dimensions(&self) -> Option<usize> {
        Some(self.embedding.len())
    }
}
