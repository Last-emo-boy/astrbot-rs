use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiKeyRecord {
    pub key_id: String,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub created_by: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

impl ApiKeyRecord {
    pub fn new(
        key_id: impl Into<String>,
        name: impl Into<String>,
        key_hash: impl Into<String>,
        key_prefix: impl Into<String>,
        scopes: impl IntoIterator<Item = impl Into<String>>,
        created_by: impl Into<String>,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            name: name.into(),
            key_hash: key_hash.into(),
            key_prefix: key_prefix.into(),
            scopes: scopes.into_iter().map(Into::into).collect(),
            created_by: created_by.into(),
            expires_at: None,
            revoked_at: None,
        }
    }

    pub fn with_expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = Some(expires_at.into());
        self
    }

    pub fn revoked(mut self, revoked_at: impl Into<String>) -> Self {
        self.revoked_at = Some(revoked_at.into());
        self
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }
}

#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    async fn store_api_key(&self, record: ApiKeyRecord) -> Result<()>;

    async fn api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKeyRecord>>;

    async fn list_api_keys(&self) -> Result<Vec<ApiKeyRecord>>;

    async fn revoke_api_key(&self, key_id: &str, revoked_at: String) -> Result<bool>;
}

#[derive(Default)]
pub struct InMemoryApiKeyRepository {
    api_keys: RwLock<HashMap<String, ApiKeyRecord>>,
}

impl InMemoryApiKeyRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ApiKeyRepository for InMemoryApiKeyRepository {
    async fn store_api_key(&self, record: ApiKeyRecord) -> Result<()> {
        self.api_keys
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))?
            .insert(record.key_id.clone(), record);
        Ok(())
    }

    async fn api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKeyRecord>> {
        self.api_keys
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))
            .map(|api_keys| {
                api_keys
                    .values()
                    .find(|record| record.key_hash == key_hash)
                    .cloned()
            })
    }

    async fn list_api_keys(&self) -> Result<Vec<ApiKeyRecord>> {
        let mut api_keys = self
            .api_keys
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        api_keys.sort_by(|left, right| left.key_id.cmp(&right.key_id));
        Ok(api_keys)
    }

    async fn revoke_api_key(&self, key_id: &str, revoked_at: String) -> Result<bool> {
        let mut api_keys = self
            .api_keys
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))?;
        let Some(record) = api_keys.get_mut(key_id) else {
            return Ok(false);
        };
        record.revoked_at = Some(revoked_at);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiKeyRecord, ApiKeyRepository, InMemoryApiKeyRepository};

    #[tokio::test]
    async fn api_key_repository_stores_finds_and_revokes_keys() {
        let repository = InMemoryApiKeyRepository::new();
        let record = ApiKeyRecord::new(
            "key-1",
            "Automation",
            "hash-1",
            "ak_1234",
            ["management.read", "openapi.chat"],
            "admin",
        );

        repository
            .store_api_key(record.clone())
            .await
            .expect("api key should store");

        assert_eq!(
            repository
                .api_key_by_hash("hash-1")
                .await
                .expect("api key should load"),
            Some(record)
        );
        assert_eq!(
            repository
                .list_api_keys()
                .await
                .expect("api keys should list")
                .len(),
            1
        );
        assert!(
            repository
                .revoke_api_key("key-1", "2026-05-17T00:00:00Z".to_string())
                .await
                .expect("api key should revoke")
        );
        assert!(
            repository
                .api_key_by_hash("hash-1")
                .await
                .expect("api key should load")
                .expect("api key should exist")
                .is_revoked()
        );
    }
}
