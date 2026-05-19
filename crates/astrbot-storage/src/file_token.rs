use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::SqliteJsonStore;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FileTokenScope {
    Dashboard,
    Plugin,
    Backup,
    Attachment,
    OpenApiFile,
    Custom(String),
}

impl FileTokenScope {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Plugin => "plugin",
            Self::Backup => "backup",
            Self::Attachment => "attachment",
            Self::OpenApiFile => "openapi.file",
            Self::Custom(scope) => scope,
        }
    }
}

impl From<&str> for FileTokenScope {
    fn from(scope: &str) -> Self {
        match scope.trim() {
            "dashboard" => Self::Dashboard,
            "plugin" => Self::Plugin,
            "backup" => Self::Backup,
            "attachment" => Self::Attachment,
            "openapi.file" | "file" => Self::OpenApiFile,
            other => Self::Custom(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTokenRecord {
    pub token: String,
    pub file_path: PathBuf,
    pub scope: FileTokenScope,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub expires_at_unix: Option<u64>,
    pub single_use: bool,
}

#[derive(Clone, Debug)]
pub struct SqliteFileTokenRepository {
    store: SqliteJsonStore,
}

impl SqliteFileTokenRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

impl FileTokenRecord {
    pub fn new(
        token: impl Into<String>,
        file_path: impl Into<PathBuf>,
        scope: FileTokenScope,
    ) -> Self {
        Self {
            token: token.into(),
            file_path: file_path.into(),
            scope,
            filename: None,
            content_type: None,
            expires_at_unix: None,
            single_use: true,
        }
    }

    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = non_empty_string(filename);
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = non_empty_string(content_type);
        self
    }

    pub fn expires_at_unix(mut self, expires_at_unix: u64) -> Self {
        self.expires_at_unix = Some(expires_at_unix);
        self
    }

    pub fn reusable(mut self) -> Self {
        self.single_use = false;
        self
    }

    pub fn is_expired_at(&self, now_unix: u64) -> bool {
        self.expires_at_unix
            .is_some_and(|expires_at| expires_at <= now_unix)
    }
}

#[async_trait]
pub trait FileTokenRepository: Send + Sync {
    async fn put_file_token(&self, record: FileTokenRecord) -> Result<()>;

    async fn file_token(&self, token: &str) -> Result<Option<FileTokenRecord>>;

    async fn consume_file_token(
        &self,
        token: &str,
        now_unix: u64,
    ) -> Result<Option<FileTokenRecord>>;

    async fn remove_expired_file_tokens(&self, now_unix: u64) -> Result<usize>;
}

#[derive(Default)]
pub struct InMemoryFileTokenRepository {
    tokens: RwLock<HashMap<String, FileTokenRecord>>,
}

impl InMemoryFileTokenRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl FileTokenRepository for InMemoryFileTokenRepository {
    async fn put_file_token(&self, mut record: FileTokenRecord) -> Result<()> {
        let token = record.token.trim();
        if token.is_empty() {
            return Err(AstrbotError::Pipeline(
                "file token must not be empty".to_string(),
            ));
        }
        record.token = token.to_string();
        self.tokens
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("file token lock: {err}")))?
            .insert(record.token.clone(), record);
        Ok(())
    }

    async fn file_token(&self, token: &str) -> Result<Option<FileTokenRecord>> {
        Ok(self
            .tokens
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("file token lock: {err}")))?
            .get(token.trim())
            .cloned())
    }

    async fn consume_file_token(
        &self,
        token: &str,
        now_unix: u64,
    ) -> Result<Option<FileTokenRecord>> {
        let token = token.trim();
        let mut tokens = self
            .tokens
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("file token lock: {err}")))?;
        let Some(record) = tokens.get(token).cloned() else {
            return Ok(None);
        };
        if record.is_expired_at(now_unix) {
            tokens.remove(token);
            return Ok(None);
        }
        if record.single_use {
            tokens.remove(token);
        }
        Ok(Some(record))
    }

    async fn remove_expired_file_tokens(&self, now_unix: u64) -> Result<usize> {
        let mut tokens = self
            .tokens
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("file token lock: {err}")))?;
        let before = tokens.len();
        tokens.retain(|_, record| !record.is_expired_at(now_unix));
        Ok(before - tokens.len())
    }
}

