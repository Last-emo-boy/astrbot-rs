use astrbot_tool::{ToolCallReferencePayload, ToolReferenceExtractor, ToolReferenceSet};
use serde::{Deserialize, Serialize};

use crate::AgentResponseEvent;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResponseReferences {
    #[serde(default)]
    pub tool_refs: ToolReferenceSet,
}

impl AgentResponseReferences {
    pub fn new(tool_refs: ToolReferenceSet) -> Self {
        Self { tool_refs }
    }

    pub fn is_empty(&self) -> bool {
        self.tool_refs.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentReferenceDecorator {
    extractor: ToolReferenceExtractor,
}

impl AgentReferenceDecorator {
    pub fn new(extractor: ToolReferenceExtractor) -> Self {
        Self { extractor }
    }

    pub fn decorate_text(
        &self,
        response_text: &str,
        tool_calls: &[ToolCallReferencePayload],
    ) -> AgentResponseReferences {
        AgentResponseReferences::new(
            self.extractor
                .extract_from_tool_calls(response_text, tool_calls),
        )
    }

    pub fn decorate_event(
        &self,
        event: AgentResponseEvent,
        tool_calls: &[ToolCallReferencePayload],
    ) -> AgentResponseEvent {
        let references = self.decorate_text(&event.chain.plain_text(), tool_calls);
        event.with_references(references)
    }
}
