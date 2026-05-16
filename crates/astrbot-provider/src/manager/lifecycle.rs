use astrbot_core::{AstrbotError, Result};
use astrbot_observability::{ComponentKind, ComponentStatus, StatusEvent, StatusSeverity};

use super::ProviderManager;

impl ProviderManager {
    pub async fn terminate(&self) -> Result<()> {
        for (provider_id, provider) in &self.chat_providers {
            self.emit_stopping("chat", provider_id);
            provider.terminate().await.map_err(|err| {
                self.emit_failed("chat", provider_id, &err);
                AstrbotError::Provider(format!("terminate chat provider {provider_id}: {err}"))
            })?;
            self.emit_stopped("chat", provider_id);
        }
        for (provider_id, provider) in &self.speech_to_text_providers {
            self.emit_stopping("speech-to-text", provider_id);
            provider.terminate().await.map_err(|err| {
                self.emit_failed("speech-to-text", provider_id, &err);
                AstrbotError::Provider(format!(
                    "terminate speech-to-text provider {provider_id}: {err}"
                ))
            })?;
            self.emit_stopped("speech-to-text", provider_id);
        }
        for (provider_id, provider) in &self.text_to_speech_providers {
            self.emit_stopping("text-to-speech", provider_id);
            provider.terminate().await.map_err(|err| {
                self.emit_failed("text-to-speech", provider_id, &err);
                AstrbotError::Provider(format!(
                    "terminate text-to-speech provider {provider_id}: {err}"
                ))
            })?;
            self.emit_stopped("text-to-speech", provider_id);
        }
        for (provider_id, provider) in &self.embedding_providers {
            self.emit_stopping("embedding", provider_id);
            provider.terminate().await.map_err(|err| {
                self.emit_failed("embedding", provider_id, &err);
                AstrbotError::Provider(format!("terminate embedding provider {provider_id}: {err}"))
            })?;
            self.emit_stopped("embedding", provider_id);
        }
        for (provider_id, provider) in &self.rerank_providers {
            self.emit_stopping("rerank", provider_id);
            provider.terminate().await.map_err(|err| {
                self.emit_failed("rerank", provider_id, &err);
                AstrbotError::Provider(format!("terminate rerank provider {provider_id}: {err}"))
            })?;
            self.emit_stopped("rerank", provider_id);
        }
        Ok(())
    }

    fn emit_stopping(&self, capability: &str, provider_id: &str) {
        self.status_sink.emit(
            StatusEvent::new(ComponentKind::Provider, ComponentStatus::Stopping)
                .with_component_id(provider_id.to_string())
                .with_message(format!("{capability} provider stopping")),
        );
    }

    fn emit_stopped(&self, capability: &str, provider_id: &str) {
        self.status_sink.emit(
            StatusEvent::new(ComponentKind::Provider, ComponentStatus::Stopped)
                .with_component_id(provider_id.to_string())
                .with_message(format!("{capability} provider stopped")),
        );
    }

    fn emit_failed(&self, capability: &str, provider_id: &str, error: impl ToString) {
        self.status_sink.emit(
            StatusEvent::new(ComponentKind::Provider, ComponentStatus::Failed)
                .with_component_id(provider_id.to_string())
                .with_severity(StatusSeverity::Error)
                .with_message(format!(
                    "{capability} provider failed: {}",
                    error.to_string()
                )),
        );
    }
}