#[async_trait]
impl FileTokenRepository for SqliteFileTokenRepository {
    async fn put_file_token(&self, mut record: FileTokenRecord) -> Result<()> {
        let token = record.token.trim();
        if token.is_empty() {
            return Err(AstrbotError::Pipeline(
                "file token must not be empty".to_string(),
            ));
        }
        record.token = token.to_string();
        self.store.put_json("file_tokens", &record.token, &record)
    }

    async fn file_token(&self, token: &str) -> Result<Option<FileTokenRecord>> {
        self.store.get_json("file_tokens", token.trim())
    }

    async fn consume_file_token(
        &self,
        token: &str,
        now_unix: u64,
    ) -> Result<Option<FileTokenRecord>> {
        let token = token.trim();
        let Some(record) = self
            .store
            .get_json::<FileTokenRecord>("file_tokens", token)?
        else {
            return Ok(None);
        };
        if record.is_expired_at(now_unix) {
            self.store.delete_json("file_tokens", token)?;
            return Ok(None);
        }
        if record.single_use {
            self.store.delete_json("file_tokens", token)?;
        }
        Ok(Some(record))
    }

    async fn remove_expired_file_tokens(&self, now_unix: u64) -> Result<usize> {
        let expired = self
            .store
            .list_json::<FileTokenRecord>("file_tokens")?
            .into_iter()
            .filter(|record| record.is_expired_at(now_unix))
            .map(|record| record.token)
            .collect::<Vec<_>>();
        let mut removed = 0;
        for token in expired {
            if self.store.delete_json("file_tokens", &token)? {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        FileTokenRecord, FileTokenRepository, FileTokenScope, InMemoryFileTokenRepository,
        SqliteFileTokenRepository,
    };
    use crate::SqliteJsonStore;

    #[tokio::test]
    async fn file_token_repository_consumes_single_use_scoped_records() {
        let repository = InMemoryFileTokenRepository::new();
        repository
            .put_file_token(
                FileTokenRecord::new(" token-1 ", "exports/report.zip", FileTokenScope::Backup)
                    .with_filename("report.zip")
                    .with_content_type("application/zip")
                    .expires_at_unix(200),
            )
            .await
            .expect("file token should store");

        let record = repository
            .consume_file_token("token-1", 100)
            .await
            .expect("file token should consume")
            .expect("file token should exist");
        assert_eq!(record.scope, FileTokenScope::Backup);
        assert_eq!(record.filename.as_deref(), Some("report.zip"));
        assert_eq!(record.content_type.as_deref(), Some("application/zip"));
        assert_eq!(
            repository
                .consume_file_token("token-1", 100)
                .await
                .expect("file token lookup should not fail"),
            None
        );
    }

    #[tokio::test]
    async fn file_token_repository_keeps_reusable_tokens_and_cleans_expired() {
        let repository = InMemoryFileTokenRepository::new();
        repository
            .put_file_token(
                FileTokenRecord::new("token-1", "dashboard/index.html", FileTokenScope::Dashboard)
                    .expires_at_unix(200)
                    .reusable(),
            )
            .await
            .expect("file token should store");

        assert!(
            repository
                .consume_file_token("token-1", 100)
                .await
                .expect("file token should consume")
                .is_some()
        );
        assert!(
            repository
                .file_token("token-1")
                .await
                .expect("file token should load")
                .is_some()
        );
        assert_eq!(
            repository
                .remove_expired_file_tokens(200)
                .await
                .expect("expired file token should cleanup"),
            1
        );
        assert_eq!(
            repository
                .file_token("token-1")
                .await
                .expect("file token should load"),
            None
        );
    }

    #[tokio::test]
    async fn sqlite_file_token_repository_consumes_persisted_tokens() {
        let store = SqliteJsonStore::open_in_memory().expect("sqlite store should open");
        let repository = SqliteFileTokenRepository::new(store.clone());
        repository
            .put_file_token(
                FileTokenRecord::new("token-1", "dashboard/index.html", FileTokenScope::Dashboard)
                    .expires_at_unix(200),
            )
            .await
            .expect("file token should store");

        let reloaded = SqliteFileTokenRepository::new(store);
        assert!(
            reloaded
                .consume_file_token("token-1", 100)
                .await
                .expect("file token should consume")
                .is_some()
        );
        assert!(
            reloaded
                .file_token("token-1")
                .await
                .expect("file token should load")
                .is_none()
        );
    }
}
