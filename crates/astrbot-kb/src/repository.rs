use std::sync::{Arc, RwLock};

use astrbot_core::Result;
use astrbot_storage::SqliteJsonStore;
use async_trait::async_trait;

use crate::document::{KnowledgeBaseProfile, KnowledgeDocument, KnowledgeMedia};
use crate::types::{DocumentId, KnowledgeBaseId, kb_error};

const KB_REPOSITORY_PROFILE_NAMESPACE: &str = "kb_repository_profiles";
const KB_REPOSITORY_DOCUMENT_NAMESPACE: &str = "kb_repository_documents";
const KB_REPOSITORY_MEDIA_NAMESPACE: &str = "kb_repository_media";

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

#[derive(Clone, Debug)]
pub struct SqliteKnowledgeDocumentRepository {
    store: SqliteJsonStore,
}

impl InMemoryKnowledgeDocumentRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SqliteKnowledgeDocumentRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
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

#[async_trait]
impl KnowledgeDocumentRepository for SqliteKnowledgeDocumentRepository {
    async fn get_kb(&self, kb_id: &KnowledgeBaseId) -> Result<Option<KnowledgeBaseProfile>> {
        self.store
            .get_json(KB_REPOSITORY_PROFILE_NAMESPACE, kb_id.as_str())
    }

    async fn upsert_kb(&self, profile: KnowledgeBaseProfile) -> Result<()> {
        self.store.put_json(
            KB_REPOSITORY_PROFILE_NAMESPACE,
            profile.kb_id.as_str(),
            &profile,
        )
    }

    async fn upsert_document(&self, document: KnowledgeDocument) -> Result<()> {
        self.store.put_json(
            KB_REPOSITORY_DOCUMENT_NAMESPACE,
            document.doc_id.as_str(),
            &document,
        )
    }

    async fn upsert_media(&self, media: KnowledgeMedia) -> Result<()> {
        self.store.put_json(
            KB_REPOSITORY_MEDIA_NAMESPACE,
            media.media_id.as_str(),
            &media,
        )
    }

    async fn list_documents(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeDocument>> {
        let mut documents = self
            .store
            .list_json::<KnowledgeDocument>(KB_REPOSITORY_DOCUMENT_NAMESPACE)?
            .into_iter()
            .filter(|document| &document.kb_id == kb_id)
            .collect::<Vec<_>>();
        documents.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(documents)
    }

    async fn list_media(&self, doc_id: &DocumentId) -> Result<Vec<KnowledgeMedia>> {
        let mut media = self
            .store
            .list_json::<KnowledgeMedia>(KB_REPOSITORY_MEDIA_NAMESPACE)?
            .into_iter()
            .filter(|record| &record.doc_id == doc_id)
            .collect::<Vec<_>>();
        media.sort_by(|left, right| left.file_name.cmp(&right.file_name));
        Ok(media)
    }
}
