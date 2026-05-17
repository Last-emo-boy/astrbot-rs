use astrbot_core::ProviderRequest;

use crate::MemoryTranscriptRecord;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryRequestMode {
    PassiveContext,
    ActiveReply,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryPromptPlan {
    pub mode: MemoryRequestMode,
    pub history: String,
}

impl MemoryPromptPlan {
    pub fn apply_to_request(&self, request: &mut ProviderRequest) {
        if self.history.trim().is_empty() {
            return;
        }

        match self.mode {
            MemoryRequestMode::PassiveContext => {
                let addition = format!(
                    "You are now in a chatroom. The chat history is as follows: \n{}",
                    self.history
                );
                request.system_prompt = Some(match request.system_prompt.take() {
                    Some(existing) if !existing.trim().is_empty() => {
                        format!("{existing}\n{addition}")
                    }
                    _ => addition,
                });
            }
            MemoryRequestMode::ActiveReply => {
                let prompt = request.prompt.take().unwrap_or_default();
                request.prompt = Some(format!(
                    "You are now in a chatroom. The chat history is as follows:\n{}\
                     \nNow, a new message is coming: `{}`. \
                     Please react to it. Only output your response and do not output any other information. \
                     You MUST use the SAME language as the chatroom is using.",
                    self.history, prompt
                ));
                request.contexts.clear();
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryPromptPolicy {
    pub separator: String,
}

impl MemoryPromptPolicy {
    pub fn new() -> Self {
        Self {
            separator: "\n---\n".to_string(),
        }
    }

    pub fn build_plan(
        &self,
        records: &[MemoryTranscriptRecord],
        mode: MemoryRequestMode,
    ) -> Option<MemoryPromptPlan> {
        let history = records
            .iter()
            .map(|record| record.content.trim())
            .filter(|content| !content.is_empty())
            .collect::<Vec<_>>()
            .join(&self.separator);
        (!history.is_empty()).then_some(MemoryPromptPlan { mode, history })
    }
}

impl Default for MemoryPromptPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use astrbot_core::{ProviderContextMessage, ProviderRequest};

    use super::{MemoryPromptPolicy, MemoryRequestMode};
    use crate::{MemorySessionKey, MemoryTranscriptRecord};

    #[test]
    fn passive_memory_plan_appends_history_to_system_prompt() {
        let session = MemorySessionKey::new("webchat", "room-1");
        let records = vec![MemoryTranscriptRecord::new(
            session,
            "Alice",
            "[Alice]: hello",
        )];
        let plan = MemoryPromptPolicy::new()
            .build_plan(&records, MemoryRequestMode::PassiveContext)
            .expect("memory plan should build");
        let mut request = ProviderRequest::new("question", "room-1").with_system_prompt("persona");

        plan.apply_to_request(&mut request);

        assert!(
            request
                .system_prompt
                .as_deref()
                .expect("system prompt should exist")
                .contains("[Alice]: hello")
        );
        assert_eq!(request.prompt.as_deref(), Some("question"));
    }

    #[test]
    fn active_reply_plan_rewrites_prompt_and_clears_contexts() {
        let session = MemorySessionKey::new("webchat", "room-1");
        let records = vec![MemoryTranscriptRecord::new(
            session,
            "Alice",
            "[Alice]: hello",
        )];
        let plan = MemoryPromptPolicy::new()
            .build_plan(&records, MemoryRequestMode::ActiveReply)
            .expect("memory plan should build");
        let mut request = ProviderRequest::new("new message", "room-1")
            .with_context(ProviderContextMessage::text("user", "old"));

        plan.apply_to_request(&mut request);

        assert!(request.contexts.is_empty());
        let prompt = request.prompt.as_deref().expect("prompt should exist");
        assert!(prompt.contains("[Alice]: hello"));
        assert!(prompt.contains("new message"));
    }
}
