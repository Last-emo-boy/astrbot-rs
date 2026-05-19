use std::sync::Arc;

use astrbot_core::{MessageEvent, ProviderRequest, Result};
use astrbot_kb::{
    KnowledgeBaseId, KnowledgeContextFormatter, KnowledgeRetrievalRequest, KnowledgeRetriever,
    RetrievalContextFormatter,
};
use astrbot_provider::{EmbeddingProvider, EmbeddingRequest, RerankProvider};
use async_trait::async_trait;

use crate::ProviderRequestDecorator;
use crate::tool_loop::{AgentToolExecutionRequest, AgentToolExecutionResult, AgentToolExecutor};

#[async_trait]
pub trait AgentKnowledgeContextPort: Send + Sync {
    async fn formatted_knowledge_context(&self, event: &MessageEvent) -> Result<Option<String>>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentKnowledgeContextSelection {
    pub kb_ids: Vec<String>,
    pub top_k: Option<usize>,
    pub embedding_provider_id: Option<String>,
    pub rerank_provider_id: Option<String>,
}

impl AgentKnowledgeContextSelection {
    pub fn new<I, S>(kb_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            kb_ids: kb_ids
                .into_iter()
                .map(Into::into)
                .map(|kb_id| kb_id.trim().to_string())
                .filter(|kb_id| !kb_id.is_empty())
                .collect(),
            ..Self::default()
        }
    }

    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = Some(top_k.max(1));
        self
    }

    pub fn with_embedding_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.embedding_provider_id = non_empty_option(provider_id);
        self
    }

    pub fn with_rerank_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.rerank_provider_id = non_empty_option(provider_id);
        self
    }
}

#[async_trait]
pub trait AgentKnowledgeSelectionPort: Send + Sync {
    async fn selection_for_event(
        &self,
        event: &MessageEvent,
    ) -> Result<Option<AgentKnowledgeContextSelection>>;

    async fn selection_for_session(
        &self,
        _session_id: &str,
    ) -> Result<Option<AgentKnowledgeContextSelection>> {
        Ok(None)
    }
}

#[derive(Clone)]
pub struct KnowledgeRetrievalContextService {
    selection: Arc<dyn AgentKnowledgeSelectionPort>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
    retriever: Arc<dyn KnowledgeRetriever>,
    formatter: Arc<dyn KnowledgeContextFormatter>,
    rerank_provider: Option<Arc<dyn RerankProvider>>,
}

impl KnowledgeRetrievalContextService {
    pub fn new(
        selection: Arc<dyn AgentKnowledgeSelectionPort>,
        embedding_provider: Arc<dyn EmbeddingProvider>,
        retriever: Arc<dyn KnowledgeRetriever>,
    ) -> Self {
        Self {
            selection,
            embedding_provider,
            retriever,
            formatter: Arc::new(RetrievalContextFormatter::default()),
            rerank_provider: None,
        }
    }

    pub fn with_formatter(mut self, formatter: Arc<dyn KnowledgeContextFormatter>) -> Self {
        self.formatter = formatter;
        self
    }

    pub fn with_rerank_provider(mut self, rerank_provider: Arc<dyn RerankProvider>) -> Self {
        self.rerank_provider = Some(rerank_provider);
        self
    }

    pub async fn formatted_context_for_session(
        &self,
        session_id: &str,
        query: &str,
    ) -> Result<Option<String>> {
        let Some(selection) = self.selection.selection_for_session(session_id).await? else {
            return Ok(None);
        };
        self.formatted_context_for_selection(query, selection).await
    }

    async fn formatted_context_for_selection(
        &self,
        query: &str,
        selection: AgentKnowledgeContextSelection,
    ) -> Result<Option<String>> {
        let query = query.trim();
        if query.is_empty() || selection.kb_ids.is_empty() {
            return Ok(None);
        }

        let kb_ids = selection
            .kb_ids
            .into_iter()
            .map(KnowledgeBaseId::new)
            .collect::<Result<Vec<_>>>()?;
        if kb_ids.is_empty() {
            return Ok(None);
        }

        let mut embedding_request = EmbeddingRequest::new(query.to_string());
        if let Some(provider_id) = selection.embedding_provider_id.clone() {
            embedding_request = embedding_request.with_provider_id(provider_id);
        }
        let query_embedding = self
            .embedding_provider
            .embed(embedding_request)
            .await?
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| astrbot_kb::kb_error("embedding provider returned no vector"))?;

        let top_m_final = selection.top_k.unwrap_or(5).clamp(1, 50);
        let results = self
            .retriever
            .retrieve(
                KnowledgeRetrievalRequest::new(query.to_string(), kb_ids)
                    .with_query_embedding(query_embedding)
                    .with_rerank_provider_id(selection.rerank_provider_id.clone())
                    .with_limits(50, 50, top_m_final * 4, top_m_final),
                selection
                    .rerank_provider_id
                    .as_ref()
                    .and_then(|_| self.rerank_provider.clone()),
            )
            .await?;
        if results.is_empty() {
            return Ok(None);
        }

        Ok(Some(self.formatter.format_context(&results)))
    }
}

#[async_trait]
impl AgentKnowledgeContextPort for KnowledgeRetrievalContextService {
    async fn formatted_knowledge_context(&self, event: &MessageEvent) -> Result<Option<String>> {
        let Some(selection) = self.selection.selection_for_event(event).await? else {
            return Ok(None);
        };
        self.formatted_context_for_selection(&event.message_outline(), selection)
            .await
    }
}

pub struct KnowledgeSearchToolExecutor {
    context: KnowledgeRetrievalContextService,
}

impl KnowledgeSearchToolExecutor {
    pub fn new(context: KnowledgeRetrievalContextService) -> Self {
        Self { context }
    }
}

#[async_trait]
impl AgentToolExecutor for KnowledgeSearchToolExecutor {
    async fn execute(
        &self,
        request: AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        if request.descriptor.name != "astr_kb_search" {
            return Err(astrbot_kb::kb_error(format!(
                "knowledge search executor cannot handle tool {}",
                request.descriptor.name
            )));
        }
        let query = request
            .argument("query")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        if query.is_empty() {
            return Err(astrbot_kb::kb_error("astr_kb_search query is required"));
        }
        let Some(context) = self
            .context
            .formatted_context_for_session(&request.session_id, query)
            .await?
        else {
            return Ok(AgentToolExecutionResult::completed(
                "未检索到相关知识库内容。",
            ));
        };
        Ok(AgentToolExecutionResult::completed(context))
    }
}

pub struct KnowledgeContextRequestDecorator {
    context: Arc<dyn AgentKnowledgeContextPort>,
}

impl KnowledgeContextRequestDecorator {
    pub fn new(context: Arc<dyn AgentKnowledgeContextPort>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl ProviderRequestDecorator for KnowledgeContextRequestDecorator {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        let Some(context) = self.context.formatted_knowledge_context(event).await? else {
            return Ok(());
        };
        let context = context.trim();
        if context.is_empty() {
            return Ok(());
        }

        request.system_prompt = Some(match request.system_prompt.take() {
            Some(existing) if !existing.trim().is_empty() => format!("{existing}\n\n{context}"),
            _ => context.to_string(),
        });
        Ok(())
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into().trim().to_string();
    (!value.is_empty()).then_some(value)
}
