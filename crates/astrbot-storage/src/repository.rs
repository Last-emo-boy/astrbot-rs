use async_trait::async_trait;

use astrbot_core::Result;

use crate::schema::StorageSchema;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepositoryBackendKind {
    InMemory,
    Sqlite,
    External(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryImplementationDescriptor {
    pub name: String,
    pub backend: RepositoryBackendKind,
    pub schema_name: String,
    pub schema_version: String,
}

impl RepositoryImplementationDescriptor {
    pub fn new(
        name: impl Into<String>,
        backend: RepositoryBackendKind,
        schema: &StorageSchema,
    ) -> Self {
        Self {
            name: name.into(),
            backend,
            schema_name: schema.name.clone(),
            schema_version: format!("v{}", schema.version),
        }
    }
}

#[async_trait]
pub trait StorageRepositoryBoundary: Send + Sync {
    fn descriptor(&self) -> RepositoryImplementationDescriptor;

    async fn health_check(&self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::{RepositoryBackendKind, RepositoryImplementationDescriptor};
    use crate::StorageSchema;

    #[test]
    fn descriptor_carries_backend_and_schema_identity() {
        let schema = StorageSchema::repository_port_schema();
        let descriptor =
            RepositoryImplementationDescriptor::new("main", RepositoryBackendKind::Sqlite, &schema);

        assert_eq!(descriptor.name, "main");
        assert_eq!(descriptor.backend, RepositoryBackendKind::Sqlite);
        assert_eq!(descriptor.schema_name, "repository_ports");
        assert_eq!(descriptor.schema_version, "v1");
    }
}
