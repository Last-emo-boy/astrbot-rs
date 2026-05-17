use serde_json::{Value, json};

use crate::{ToolCatalog, ToolDescriptor, ToolSourceMetadata};

#[derive(Clone, Debug, PartialEq)]
pub struct InternalToolRegistration {
    pub descriptor: ToolDescriptor,
    pub provider_id: String,
    pub handler_name: Option<String>,
}

impl InternalToolRegistration {
    pub fn new(
        provider_id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        let provider_id = provider_id.into();
        Self {
            descriptor: ToolDescriptor::new(name)
                .with_description(description)
                .with_parameters(parameters)
                .with_source_metadata(ToolSourceMetadata::internal_provider(
                    provider_id.clone(),
                    "AstrBot",
                )),
            provider_id,
            handler_name: None,
        }
    }

    pub fn with_handler_name(mut self, handler_name: impl Into<String>) -> Self {
        let handler_name = handler_name.into();
        self.handler_name = (!handler_name.trim().is_empty()).then_some(handler_name);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InternalToolProviderDescriptor {
    pub provider_id: String,
    pub module_path: String,
    registrations: Vec<InternalToolRegistration>,
}

impl InternalToolProviderDescriptor {
    pub fn new(provider_id: impl Into<String>, module_path: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            module_path: module_path.into(),
            registrations: Vec::new(),
        }
    }

    pub fn with_registration(mut self, registration: InternalToolRegistration) -> Self {
        self.registrations.push(registration);
        self.registrations
            .sort_by(|left, right| left.descriptor.name.cmp(&right.descriptor.name));
        self
    }

    pub fn registrations(&self) -> &[InternalToolRegistration] {
        &self.registrations
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InternalToolProviderCatalog {
    providers: Vec<InternalToolProviderDescriptor>,
}

impl InternalToolProviderCatalog {
    pub fn new(providers: Vec<InternalToolProviderDescriptor>) -> Self {
        let mut catalog = Self { providers };
        catalog
            .providers
            .sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        catalog
    }

    pub fn providers(&self) -> &[InternalToolProviderDescriptor] {
        &self.providers
    }

    pub fn registrations(&self) -> Vec<InternalToolRegistration> {
        self.providers
            .iter()
            .flat_map(|provider| provider.registrations.iter().cloned())
            .collect()
    }

    pub fn extend_tool_catalog(&self, catalog: &mut ToolCatalog) {
        for registration in self.registrations() {
            catalog.add_tool(registration.descriptor);
        }
    }

    pub fn into_tool_catalog(self) -> ToolCatalog {
        let mut catalog = ToolCatalog::new();
        self.extend_tool_catalog(&mut catalog);
        catalog
    }
}

pub fn builtin_internal_tool_catalog() -> InternalToolProviderCatalog {
    InternalToolProviderCatalog::new(vec![
        cron_provider(),
        knowledge_base_provider(),
        send_message_provider(),
        computer_use_provider(),
    ])
}

pub fn builtin_internal_tool_registrations() -> Vec<InternalToolRegistration> {
    builtin_internal_tool_catalog().registrations()
}

fn cron_provider() -> InternalToolProviderDescriptor {
    InternalToolProviderDescriptor::new("cron", "astrbot.core.tools.cron_tools")
        .with_registration(
            InternalToolRegistration::new(
                "cron",
                "create_future_task",
                "Create a future task for scheduled follow-up or proactive actions.",
                json!({
                    "type": "object",
                    "properties": {
                        "cron_expression": {"type": "string"},
                        "run_at": {"type": "string"},
                        "note": {"type": "string"},
                        "name": {"type": "string"},
                        "run_once": {"type": "boolean"}
                    },
                    "required": ["note"]
                }),
            )
            .with_handler_name("CreateActiveCronTool"),
        )
        .with_registration(
            InternalToolRegistration::new(
                "cron",
                "delete_future_task",
                "Delete a future task by job id.",
                json!({
                    "type": "object",
                    "properties": {
                        "job_id": {"type": "string"}
                    },
                    "required": ["job_id"]
                }),
            )
            .with_handler_name("DeleteCronJobTool"),
        )
        .with_registration(
            InternalToolRegistration::new(
                "cron",
                "list_future_tasks",
                "List existing future tasks for inspection.",
                json!({
                    "type": "object",
                    "properties": {
                        "job_type": {"type": "string"}
                    }
                }),
            )
            .with_handler_name("ListCronJobsTool"),
        )
}

fn knowledge_base_provider() -> InternalToolProviderDescriptor {
    InternalToolProviderDescriptor::new("knowledge_base", "astrbot.core.tools.kb_query")
        .with_registration(
            InternalToolRegistration::new(
                "knowledge_base",
                "astr_kb_search",
                "Query the knowledge base for facts or relevant context.",
                json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                }),
            )
            .with_handler_name("KnowledgeBaseQueryTool"),
        )
}

fn send_message_provider() -> InternalToolProviderDescriptor {
    InternalToolProviderDescriptor::new("send_message", "astrbot.core.tools.send_message")
        .with_registration(
            InternalToolRegistration::new(
                "send_message",
                "send_message_to_user",
                "Directly send a proactive message to the user.",
                json!({
                    "type": "object",
                    "properties": {
                        "messages": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "type": {"type": "string"},
                                    "text": {"type": "string"},
                                    "path": {"type": "string"},
                                    "url": {"type": "string"},
                                    "mention_user_id": {"type": "string"}
                                },
                                "required": ["type"]
                            }
                        }
                    },
                    "required": ["messages"]
                }),
            )
            .with_handler_name("SendMessageToUserTool"),
        )
}

fn computer_use_provider() -> InternalToolProviderDescriptor {
    InternalToolProviderDescriptor::new(
        "computer_use",
        "astrbot.core.computer.computer_tool_provider",
    )
    .with_registration(empty_computer_registration(
        "astrbot_execute_shell",
        "Execute a command in the configured computer-use shell.",
    ))
    .with_registration(empty_computer_registration(
        "astrbot_execute_ipython",
        "Run code in the configured computer-use Python environment.",
    ))
    .with_registration(empty_computer_registration(
        "astrbot_upload_file",
        "Upload a local file to the sandbox filesystem.",
    ))
    .with_registration(empty_computer_registration(
        "astrbot_download_file",
        "Download a file from the sandbox filesystem.",
    ))
    .with_registration(empty_computer_registration(
        "astrbot_execute_browser",
        "Execute one browser automation command in the sandbox.",
    ))
    .with_registration(empty_computer_registration(
        "astrbot_execute_browser_batch",
        "Execute a browser command batch in the sandbox.",
    ))
    .with_registration(empty_computer_registration(
        "astrbot_run_browser_skill",
        "Run a released browser skill in the sandbox by skill key.",
    ))
}

fn empty_computer_registration(
    name: impl Into<String>,
    description: impl Into<String>,
) -> InternalToolRegistration {
    InternalToolRegistration::new(
        "computer_use",
        name,
        description,
        json!({
            "type": "object",
            "properties": {}
        }),
    )
}
