use std::sync::Arc;

use astrbot_core::{
    AstrbotError, MessageChain, MessageComponent, MessageSender, ProviderContentPart,
    ProviderContextMessage, ProviderRequest,
};
use astrbot_kb::{
    ChunkId, DocumentId, EmbeddedKnowledgeChunk, HybridKnowledgeRetriever, InMemoryVectorStore,
    KnowledgeBaseId, KnowledgeChunk, VectorStore, VectorStoreSparseRetriever,
};
use astrbot_memory::{ActiveReplyPolicy, MemoryImageCaptionConfig, MemoryImageCaptionRequest};
use astrbot_provider::{MockEmbeddingProvider, MockRerankProvider};
use astrbot_skill::{
    SkillActivationPolicy, SkillCatalog, SkillDescriptor, SkillPromptInventory,
    SkillPromptRenderer, SkillPromptRuntime, SkillRuntimeSnapshot, SkillSandboxCache,
    SkillSandboxEntry, SkillSource,
};

use crate::{
    AgentActiveReplyDecider, AgentKnowledgeContextSelection, AgentMemoryContextPort, AgentPersona,
    AgentRequestDecoratorComposer, CompositeProviderRequestDecorator, InMemoryAgentMemoryContext,
    KnowledgeContextRequestDecorator, KnowledgeRetrievalContextService, MemoryRequestDecorator,
    PersonaPromptDecorator, ProviderPreferenceRequestDecorator, ProviderRequestDecorator,
    ProviderRequestEnvelope, QuoteContextRequestDecorator, SessionContextRequestDecorator,
    SkillPromptInventoryRequestDecorator,
};

use super::support::{
    StaticKnowledgeContext, StaticKnowledgeSelection, StaticMemoryContext, StaticPreference,
    StaticQuoteContext, StaticSessionContext, event, group_event,
};

struct FailingMemoryCaptioner;

