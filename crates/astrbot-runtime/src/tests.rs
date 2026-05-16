mod config_io;
mod config_route;
mod config_schema;
mod config_service;
mod lifecycle;
mod message_loop;
mod platform;
mod policy;
mod provider_config;

use astrbot_platform::SentMessage;

use crate::RuntimeHandle;

pub(super) async fn wait_for_sent_messages(
    handle: &RuntimeHandle,
    expected: usize,
) -> Vec<SentMessage> {
    for _ in 0..64 {
        let sent = handle.sent_messages().await;
        if sent.len() >= expected {
            return sent;
        }
        tokio::task::yield_now().await;
    }
    handle.sent_messages().await
}

pub(super) fn temp_runtime_config_path(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-runtime-config-{}-{}.json",
        std::process::id(),
        suffix
    ))
}
