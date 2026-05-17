use std::sync::Arc;

use astrbot_core::{ProviderContentPart, ProviderContextMessage, ProviderRequest};
use astrbot_memory::ActiveReplyPolicy;
use astrbot_skill::{
    SkillActivationPolicy, SkillCatalog, SkillDescriptor, SkillPromptInventory, SkillSource,
};

use crate::{
    AgentActiveReplyDecider, AgentPersona, AgentRequestDecoratorComposer,
    CompositeProviderRequestDecorator, KnowledgeContextRequestDecorator, MemoryRequestDecorator,
    PersonaPromptDecorator, ProviderPreferenceRequestDecorator, ProviderRequestDecorator,
    ProviderRequestEnvelope, QuoteContextRequestDecorator, SessionContextRequestDecorator,
    SkillPromptInventoryRequestDecorator,
};

use super::support::{
    StaticKnowledgeContext, StaticMemoryContext, StaticPreference, StaticQuoteContext,
    StaticSessionContext, event, group_event,
};

#[tokio::test]
async fn composite_decorator_applies_preference_context_quote_and_persona() {
    let decorator = CompositeProviderRequestDecorator::new()
        .with_decorator(Arc::new(ProviderPreferenceRequestDecorator::new(Arc::new(
            StaticPreference,
        ))))
        .with_decorator(Arc::new(SessionContextRequestDecorator::new(Arc::new(
            StaticSessionContext,
        ))))
        .with_decorator(Arc::new(QuoteContextRequestDecorator::new(Arc::new(
            StaticQuoteContext,
        ))))
        .with_decorator(Arc::new(PersonaPromptDecorator::new(
            AgentPersona::new("default").with_system_prompt("persona prompt"),
        )));
    let mut request = ProviderRequest::new("hello", "conversation-1");

    decorator
        .decorate(&event("hello"), &mut request)
        .await
        .expect("request should decorate");

    assert_eq!(request.provider_id.as_deref(), Some("preferred-provider"));
    assert_eq!(request.contexts.len(), 1);
    assert_eq!(request.contexts[0].role, "assistant");
    assert_eq!(
        request.extra_user_content_parts,
        vec![ProviderContentPart::text("quoted")]
    );
    assert_eq!(request.system_prompt.as_deref(), Some("persona prompt"));
}

#[test]
fn request_envelope_marks_explicit_provider_request_and_applies_event_defaults() {
    let mut message_event = event("fallback text");
    message_event.set_provider_request(ProviderRequest::default().with_prompt("explicit text"));

    let envelope = ProviderRequestEnvelope::from_event(&message_event)
        .expect("explicit provider request should produce envelope");

    assert!(envelope.explicit);
    assert_eq!(envelope.request.prompt.as_deref(), Some("explicit text"));
    assert_eq!(
        envelope.request.session_id.as_deref(),
        Some("conversation-1")
    );
}

#[tokio::test]
async fn request_decorator_composer_keeps_decorator_order() {
    let decorator = AgentRequestDecoratorComposer::new()
        .with_decorator(Arc::new(SessionContextRequestDecorator::new(Arc::new(
            StaticSessionContext,
        ))))
        .with_decorator(Arc::new(QuoteContextRequestDecorator::new(Arc::new(
            StaticQuoteContext,
        ))))
        .build();
    let mut request = ProviderRequest::new("hello", "conversation-1");

    decorator
        .decorate(&event("hello"), &mut request)
        .await
        .expect("composed decorators should run");

    assert_eq!(request.contexts[0].role, "assistant");
    assert_eq!(
        request.extra_user_content_parts,
        vec![ProviderContentPart::text("quoted")]
    );
}

