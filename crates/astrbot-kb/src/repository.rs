use std::sync::{Arc, RwLock};

use astrbot_core::Result;
use async_trait::async_trait;

use crate::document::{KnowledgeBaseProfile, KnowledgeDocument, KnowledgeMedia};
use crate::types::{DocumentId, KnowledgeBaseId, kb_error};

#[async_trait]
pub trait KnowledgeDocumentRepository: Send + Sync {
    async fn get_kb(&self, kb_id: &KnowledgeBaseId) -> Result<Option<KnowledgeBaseProfile>>;

    async fn upsert_kb(&self, profile: KnowledgeBaseProfile) -> Result<()>;

    async fn upsert_document(&self, document: KnowledgeDocument) -> Result<()>;

    async fn upsert_media(&self, media: KnowledgeMedia) -> Result<()>;

    async fn list_documents(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeDocument>>;

    async fn list_media(&self, doc_id: &DocumentId) -> Result<Vec<KnowledgeMedia>>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryKnowledgeDocumentRepository {
    profiles: Arc<RwLock<Vec<KnowledgeBaseProfile>>>,
    documents: Arc<RwLock<Vec<KnowledgeDocument>>>,
    media: Arc<RwLock<Vec<KnowledgeMedia>>>,
}

impl InMemoryKnowledgeDocumentRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KnowledgeDocumentRepository for InMemoryKnowledgeDocumentRepository {
    async fn get_kb(&self, kb_id: &KnowledgeBaseId) -> Result<Option<KnowledgeBaseProfile>> {
        let profiles = self
            .profiles
            .read()
            .map_err(|_| kb_error("knowledge profile repository lock poisoned"))?;
        Ok(profiles
            .iter()
            .find(|profile| &profile.kb_id == kb_id)
            .cloned())
    }

    async fn upsert_kb(&self, profile: KnowledgeBaseProfile) -> Result<()> {
        let mut profiles = self
            .profiles
            .write()
            .map_err(|_| kb_error("knowledge profile repository lock poisoned"))?;
        if let Some(existing) = profiles
            .iter_mut()
            .find(|existing| existing.kb_id == profile.kb_id)
        {
            *existing = profile;
        } else {
            profiles.push(profile);
        }
        Ok(())
    }

    async fn upsert_document(&self, document: KnowledgeDocument) -> Result<()> {
        let mut documents = self
            .documents
            .write()
            .map_err(|_| kb_error("knowledge document repository lock poisoned"))?;
        if let Some(existing) = documents
            .iter_mut()
            .find(|existing| existing.doc_id == document.doc_id)
        {
            *existing = document;
        } else {
            documents.push(document);
        }
        Ok(())
    }

    async fn upsert_media(&self, media: KnowledgeMedia) -> Result<()> {
        let mut records = self
            .media
            .write()
            .map_err(|_| kb_error("knowledge media repository lock poisoned"))?;
        if let Some(existing) = records
            .iter_mut()
            .find(|existing| existing.media_id == media.media_id)
        {
            *existing = media;
        } else {
            records.push(media);
        }
        Ok(())
    }

    async fn list_documents(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeDocument>> {
        let documents = self
            .documents
            .read()
            .map_err(|_| kb_error("knowledge document repository lock poisoned"))?;
        Ok(documents
            .iter()
            .filter(|document| &document.kb_id == kb_id)
            .cloned()
            .collect())
    }

    async fn list_media(&self, doc_id: &DocumentId) -> Result<Vec<KnowledgeMedia>> {
        let media = self
            .media
            .read()
            .map_err(|_| kb_error("knowledge media repository lock poisoned"))?;
        Ok(media
            .iter()
            .filter(|record| &record.doc_id == doc_id)
            .cloned()
            .collect())
    }
}
