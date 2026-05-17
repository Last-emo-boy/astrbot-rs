use std::sync::Arc;

use astrbot_core::{ProviderContentPart, ProviderContextMessage, ProviderRequest};

use crate::{
    AgentContextCompressor, AgentContextWindow, ApproximateTokenCounter, ContextTokenBudget,
    ContextTruncationPolicy, ContextWindowManager, ContextWindowRequestDecorator,
    NoopContextCompressor, ProviderRequestDecorator,
};

use super::support::{OneTokenPerMessageCounter, event};

#[test]
fn approximate_counter_counts_text_parts_across_context_window() {
    let counter = ApproximateTokenCounter;
    let window = AgentContextWindow::from_messages(vec![
        ProviderContextMessage::text("user", "hello"),
        ProviderContextMessage::text("assistant", "你好"),
    ]);

    assert_eq!(window.total_tokens(&counter), 4);
}

#[tokio::test]
async fn context_window_manager_truncates_to_newest_messages_under_budget() {
    let manager = ContextWindowManager::new(ContextTokenBudget::new(2))
        .with_token_counter(Arc::new(OneTokenPerMessageCounter))
        .with_truncation_policy(ContextTruncationPolicy::new());
    let messages = vec![
        ProviderContextMessage::text("user", "old user"),
        ProviderContextMessage::text("assistant", "old assistant"),
        ProviderContextMessage::text("user", "new user"),
        ProviderContextMessage::text("assistant", "new assistant"),
    ];

    let prepared = manager
        .prepare_messages(messages)
        .await
        .expect("context should truncate");

    assert_eq!(
        prepared,
        vec![
            ProviderContextMessage::text("user", "new user"),
            ProviderContextMessage::text("assistant", "new assistant"),
        ]
    );
}

#[tokio::test]
async fn context_window_decorator_rewrites_only_contexts() {
    let manager = Arc::new(
        ContextWindowManager::new(ContextTokenBudget::new(1))
            .with_token_counter(Arc::new(OneTokenPerMessageCounter)),
    );
    let decorator = ContextWindowRequestDecorator::new(manager);
    let mut request = ProviderRequest::new("hello", "conversation-1")
        .with_provider_id("provider-1")
        .with_extra_user_content_part(ProviderContentPart::text("quoted"));
    request.contexts = vec![
        ProviderContextMessage::text("user", "old user"),
        ProviderContextMessage::text("assistant", "old assistant"),
        ProviderContextMessage::text("user", "new user"),
    ];

    decorator
        .decorate(&event("hello"), &mut request)
        .await
        .expect("context decorator should run");

    assert_eq!(request.prompt.as_deref(), Some("hello"));
    assert_eq!(request.provider_id.as_deref(), Some("provider-1"));
    assert_eq!(
        request.extra_user_content_parts,
        vec![ProviderContentPart::text("quoted")]
    );
    assert_eq!(
        request.contexts,
        vec![ProviderContextMessage::text("user", "new user")]
    );
}

#[tokio::test]
async fn noop_context_compressor_keeps_window_shape() {
    let compressor = NoopContextCompressor;
    let counter = ApproximateTokenCounter;
    let budget = ContextTokenBudget::new(1);
    let window = AgentContextWindow::from_messages(vec![
        ProviderContextMessage::text("user", "hello"),
        ProviderContextMessage::text("assistant", "world"),
    ]);

    let compressed = compressor
        .compress(window.clone(), &budget, &counter)
        .await
        .expect("noop compressor should succeed");

    assert_eq!(compressed, window);
}
