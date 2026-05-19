use astrbot_platform::WEBCHAT_PLATFORM_TYPE;

use crate::{RuntimeChatProviderConfig, RuntimePlatformConfig};

pub(crate) const DEFAULT_EVENT_QUEUE_CAPACITY: usize = 8;
pub(crate) const DEFAULT_MOCK_PLATFORM_ID: &str = "mock";
pub(crate) const DEFAULT_MOCK_PROVIDER_ID: &str = "default-mock";
pub(crate) const DEFAULT_MOCK_RESPONSE: &str = "hello from astrbot-rs";

pub(crate) fn default_event_queue_capacity() -> usize {
    DEFAULT_EVENT_QUEUE_CAPACITY
}

pub(crate) fn default_provider_timeout_secs() -> u64 {
    120
}

pub(crate) fn default_chat_provider_id() -> String {
    DEFAULT_MOCK_PROVIDER_ID.to_string()
}

pub(crate) fn default_chat_providers() -> Vec<RuntimeChatProviderConfig> {
    vec![RuntimeChatProviderConfig::mock(
        DEFAULT_MOCK_PROVIDER_ID,
        DEFAULT_MOCK_RESPONSE,
    )]
}

pub(crate) fn default_dashboard_username() -> String {
    "astrbot".to_string()
}

pub(crate) fn default_dashboard_password() -> String {
    "77b90590a8945a7d36c963981a307dc9".to_string()
}

pub(crate) fn default_dashboard_jwt_secret() -> String {
    "astrbot-rs-dashboard-secret".to_string()
}

pub(crate) fn default_dashboard_token_ttl_seconds() -> u64 {
    7 * 24 * 60 * 60
}

pub(crate) fn default_provider_error_message_option() -> Option<String> {
    Some("LLM 请求失败，请稍后再试。".to_string())
}

pub(crate) fn default_platforms() -> Vec<RuntimePlatformConfig> {
    vec![RuntimePlatformConfig::mock(DEFAULT_MOCK_PLATFORM_ID)]
}

pub(crate) fn default_whitelist_bypass_platform_ids() -> Vec<String> {
    vec![WEBCHAT_PLATFORM_TYPE.to_string()]
}

pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_false() -> bool {
    false
}

pub(crate) fn default_webchat_platform_id() -> String {
    WEBCHAT_PLATFORM_TYPE.to_string()
}

pub(crate) fn default_webchat_server_host() -> String {
    "127.0.0.1".to_string()
}

pub(crate) fn default_webchat_server_port() -> u16 {
    6185
}
