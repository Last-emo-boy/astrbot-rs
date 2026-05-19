use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::SqliteJsonStore;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KbProfileRecord {
    pub kb_id: String,
    pub name: String,
    pub description: Option<String>,
    pub embedding_provider_id: String,
    pub doc_count: usize,
    pub chunk_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KbDocumentRecord {
    pub doc_id: String,
    pub kb_id: String,
    pub name: String,
    pub file_type: String,
    pub file_size: usize,
    pub file_path: Option<String>,
    pub chunk_count: usize,
    pub media_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KbMediaRecord {
    pub media_id: String,
    pub doc_id: String,
    pub kb_id: String,
    pub media_type: String,
    pub file_name: String,
    pub file_path: Option<String>,
    pub file_size: usize,
    pub mime_type: String,
}

#[async_trait]
pub trait KbDocumentRepository: Send + Sync {
    async fn upsert_profile(&self, profile: KbProfileRecord) -> Result<()>;

    async fn get_profile(&self, kb_id: &str) -> Result<Option<KbProfileRecord>>;

    async fn upsert_document(&self, document: KbDocumentRecord) -> Result<()>;

    async fn list_documents(&self, kb_id: &str) -> Result<Vec<KbDocumentRecord>>;

    async fn upsert_media(&self, media: KbMediaRecord) -> Result<()>;

    async fn list_media(&self, doc_id: &str) -> Result<Vec<KbMediaRecord>>;
}

#[derive(Default)]
pub struct InMemoryKbDocumentRepository {
    profiles: RwLock<Vec<KbProfileRecord>>,
    documents: RwLock<Vec<KbDocumentRecord>>,
    media: RwLock<Vec<KbMediaRecord>>,
}

#[derive(Clone, Debug)]
pub struct SqliteKbDocumentRepository {
    store: SqliteJsonStore,
}

impl SqliteKbDocumentRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

impl InMemoryKbDocumentRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KbDocumentRepository for InMemoryKbDocumentRepository {
    async fn upsert_profile(&self, profile: KbProfileRecord) -> Result<()> {
        let mut profiles = self
            .profiles
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("kb profile lock: {err}")))?;
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

    async fn get_profile(&self, kb_id: &str) -> Result<Option<KbProfileRecord>> {
        let profiles = self
            .profiles
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("kb profile lock: {err}")))?;
        Ok(profiles
            .iter()
            .find(|profile| profile.kb_id == kb_id)
            .cloned())
    }

    async fn upsert_document(&self, document: KbDocumentRecord) -> Result<()> {
        let mut documents = self
            .documents
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("kb document lock: {err}")))?;
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

    async fn list_documents(&self, kb_id: &str) -> Result<Vec<KbDocumentRecord>> {
        let documents = self
            .documents
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("kb document lock: {err}")))?;
        Ok(documents
            .iter()
            .filter(|document| document.kb_id == kb_id)
            .cloned()
            .collect())
    }

    async fn upsert_media(&self, media: KbMediaRecord) -> Result<()> {
        let mut records = self
            .media
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("kb media lock: {err}")))?;
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

    async fn list_media(&self, doc_id: &str) -> Result<Vec<KbMediaRecord>> {
        let media = self
            .media
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("kb media lock: {err}")))?;
        Ok(media
            .iter()
            .filter(|record| record.doc_id == doc_id)
            .cloned()
            .collect())
    }
}

#[async_trait]
impl KbDocumentRepository for SqliteKbDocumentRepository {
    async fn upsert_profile(&self, profile: KbProfileRecord) -> Result<()> {
        self.store
            .put_json("kb_document_profiles", &profile.kb_id, &profile)
    }

    async fn get_profile(&self, kb_id: &str) -> Result<Option<KbProfileRecord>> {
        self.store.get_json("kb_document_profiles", kb_id)
    }

    async fn upsert_document(&self, document: KbDocumentRecord) -> Result<()> {
        self.store
            .put_json("kb_document_records", &document.doc_id, &document)
    }

    async fn list_documents(&self, kb_id: &str) -> Result<Vec<KbDocumentRecord>> {
        Ok(self
            .store
            .list_json::<KbDocumentRecord>("kb_document_records")?
            .into_iter()
            .filter(|document| document.kb_id == kb_id)
            .collect())
    }

    async fn upsert_media(&self, media: KbMediaRecord) -> Result<()> {
        self.store
            .put_json("kb_media_records", &media.media_id, &media)
    }

    async fn list_media(&self, doc_id: &str) -> Result<Vec<KbMediaRecord>> {
        Ok(self
            .store
            .list_json::<KbMediaRecord>("kb_media_records")?
            .into_iter()
            .filter(|record| record.doc_id == doc_id)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InMemoryKbDocumentRepository, KbDocumentRecord, KbDocumentRepository, KbProfileRecord,
        SqliteKbDocumentRepository,
    };
    use crate::SqliteJsonStore;

    #[tokio::test]
    async fn kb_document_repository_keeps_metadata_outside_dashboard_routes() {
        let repository = InMemoryKbDocumentRepository::new();
        repository
            .upsert_profile(KbProfileRecord {
                kb_id: "kb-1".to_string(),
                name: "Docs".to_string(),
                description: None,
                embedding_provider_id: "embedding".to_string(),
                doc_count: 0,
                chunk_count: 0,
            })
            .await
            .expect("profile should store");
        repository
            .upsert_document(KbDocumentRecord {
                doc_id: "doc-1".to_string(),
                kb_id: "kb-1".to_string(),
                name: "intro.txt".to_string(),
                file_type: "txt".to_string(),
                file_size: 11,
                file_path: None,
                chunk_count: 2,
                media_count: 0,
            })
            .await
            .expect("document should store");

        assert_eq!(
            repository
                .get_profile("kb-1")
                .await
                .expect("profile should load")
                .expect("profile should exist")
                .name,
            "Docs"
        );
        assert_eq!(
            repository
                .list_documents("kb-1")
                .await
                .expect("documents should list")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn sqlite_kb_document_repository_keeps_metadata() {
        let repository = SqliteKbDocumentRepository::new(
            SqliteJsonStore::open_in_memory().expect("sqlite store should open"),
        );
        repository
            .upsert_profile(KbProfileRecord {
                kb_id: "kb-1".to_string(),
                name: "Docs".to_string(),
                description: None,
                embedding_provider_id: "embedding".to_string(),
                doc_count: 0,
                chunk_count: 0,
            })
            .await
            .expect("profile should store");

        assert_eq!(
            repository
                .get_profile("kb-1")
                .await
                .expect("profile should load")
                .expect("profile should exist")
                .name,
            "Docs"
        );
    }
}
