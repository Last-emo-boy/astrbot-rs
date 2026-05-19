use astrbot_core::Result;
use astrbot_provider::{EmbeddingProvider, EmbeddingRequest};
use serde::{Deserialize, Serialize};

use crate::types::KnowledgeChunk;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedKnowledgeChunk {
    pub chunk: KnowledgeChunk,
    pub embedding: Vec<f32>,
}

impl EmbeddedKnowledgeChunk {
    pub fn new(chunk: KnowledgeChunk, embedding: Vec<f32>) -> Self {
        Self { chunk, embedding }
    }
}

pub async fn embed_chunks(
    provider: &dyn EmbeddingProvider,
    chunks: Vec<KnowledgeChunk>,
    provider_id: Option<String>,
    model: Option<String>,
) -> Result<Vec<EmbeddedKnowledgeChunk>> {
    let mut request = EmbeddingRequest::batch(chunks.iter().map(|chunk| chunk.content.clone()));
    if let Some(provider_id) = provider_id {
        request = request.with_provider_id(provider_id);
    }
    if let Some(model) = model {
        request = request.with_model(model);
    }

    let response = provider.embed(request).await?;
    if response.embeddings.len() != chunks.len() {
        return Err(crate::types::kb_error(format!(
            "embedding count mismatch: expected {}, got {}",
            chunks.len(),
            response.embeddings.len()
        )));
    }

    Ok(chunks
        .into_iter()
        .zip(response.embeddings)
        .map(|(chunk, embedding)| EmbeddedKnowledgeChunk::new(chunk, embedding))
        .collect())
}