#[tokio::test]
async fn skill_prompt_inventory_decorator_appends_active_skill_prompt_without_package_logic() {
    let mut catalog = SkillCatalog::new();
    catalog.add_skill(
        SkillDescriptor::new("writer", "C:\\skills\\writer\\SKILL.md")
            .with_description("Draft clean text"),
    );
    catalog.add_skill(
        SkillDescriptor::new("preset", "/workspace/skills/preset/SKILL.md")
            .with_description("Sandbox preset")
            .with_source(SkillSource::Sandbox),
    );
    let inventory = SkillPromptInventory::from_catalog(
        &catalog,
        &SkillActivationPolicy::all_enabled().disable("preset"),
    );
    let persona = AgentPersona::new("default")
        .with_system_prompt("persona prompt")
        .with_skills(Some(vec!["writer".to_string()]));
    let decorator = CompositeProviderRequestDecorator::new()
        .with_decorator(Arc::new(PersonaPromptDecorator::new(persona.clone())))
        .with_decorator(Arc::new(SkillPromptInventoryRequestDecorator::for_persona(
            inventory, &persona,
        )));
    let mut request = ProviderRequest::new("hello", "conversation-1");

    decorator
        .decorate(&event("hello"), &mut request)
        .await
        .expect("request should decorate with skill inventory");

    let system_prompt = request.system_prompt.expect("system prompt should exist");
    assert!(system_prompt.contains("persona prompt"));
    assert!(system_prompt.contains("## Skills"));
    assert!(system_prompt.contains("**writer**"));
    assert!(!system_prompt.contains("**preset**"));
    assert!(system_prompt.contains("C:/skills/writer/SKILL.md"));
}

#[tokio::test]
async fn memory_request_decorator_appends_history_to_system_prompt() {
    let decorator = MemoryRequestDecorator::new(Arc::new(StaticMemoryContext));
    let mut request = ProviderRequest::new("what happened", "room-1")
        .with_system_prompt("persona")
        .with_context(ProviderContextMessage::text("assistant", "old"));

    decorator
        .decorate(&group_event("what happened"), &mut request)
        .await
        .expect("memory decorator should run");

    let system_prompt = request
        .system_prompt
        .as_deref()
        .expect("system prompt should exist");
    assert!(system_prompt.contains("persona"));
    assert!(system_prompt.contains("[Alice/12:00:00]: hello"));
    assert_eq!(request.contexts.len(), 1);
}

#[tokio::test]
async fn memory_request_decorator_can_rewrite_active_reply_prompt() {
    let decorator = MemoryRequestDecorator::new(Arc::new(StaticMemoryContext)).active_reply();
    let mut request = ProviderRequest::new("new message", "room-1")
        .with_context(ProviderContextMessage::text("assistant", "old"));

    decorator
        .decorate(&group_event("new message"), &mut request)
        .await
        .expect("memory decorator should run");

    assert!(request.contexts.is_empty());
    let prompt = request.prompt.as_deref().expect("prompt should exist");
    assert!(prompt.contains("[Alice/12:00:00]: hello"));
    assert!(prompt.contains("new message"));
}

#[tokio::test]
async fn knowledge_context_decorator_consumes_formatted_context_without_ingestion_ports() {
    let decorator = KnowledgeContextRequestDecorator::new(Arc::new(StaticKnowledgeContext));
    let mut request =
        ProviderRequest::new("question", "conversation-1").with_system_prompt("persona prompt");

    decorator
        .decorate(&event("question"), &mut request)
        .await
        .expect("knowledge context should decorate");

    let prompt = request
        .system_prompt
        .as_deref()
        .expect("system prompt should exist");
    assert!(prompt.contains("persona prompt"));
    assert!(prompt.contains("【知识 1】"));
    assert!(prompt.contains("Rust boundary"));
}

#[test]
fn active_reply_decider_uses_memory_policy_without_platform_adapter() {
    let decider = AgentActiveReplyDecider::new(
        ActiveReplyPolicy::probability(0.5).with_whitelist(["room-1"]),
    );

    assert!(decider.should_reply(&group_event("hello"), 0.25));
    assert!(!decider.should_reply(&group_event("hello"), 0.75));

    let mut wake_event = group_event("hello");
    wake_event.mark_wake(true);
    assert!(!decider.should_reply(&wake_event, 0.25));
    assert!(!decider.should_reply(&event("direct"), 0.25));
}