#[async_trait::async_trait]
impl astrbot_memory::MemoryImageCaptioner for FailingMemoryCaptioner {
    async fn caption_image(
        &self,
        request: MemoryImageCaptionRequest,
    ) -> astrbot_core::Result<Option<String>> {
        assert_eq!(request.image_url, "image.png");
        Err(AstrbotError::Provider("caption failed".to_string()))
    }
}

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
async fn skill_prompt_decorator_uses_runtime_snapshot_persona_allowlist_and_sandbox_paths() {
    let catalog = SkillCatalog::from_skills([
        SkillDescriptor::new("writer", "C:\\skills\\writer\\SKILL.md")
            .with_description("Draft clean text"),
        SkillDescriptor::new("draw", "C:\\skills\\draw\\SKILL.md").with_description("Draw image"),
    ]);
    let mut runtime = SkillRuntimeSnapshot::new(catalog).with_sandbox_cache(
        SkillSandboxCache::from_entries([
            SkillSandboxEntry::new("preset").with_description("Sandbox preset")
        ]),
        true,
    );
    runtime
        .set_active("draw", false)
        .expect("draw should be disabled by activation policy");
    let persona = AgentPersona::new("writer-persona").with_skills(Some(vec![
        "writer".to_string(),
        "draw".to_string(),
        "preset".to_string(),
    ]));
    let decorator =
        SkillPromptInventoryRequestDecorator::from_runtime_for_persona(&runtime, &persona)
            .with_renderer(SkillPromptRenderer::new().with_runtime(SkillPromptRuntime::Sandbox));
    let mut request = ProviderRequest::new("hello", "conversation-1");

    decorator
        .decorate(&event("hello"), &mut request)
        .await
        .expect("runtime skills should decorate request");

    let prompt = request.system_prompt.expect("skills prompt should exist");
    assert!(prompt.contains("**writer**"));
    assert!(prompt.contains("**preset**"));
    assert!(!prompt.contains("**draw**"));
    assert!(prompt.contains("C:/skills/writer/SKILL.md"));
    assert!(prompt.contains("/workspace/skills/preset/SKILL.md"));
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
async fn in_memory_agent_memory_records_group_transcript_and_applies_max_count() {
    let memory = Arc::new(
        InMemoryAgentMemoryContext::new()
            .with_retention_policy(astrbot_memory::MemoryRetentionPolicy::new(2)),
    );
    let mut first = group_event("first");
    first.sender = MessageSender::new("user-1", Some("Alice".to_string()));
    let mut second = group_event("second");
    second.sender = MessageSender::new("user-2", Some("Bob".to_string()));
    let mut third = group_event("third");
    third.sender = MessageSender::new("user-3", Some("Carol".to_string()));

    memory
        .record_message_with_timestamp(&first, "12:00:00")
        .await
        .expect("first message should record");
    memory
        .record_message_with_timestamp(&second, "12:00:01")
        .await
        .expect("second message should record");
    memory
        .record_message_with_timestamp(&third, "12:00:02")
        .await
        .expect("third message should record");

    let records = memory
        .memory_records(&third)
        .await
        .expect("records should load");
    assert_eq!(records.len(), 2);
    assert!(!records[0].content.contains("first"));
    assert!(records[0].content.contains("[Bob/12:00:01]: second"));
    assert!(records[1].content.contains("[Carol/12:00:02]: third"));
}

#[tokio::test]
async fn in_memory_agent_memory_degrades_image_caption_failures_and_records_mentions() {
    let memory = Arc::new(InMemoryAgentMemoryContext::new().with_captioner(
        Arc::new(FailingMemoryCaptioner),
        MemoryImageCaptionConfig::enabled("caption"),
    ));
    let mut event = group_event("");
    event.message = MessageChain::new(vec![
        MessageComponent::plain("look"),
        MessageComponent::image("image.png"),
        MessageComponent::mention("user-2"),
    ]);

    let record = memory
        .record_message_with_timestamp(&event, "12:00:00")
        .await
        .expect("caption failure should not fail memory")
        .expect("record should exist");

    assert_eq!(
        record.content,
        "[Alice/12:00:00]: look [Image] [At: user-2]"
    );
}

#[tokio::test]
async fn in_memory_agent_memory_records_llm_response_after_existing_transcript() {
    let memory = Arc::new(InMemoryAgentMemoryContext::new());
    let event = group_event("hello");
    memory
        .record_message_with_timestamp(&event, "12:00:00")
        .await
        .expect("message should record");

    memory
        .record_response_text_with_timestamp(&event, "answer", "12:00:01")
        .await
        .expect("response should record");

    let records = memory
        .memory_records(&event)
        .await
        .expect("records should load");
    assert_eq!(records.len(), 2);
    assert!(records[1].content.contains("[You/12:00:01]: answer"));
}

#[tokio::test]
async fn in_memory_agent_memory_active_reply_decorator_rewrites_prompt_and_cleans_session() {
    let memory = Arc::new(InMemoryAgentMemoryContext::new());
    let event = group_event("new message");
    memory
        .record_message_with_timestamp(&event, "12:00:00")
        .await
        .expect("message should record");

    let decorator = MemoryRequestDecorator::new(memory.clone()).active_reply();
    let mut request = ProviderRequest::new("new message", "room-1")
        .with_context(ProviderContextMessage::text("assistant", "old"));
    decorator
        .decorate(&event, &mut request)
        .await
        .expect("memory should decorate");

    assert!(request.contexts.is_empty());
    let prompt = request.prompt.as_deref().expect("prompt should exist");
    assert!(prompt.contains("[Alice/12:00:00]: new message"));
    assert!(prompt.contains("Please react to it"));

    let removed = memory
        .after_message_sent_cleanup(&event, true)
        .expect("cleanup should remove session");
    assert_eq!(removed, 1);
    assert!(
        memory
            .memory_records(&event)
            .await
            .expect("records should load")
            .is_empty()
    );
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

#[tokio::test]
async fn knowledge_context_decorator_retrieves_session_kb_context() {
    let vector_store = Arc::new(InMemoryVectorStore::default());
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    let doc_id = DocumentId::new("doc-1").expect("doc id");
    vector_store
        .upsert_chunks(vec![
            EmbeddedKnowledgeChunk::new(
                KnowledgeChunk::new(
                    ChunkId::new("chunk-1").expect("chunk id"),
                    kb_id.clone(),
                    doc_id.clone(),
                    0,
                    "Rust knowledge survives retrieval.",
                )
                .with_metadata("kb_name", serde_json::json!("Docs"))
                .with_metadata("doc_name", serde_json::json!("intro.md")),
                vec![0.9, 0.1],
            ),
            EmbeddedKnowledgeChunk::new(
                KnowledgeChunk::new(
                    ChunkId::new("chunk-2").expect("chunk id"),
                    kb_id.clone(),
                    doc_id,
                    1,
                    "Unrelated provider settings.",
                )
                .with_metadata("kb_name", serde_json::json!("Docs"))
                .with_metadata("doc_name", serde_json::json!("intro.md")),
                vec![0.1, 0.9],
            ),
        ])
        .await
        .expect("vectors should seed");
    let retriever = Arc::new(HybridKnowledgeRetriever::new(
        vector_store.clone(),
        Arc::new(VectorStoreSparseRetriever::new(vector_store)),
    ));
    let selection = StaticKnowledgeSelection::new(
        AgentKnowledgeContextSelection::new(["kb-1"])
            .with_top_k(1)
            .with_embedding_provider_id("embedding")
            .with_rerank_provider_id("rerank"),
    );
    let context = KnowledgeRetrievalContextService::new(
        Arc::new(selection),
        Arc::new(MockEmbeddingProvider::new(vec![1.0, 0.0])),
        retriever,
    )
    .with_rerank_provider(Arc::new(MockRerankProvider::new(vec![0.9])));
    let decorator = KnowledgeContextRequestDecorator::new(Arc::new(context));
    let mut request =
        ProviderRequest::new("Rust retrieval?", "conversation-1").with_system_prompt("persona");

    decorator
        .decorate(&event("Rust retrieval?"), &mut request)
        .await
        .expect("knowledge context should retrieve");

    let prompt = request
        .system_prompt
        .as_deref()
        .expect("system prompt should exist");
    assert!(prompt.contains("persona"));
    assert!(prompt.contains("【知识 1】"));
    assert!(prompt.contains("来源: Docs / intro.md"));
    assert!(prompt.contains("Rust knowledge survives retrieval."));
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
