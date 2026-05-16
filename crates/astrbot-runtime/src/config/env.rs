use crate::{RuntimeChatProviderConfig, RuntimeConfig};

use super::secrets::SecretValue;

const OPENAI_PROVIDER_ID: &str = "env-openai";
const DEFAULT_OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";
const DEFAULT_OPENAI_TIMEOUT_SECS: u64 = 120;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeEnvConfigSource {
    pub api_key_var: &'static str,
    pub api_base_var: &'static str,
    pub model_var: &'static str,
    pub timeout_secs_var: &'static str,
}

impl Default for RuntimeEnvConfigSource {
    fn default() -> Self {
        Self {
            api_key_var: "ASTRBOT_OPENAI_API_KEY",
            api_base_var: "ASTRBOT_OPENAI_API_BASE",
            model_var: "ASTRBOT_OPENAI_MODEL",
            timeout_secs_var: "ASTRBOT_OPENAI_TIMEOUT_SECS",
        }
    }
}

impl RuntimeEnvConfigSource {
    pub fn load(&self) -> RuntimeConfig {
        self.load_from(|key| std::env::var(key).ok())
    }

    pub fn load_from<F>(&self, lookup: F) -> RuntimeConfig
    where
        F: Fn(&str) -> Option<String>,
    {
        match self.openai_provider_from_lookup(&lookup) {
            Some(provider) => RuntimeConfig {
                default_chat_provider_id: OPENAI_PROVIDER_ID.to_string(),
                chat_providers: vec![provider],
                ..RuntimeConfig::default()
            },
            None => RuntimeConfig::default(),
        }
    }

    fn openai_provider_from_lookup<F>(&self, lookup: &F) -> Option<RuntimeChatProviderConfig>
    where
        F: Fn(&str) -> Option<String>,
    {
        let api_key = lookup(self.api_key_var)?;
        if api_key.trim().is_empty() {
            return None;
        }

        let api_base = lookup(self.api_base_var).unwrap_or_else(|| DEFAULT_OPENAI_API_BASE.into());
        let model = lookup(self.model_var).unwrap_or_else(|| DEFAULT_OPENAI_MODEL.into());
        let timeout_secs = lookup(self.timeout_secs_var)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_OPENAI_TIMEOUT_SECS);
        let secret = SecretValue::new(api_key);

        Some(
            RuntimeChatProviderConfig::openai_compatible(OPENAI_PROVIDER_ID, api_base, model)
                .with_api_key(secret.expose_secret().to_string())
                .with_timeout_secs(timeout_secs),
        )
    }
}

pub fn runtime_config_from_env() -> RuntimeConfig {
    RuntimeEnvConfigSource::default().load()
}
