use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ComputerComponent;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooterKind {
    #[default]
    Local,
    Shipyard,
    ShipyardNeo,
    Boxlite,
    Remote,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxEndpoint {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
}

impl SandboxEndpoint {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            access_token: None,
        }
    }

    pub fn with_access_token(mut self, access_token: impl Into<String>) -> Self {
        let access_token = access_token.into();
        self.access_token = (!access_token.trim().is_empty()).then_some(access_token);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerRuntimeConfig {
    pub kind: BooterKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<SandboxEndpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default = "default_ttl_seconds")]
    pub ttl_seconds: u64,
    #[serde(default)]
    pub components: Vec<ComputerComponent>,
}

impl ComputerRuntimeConfig {
    pub fn local() -> Self {
        Self {
            kind: BooterKind::Local,
            endpoint: None,
            profile: None,
            ttl_seconds: default_ttl_seconds(),
            components: vec![ComputerComponent::Shell, ComputerComponent::Python],
        }
    }

    pub fn sandbox(kind: BooterKind) -> Self {
        Self {
            kind,
            endpoint: None,
            profile: None,
            ttl_seconds: default_ttl_seconds(),
            components: ComputerComponent::sandbox_defaults(),
        }
    }

    pub fn with_endpoint(mut self, endpoint: SandboxEndpoint) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    pub fn with_profile(mut self, profile: impl Into<String>) -> Self {
        let profile = profile.into();
        self.profile = (!profile.trim().is_empty()).then_some(profile);
        self
    }

    pub fn with_components<I>(mut self, components: I) -> Self
    where
        I: IntoIterator<Item = ComputerComponent>,
    {
        self.components = normalize_components(components);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SandboxLifecycleState {
    #[default]
    NotBooted,
    Booting,
    Ready,
    Unavailable,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BooterSession {
    pub session_id: String,
    pub config: ComputerRuntimeConfig,
    pub state: SandboxLifecycleState,
    pub components: Vec<ComputerComponent>,
}

impl BooterSession {
    pub fn new(session_id: impl Into<String>, config: ComputerRuntimeConfig) -> Self {
        let components = if config.components.is_empty() {
            ComputerComponent::defaults_for(config.kind)
        } else {
            config.components.clone()
        };
        Self {
            session_id: session_id.into(),
            config,
            state: SandboxLifecycleState::NotBooted,
            components,
        }
    }

    pub fn with_state(mut self, state: SandboxLifecycleState) -> Self {
        self.state = state;
        self
    }

    pub fn supports(&self, component: ComputerComponent) -> bool {
        self.components.contains(&component)
    }
}

#[async_trait]
pub trait ComputerBooter: Send + Sync {
    async fn boot(&self, session_id: &str, config: &ComputerRuntimeConfig)
    -> Result<BooterSession>;

    async fn shutdown(&self, session_id: &str) -> Result<()>;

    async fn available(&self, session_id: &str) -> Result<bool>;

    fn default_components(&self, kind: BooterKind) -> Vec<ComputerComponent> {
        ComputerComponent::defaults_for(kind)
    }
}

#[async_trait]
pub trait BooterRegistry: Send + Sync {
    async fn session(&self, session_id: &str) -> Result<Option<BooterSession>>;

    async fn upsert_session(&self, session: BooterSession) -> Result<()>;

    async fn remove_session(&self, session_id: &str) -> Result<bool>;

    async fn active_sessions(&self) -> Result<Vec<BooterSession>>;
}

#[derive(Default)]
pub struct InMemoryBooterRegistry {
    sessions: RwLock<HashMap<String, BooterSession>>,
}

impl InMemoryBooterRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl BooterRegistry for InMemoryBooterRegistry {
    async fn session(&self, session_id: &str) -> Result<Option<BooterSession>> {
        Ok(self
            .sessions
            .read()
            .map_err(lock_error)?
            .get(session_id)
            .cloned())
    }

    async fn upsert_session(&self, session: BooterSession) -> Result<()> {
        self.sessions
            .write()
            .map_err(lock_error)?
            .insert(session.session_id.clone(), session);
        Ok(())
    }

    async fn remove_session(&self, session_id: &str) -> Result<bool> {
        Ok(self
            .sessions
            .write()
            .map_err(lock_error)?
            .remove(session_id)
            .is_some())
    }

    async fn active_sessions(&self) -> Result<Vec<BooterSession>> {
        let mut sessions = self
            .sessions
            .read()
            .map_err(lock_error)?
            .values()
            .filter(|session| session.state == SandboxLifecycleState::Ready)
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(sessions)
    }
}

pub struct StaticComputerBooter {
    registry: Arc<dyn BooterRegistry>,
}

impl StaticComputerBooter {
    pub fn new(registry: Arc<dyn BooterRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ComputerBooter for StaticComputerBooter {
    async fn boot(
        &self,
        session_id: &str,
        config: &ComputerRuntimeConfig,
    ) -> Result<BooterSession> {
        let session =
            BooterSession::new(session_id, config.clone()).with_state(SandboxLifecycleState::Ready);
        self.registry.upsert_session(session.clone()).await?;
        Ok(session)
    }

    async fn shutdown(&self, session_id: &str) -> Result<()> {
        if let Some(mut session) = self.registry.session(session_id).await? {
            session.state = SandboxLifecycleState::Shutdown;
            self.registry.upsert_session(session).await?;
        }
        Ok(())
    }

    async fn available(&self, session_id: &str) -> Result<bool> {
        Ok(self
            .registry
            .session(session_id)
            .await?
            .is_some_and(|session| session.state == SandboxLifecycleState::Ready))
    }
}

fn default_ttl_seconds() -> u64 {
    3600
}

fn normalize_components<I>(components: I) -> Vec<ComputerComponent>
where
    I: IntoIterator<Item = ComputerComponent>,
{
    let mut components = components.into_iter().collect::<Vec<_>>();
    components.sort();
    components.dedup();
    components
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("computer booter lock: {err}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        BooterKind, BooterRegistry, ComputerBooter, ComputerRuntimeConfig, InMemoryBooterRegistry,
        SandboxLifecycleState, StaticComputerBooter,
    };
    use crate::ComputerComponent;

    #[tokio::test]
    async fn static_booter_tracks_lifecycle_outside_plugin_sdk() {
        let registry = Arc::new(InMemoryBooterRegistry::new());
        let booter = StaticComputerBooter::new(registry.clone());

        let session = booter
            .boot(
                "session-1",
                &ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo)
                    .with_components([ComputerComponent::Shell, ComputerComponent::Browser]),
            )
            .await
            .expect("session should boot");

        assert_eq!(session.state, SandboxLifecycleState::Ready);
        assert!(session.supports(ComputerComponent::Browser));
        assert!(booter.available("session-1").await.expect("available"));
        assert_eq!(
            registry
                .active_sessions()
                .await
                .expect("sessions should list")
                .len(),
            1
        );

        booter
            .shutdown("session-1")
            .await
            .expect("session should shutdown");
        assert!(!booter.available("session-1").await.expect("available"));
    }

    #[test]
    fn runtime_config_has_local_and_sandbox_defaults() {
        assert_eq!(
            ComputerRuntimeConfig::local().components,
            vec![ComputerComponent::Shell, ComputerComponent::Python]
        );
        assert!(
            ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo)
                .components
                .contains(&ComputerComponent::Browser)
        );
    }
}
