use std::collections::BTreeMap;
use std::fmt;

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};

pub fn kb_error(message: impl Into<String>) -> AstrbotError {
    AstrbotError::Pipeline(format!("knowledge-base error: {}", message.into()))
}

macro_rules! id_type {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into().trim().to_string();
                if value.is_empty() {
                    return Err(kb_error(format!("{} cannot be empty", $label)));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = AstrbotError;

            fn try_from(value: &str) -> Result<Self> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = AstrbotError;

            fn try_from(value: String) -> Result<Self> {
                Self::new(value)
            }
        }
    };
}

id_type!(KnowledgeBaseId, "knowledge base id");
id_type!(DocumentId, "document id");
id_type!(ChunkId, "chunk id");
id_type!(MediaId, "media id");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeChunk {
    pub chunk_id: ChunkId,
    pub kb_id: KnowledgeBaseId,
    pub doc_id: DocumentId,
    pub chunk_index: usize,
    pub content: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl KnowledgeChunk {
    pub fn new(
        chunk_id: ChunkId,
        kb_id: KnowledgeBaseId,
        doc_id: DocumentId,
        chunk_index: usize,
        content: impl Into<String>,
    ) -> Self {
        Self {
            chunk_id,
            kb_id,
            doc_id,
            chunk_index,
            content: content.into(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        let key = key.into().trim().to_string();
        if !key.is_empty() {
            self.metadata.insert(key, value);
        }
        self
    }

    pub fn char_count(&self) -> usize {
        self.content.chars().count()
    }
}
