use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use super::ManagementApiState;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardCapabilitiesResponse {
    pub services: Vec<DashboardServiceCapability>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardServiceCapability {
    pub id: String,
    pub label: String,
    pub configured: bool,
    pub api_base: String,
    pub closure_level: DashboardClosureLevel,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DashboardClosureLevel {
    Runtime,
    InMemory,
    PlanOnly,
    Unavailable,
}

impl DashboardServiceCapability {
    fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        configured: bool,
        api_base: impl Into<String>,
        closure_level: DashboardClosureLevel,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            configured,
            api_base: api_base.into(),
            closure_level,
            notes: Vec::new(),
        }
    }

    fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

pub async fn capabilities(
    State(state): State<ManagementApiState>,
) -> Json<DashboardCapabilitiesResponse> {
    Json(DashboardCapabilitiesResponse {
        services: vec![
            DashboardServiceCapability::new(
                "status",
                "运行状态",
                true,
                "/api/management/status",
                DashboardClosureLevel::Runtime,
            ),
            DashboardServiceCapability::new(
                "webchat",
                "Chat",
                state.platforms().webchat_platform_count > 0,
                "/api/webchat",
                DashboardClosureLevel::Runtime,
            ),
            DashboardServiceCapability::new(
                "conversation",
                "Conversation",
                state.platforms().webchat_platform_count > 0,
                "/api/webchat/{conversation_id}/messages",
                DashboardClosureLevel::Runtime,
            )
            .with_note("Rust 当前暴露 WebChat conversation history；管理 CRUD 由 conversation_management 能力承接。"),
            DashboardServiceCapability::new(
                "conversation_management",
                "Conversation 管理",
                state.conversations().is_some(),
                "/api/management/conversations",
                if state.conversations().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("Conversation list/create/rename/delete/batch-delete 使用 ConversationService directory。"),
            DashboardServiceCapability::new(
                "openapi_chat",
                "OpenAPI Chat",
                state.platforms().webchat_platform_count > 0 && state.api_keys().is_some(),
                "/api/openapi/chat",
                if state.platforms().webchat_platform_count > 0 && state.api_keys().is_some() {
                    DashboardClosureLevel::Runtime
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("OpenAPI Chat 使用 API key chat scope 鉴权，并兼容旧 openapi.chat scope。"),
            DashboardServiceCapability::new(
                "config",
                "配置",
                state.config_service().is_some(),
                "/api/management/config",
                if state.config_service().is_some() {
                    DashboardClosureLevel::Runtime
                } else {
                    DashboardClosureLevel::Unavailable
                },
            ),
            DashboardServiceCapability::new(
                "providers",
                "Provider 管理",
                state.config_service().is_some(),
                "/api/management/providers",
                if state.config_service().is_some() {
                    DashboardClosureLevel::Runtime
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("Provider catalog/upsert/delete/check 通过 RuntimeConfigService 写入 runtime config。"),
            DashboardServiceCapability::new(
                "platforms",
                "Platform 管理",
                state.config_service().is_some(),
                "/api/management/platforms",
                if state.config_service().is_some() {
                    DashboardClosureLevel::Runtime
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("Platform catalog/upsert/delete/check 通过 RuntimeConfigService 写入 runtime config。"),
            DashboardServiceCapability::new(
                "knowledge_base",
                "知识库",
                state.knowledge_base().is_some(),
                "/api/management/kb",
                if state.knowledge_base().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("当前 CLI 默认接入内存管理服务；持久化索引仍依赖后续 storage backend。"),
            DashboardServiceCapability::new(
                "tools",
                "工具",
                state.tools().is_some(),
                "/api/management/tools",
                if state.tools().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            ),
            DashboardServiceCapability::new(
                "commands",
                "Commands",
                state.config_service().is_some(),
                "/api/management/commands",
                if state.config_service().is_some() {
                    DashboardClosureLevel::Runtime
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("Command catalog/update 通过 RuntimeConfigService 管理 command_plugins 的启停、命令片段和权限。"),
            DashboardServiceCapability::new(
                "mcp",
                "MCP Servers",
                state.mcp().is_some(),
                "/api/management/mcp/servers",
                if state.mcp().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("MCP check/sync 当前只做配置验证和 bridge plan，不执行真实 process/network 探测。"),
            DashboardServiceCapability::new(
                "session_rules",
                "会话规则",
                state.session_rules().is_some(),
                "/api/management/session-rules",
                if state.session_rules().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            ),
            DashboardServiceCapability::new(
                "chat_projects",
                "Chat 项目",
                state.chat_projects().is_some(),
                "/api/management/chat-projects",
                if state.chat_projects().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            ),
            DashboardServiceCapability::new(
                "plugins",
                "插件",
                true,
                "/api/management/plugins",
                DashboardClosureLevel::Runtime,
            )
            .with_note("Handler snapshot 来自 runtime registry；lifecycle/config/upload/source 操作在配置了 plugin lifecycle state 时通过 in-memory 管理面暴露。"),
            DashboardServiceCapability::new(
                "plugin_market",
                "插件市场",
                state.plugin_market().is_some(),
                "/api/management/plugin-market",
                if state.plugin_market().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("当前提供 install/update/uninstall 的内存执行闭环；下载、解包、热加载执行器仍需后续接入。"),
            DashboardServiceCapability::new(
                "skills",
                "Skills",
                state.skills().is_some(),
                "/api/management/skills",
                if state.skills().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            ),
            DashboardServiceCapability::new(
                "api_keys",
                "API Keys",
                state.api_keys().is_some(),
                "/api/management/api-keys",
                if state.api_keys().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("当前暴露 API key list/issue/revoke 管理闭环；secret 只在签发响应中返回一次。"),
            DashboardServiceCapability::new(
                "subagent",
                "SubAgent",
                state.subagents().is_some(),
                "/api/management/subagents",
                if state.subagents().is_some() {
                    DashboardClosureLevel::Runtime
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note(
                "暴露 source-compatible SubAgent 配置读写、handoff 预览与 execution bridge 接口；CLI 默认配置持久化到 main.sqlite。",
            ),
            DashboardServiceCapability::new(
                "backup",
                "备份",
                state.backup().is_some(),
                "/api/management/backup",
                if state.backup().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("当前 CLI 默认接入内存 export/import job 与上传会话闭环；真实文件打包依赖后续 repository backend。"),
            DashboardServiceCapability::new(
                "maintenance",
                "更新维护",
                state.maintenance().is_some(),
                "/api/management/update",
                if state.maintenance().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("当前提供更新、迁移、包安装的内存 operation 执行闭环；真实升级/安装命令执行器仍需后续接入。"),
            DashboardServiceCapability::new(
                "console",
                "Console",
                state.observability().is_some(),
                "/api/management/logs",
                if let Some(observability) = state.observability() {
                    if observability.has_log_store() {
                        DashboardClosureLevel::Runtime
                    } else {
                        DashboardClosureLevel::InMemory
                    }
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("兼容 /api/live-log 与 /api/log-history；CLI 默认使用 JSONL-backed log history 并保留 management SSE。"),
            DashboardServiceCapability::new(
                "trace",
                "Trace",
                state.observability().is_some(),
                "/api/management/trace",
                if let Some(observability) = state.observability() {
                    if observability.has_trace_settings_store() {
                        DashboardClosureLevel::Runtime
                    } else {
                        DashboardClosureLevel::InMemory
                    }
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("兼容 /api/trace/settings；trace 设置持久化，trace 输出应用 outline/redact/max-events 策略。"),
            DashboardServiceCapability::new(
                "stats",
                "统计",
                state.observability().is_some(),
                "/api/management/stats",
                if state.observability().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("当前聚合 management metric events；持久化统计与实时图表仍需后续绑定。"),
            DashboardServiceCapability::new(
                "personas",
                "Persona",
                state.personas().is_some(),
                "/api/management/personas",
                if state.personas().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            ),
            DashboardServiceCapability::new(
                "cron",
                "Cron",
                state.cron().is_some(),
                "/api/management/cron",
                if state.cron().is_some() {
                    DashboardClosureLevel::InMemory
                } else {
                    DashboardClosureLevel::Unavailable
                },
            )
            .with_note("当前 CLI 默认接入内存 scheduler，并通过 runtime lifecycle 执行 run_once 到期唤醒。"),
        ],
    })
}
