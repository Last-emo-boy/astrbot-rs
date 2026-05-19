# AstroBot Rust Dashboard Backend — HTTP/SSE/WS Endpoint Map

**Survey Date:** May 2026  
**Scope:** `crates/astrbot-web/src/` (all management, routes, openapi, realtime modules)  
**Framework:** Axum + Tokio

---

## Overview

The backend exposes three main router tiers:
- **`webchat_router`** — Legacy chat API + WebSocket (routes.rs)
- **`management_router`** — Admin dashboard management (management/mod.rs)
- **`openapi_chat_router`** — API v1 OpenAPI chat + realtime subscriptions (openapi.rs)
- **`dashboard_router`** — Static asset serving

All management endpoints require auth middleware (`require_management_auth`).  
OpenAPI endpoints require Bearer token validation against API key store.

---

## I. WebChat Endpoints (Text + WebSocket)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth Scope |
|---|---|---|---|---|---|
| POST | `/api/webchat/{conversation_id}` | `submit_text` | `SubmitTextRequest` | `SubmitTextResponse` | None |
| GET | `/api/webchat/{conversation_id}/messages` | `list_messages` | — | `WebChatMessagesResponse` | None |
| POST | `/api/chat/send` | `legacy_send` | `Value` (JSON) | `Value` | None |
| GET | `/api/chat/new_session` | `legacy_new_session` | — | `Value` | None |
| GET | `/api/chat/sessions` | `legacy_sessions` | — | `Value` | None |
| GET | `/api/chat/get_session` | `legacy_get_session` | Query: `session_id` | `Value` | None |
| GET | `/api/chat/delete_session` | `legacy_delete_session` | Query: `session_id` | `Value` | None |
| POST | `/api/chat/batch_delete_sessions` | `legacy_batch_delete_sessions` | `Value` | `Value` | None |
| POST | `/api/chat/update_session_display_name` | `legacy_update_session_display_name` | `Value` | `Value` | None |
| POST | `/api/chat/stop` | `legacy_stop` | `Value` | `Value` | None |
| POST | `/api/chat/respond_elicitation` | `legacy_respond_elicitation` | `Value` | `Value` | None |
| POST | `/api/chat/post_file` | `legacy_post_file` | `Multipart` | `Value` | None |
| GET | `/api/chat/get_attachment` | `legacy_get_attachment` | Query: `attachment_id` | bytes + headers | None |
| GET | `/api/chat/get_file` | `legacy_get_file` | Query: `filename` | bytes + headers | None |
| **WS** | `/api/live_chat/ws` | `live_chat_ws` | WS messages (JSON) | WS frames | Query: `token` |
| **WS** | `/api/unified_chat/ws` | `unified_chat_ws` | WS messages (JSON) | WS frames | None |

**Source:** `crates/astrbot-web/src/routes.rs:67–102`

---

## II. OpenAPI Chat Endpoints (JSON + Realtime + WS)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth Scope |
|---|---|---|---|---|---|
| POST | `/api/openapi/chat` | `chat` | `OpenApiChatMessageRequest` | `OpenApiChatHttpResponse` | Bearer token (chat scope) |
| POST | `/api/v1/chat` | `chat` | `OpenApiChatMessageRequest` | `OpenApiChatHttpResponse` | Bearer token |
| GET | `/api/openapi/chat/subscriptions` | `subscriptions` | — | `RealtimeSubscriptionCatalogResponse` | Bearer token |
| GET | `/api/v1/chat/sessions` | `v1_chat_sessions` | — | `Value` (wrapped) | Bearer token |
| GET | `/api/openapi/chat/subscriptions/{request_id}` | `subscription` | Path: `request_id` | `RealtimeChatSubscriptionRecord` | Bearer token |
| **WS** | `/api/v1/chat/ws` | `v1_chat_ws` | WS frames | WS frames | Bearer token |
| POST | `/api/openapi/chat/stop` | `stop_chat` | `RealtimeStopRequest` | `RealtimeStopResponse` | Bearer token |
| GET | `/api/openapi/elicitation` | `elicitations` | — | `RealtimeElicitationCatalogResponse` | Bearer token |
| POST | `/api/openapi/elicitation` | `create_elicitation` | `RealtimeElicitationCreateRequest` | `RealtimeElicitationRecord` | Bearer token |
| POST | `/api/openapi/elicitation/respond` | `respond_elicitation` | `RealtimeElicitationRespondRequest` | `RealtimeElicitationRecord` | Bearer token |

**Source:** `crates/astrbot-web/src/openapi.rs:65–100`

---

## III. Management API Endpoints (Dashboard Admin)

### III.A Authentication & Authorization

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth Required |
|---|---|---|---|---|---|
| POST | `/api/auth/login` | `auth::login` | `DashboardLoginRequest` | `DashboardAuthResponse` | No |
| POST | `/api/auth/account/edit` | `auth::edit_account` | `DashboardAccountEditRequest` | `DashboardAuthResponse` | No (embedded auth) |

**Source:** `crates/astrbot-web/src/management/auth.rs` + `mod.rs:689–694`

---

### III.B Providers (LLM, Embedding, etc.)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/providers` | `providers` | — | `ProviderManagementResponse` | Yes |
| GET | `/api/management/providers/catalog` | `providers::catalog` | — | `ManagementProviderCatalogResponse` | Yes |
| POST | `/api/management/providers/upsert` | `providers::upsert` | `ManagementProviderUpsertRequest` | `ManagementProviderMutationResponse` | Yes |
| POST | `/api/management/providers/delete` | `providers::delete` | `ManagementProviderDeleteRequest` | `ManagementProviderMutationResponse` | Yes |
| POST | `/api/management/providers/check` | `providers::check` | `ManagementProviderCheckRequest` | `ManagementProviderCheckResponse` | Yes |
| POST | `/api/management/providers/models` | `providers::models` | `ManagementProviderModelsRequest` | `ManagementProviderModelsResponse` | Yes |

**Legacy:** `/api/config/provider/*` (6 endpoints)

**Source:** `crates/astrbot-web/src/management/providers.rs:341–840`

---

### III.C Platforms (Discord, Telegram, QQ, etc.)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/platforms` | `platforms` | — | `PlatformManagementResponse` | Yes |
| GET | `/api/management/platforms/catalog` | `platforms::catalog` | — | `ManagementPlatformCatalogResponse` | Yes |
| POST | `/api/management/platforms/upsert` | `platforms::upsert` | `ManagementPlatformUpsertRequest` | `ManagementPlatformMutationResponse` | Yes |
| POST | `/api/management/platforms/delete` | `platforms::delete` | `ManagementPlatformDeleteRequest` | `ManagementPlatformMutationResponse` | Yes |
| POST | `/api/management/platforms/check` | `platforms::check` | `ManagementPlatformCheckRequest` | `ManagementPlatformCheckResponse` | Yes |
| GET/POST | `/api/platform/webhook/{webhook_uuid}` | `platforms::legacy_webhook` | JSON | JSON | None |
| GET | `/api/platform/stats` | `platforms::legacy_stats` | — | JSON | None |

**Legacy:** `/api/config/platform/*` (3 endpoints)

**Source:** `crates/astrbot-web/src/management/platforms.rs`

---

### III.D Plugins (Install, Config, Lifecycle)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/plugins` | `plugins` | — | `PluginManagementResponse` | Yes |
| GET | `/api/management/plugins/lifecycle` | `plugins::lifecycle_catalog` | — | `ManagementPluginLifecycleCatalogResponse` | Yes |
| POST | `/api/management/plugins/lifecycle/action` | `plugins::lifecycle_action` | `ManagementPluginLifecycleActionRequest` | `ManagementPluginLifecycleMutationResponse` | Yes |
| POST | `/api/management/plugins/upload-plan` | `plugins::upload_plan` | `ManagementPluginUploadPlanRequest` | `ManagementPluginUploadPlanResponse` | Yes |
| POST | `/api/management/plugins/source-plan` | `plugins::source_plan` | `ManagementPluginSourcePlanRequest` | `ManagementPluginSourcePlanResponse` | Yes |
| POST | `/api/management/plugins/config` | `plugins::save_config` | `ManagementPluginConfigRequest` | `ManagementPluginConfigRequest` | Yes |
| POST | `/api/management/plugins/config-file/list` | `plugins::list_config_files` | `ManagementPluginConfigFileListRequest` | `ManagementPluginConfigFileListResponse` | Yes |
| POST | `/api/management/plugins/config-file/read` | `plugins::read_config_file` | `ManagementPluginConfigFileRequest` | `ManagementPluginConfigFileReadResponse` | Yes |
| POST | `/api/management/plugins/config-file/write` | `plugins::write_config_file` | `ManagementPluginConfigFileWriteRequest` | `ManagementPluginConfigFileWriteResponse` | Yes |
| POST | `/api/management/plugins/config-file/delete` | `plugins::delete_config_file` | `ManagementPluginConfigFileRequest` | `ManagementPluginConfigFileDeleteResponse` | Yes |

**Legacy Plugin Market:** `/api/plugin/*` (12 endpoints)

**Source:** `crates/astrbot-web/src/management/plugins.rs` + `plugin_market.rs`

---

### III.E Knowledge Base (RAG)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/kb/catalog` | `knowledge_base::catalog` | — | `ManagementKnowledgeBaseCatalogResponse` | Yes |
| POST | `/api/management/kb/create` | `knowledge_base::create` | `ManagementKnowledgeBaseCreateRequest` | `ManagementKnowledgeBaseResponse` | Yes |
| POST | `/api/management/kb/get` | `knowledge_base::get` | `ManagementKnowledgeBaseIdRequest` | `ManagementKnowledgeBaseResponse` | Yes |
| POST | `/api/management/kb/update` | `knowledge_base::update` | `ManagementKnowledgeBaseUpdateRequest` | `ManagementKnowledgeBaseCatalogResponse` | Yes |
| POST | `/api/management/kb/delete` | `knowledge_base::delete` | `ManagementKnowledgeBaseIdRequest` | `ManagementKnowledgeMutationResponse` | Yes |
| POST | `/api/management/kb/preflight` | `knowledge_base::preflight` | `ManagementKnowledgeProviderPreflightRequest` | `ManagementKnowledgePreflightResponse` | Yes |
| POST | `/api/management/kb/document/list` | `knowledge_base::list_documents` | `ManagementKnowledgeDocumentIdRequest` | `ManagementKnowledgeDocumentCatalogResponse` | Yes |
| POST | `/api/management/kb/document/get` | `knowledge_base::get_document` | `ManagementKnowledgeDocumentIdRequest` | `ManagementKnowledgeDocumentCatalogResponse` | Yes |
| POST | `/api/management/kb/document/delete` | `knowledge_base::delete_document` | `ManagementKnowledgeDocumentIdRequest` | `ManagementKnowledgeMutationResponse` | Yes |
| POST | `/api/management/kb/chunk/list` | `knowledge_base::list_chunks` | `ManagementKnowledgeBaseIdRequest` | `ManagementKnowledgeChunkCatalogResponse` | Yes |
| POST | `/api/management/kb/chunk/delete` | `knowledge_base::delete_chunk` | `ManagementKnowledgeChunkDeleteRequest` | `ManagementKnowledgeMutationResponse` | Yes |
| POST | `/api/management/kb/retrieve` | `knowledge_base::retrieve` | `ManagementKnowledgeIngestRequest` | `ManagementKnowledgeIngestResponse` | Yes |
| POST | `/api/management/kb/ingest` | `knowledge_base::ingest` | `ManagementKnowledgeIngestRequest` | `ManagementKnowledgeIngestResponse` | Yes |
| POST | `/api/management/kb/upload/plan` | `knowledge_base::plan_upload` | `ManagementKnowledgeUploadPlanRequest` | `ManagementKnowledgeUploadTaskResponse` | Yes |
| POST | `/api/management/kb/upload/progress` | `knowledge_base::update_upload_progress` | `ManagementKnowledgeUploadProgressRequest` | `ManagementKnowledgeUploadTaskResponse` | Yes |
| POST | `/api/management/kb/upload/complete` | `knowledge_base::complete_upload` | `ManagementKnowledgeUploadCompleteRequest` | `ManagementKnowledgeMutationResponse` | Yes |
| POST | `/api/management/kb/upload/fail` | `knowledge_base::fail_upload` | `ManagementKnowledgeUploadFailRequest` | `ManagementKnowledgeMutationResponse` | Yes |
| GET | `/api/management/kb/upload/progress/{task_id}` | `knowledge_base::upload_progress` | Path: `task_id` | `ManagementKnowledgeUploadTaskResponse` | Yes |

**Legacy:** `/api/kb/*` (9 endpoints)

**Source:** `crates/astrbot-web/src/management/knowledge_base.rs`

---

### III.F Observability (Logs, Traces, Metrics)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/logs` | `observability::logs` | Query: `level`, `limit` | `ManagementLogResponse` | Yes |
| **SSE** | `/api/management/logs/stream` | `observability::logs_stream` | Query: `level` | `Sse<Event>` (text/event-stream) | Yes |
| GET | `/api/management/trace` | `observability::trace` | — | `ManagementTraceResponse` | Yes |
| GET | `/api/management/trace/settings` | `observability::trace_settings` | — | `Json<ManagementTraceSettings>` | Yes |
| POST | `/api/management/trace/settings` | `observability::update_trace_settings` | `ManagementTraceSettings` | `Json<ManagementTraceSettings>` | Yes |
| GET | `/api/management/stats` | `observability::stats` | — | `ManagementStatsResponse` | Yes |
| POST | `/api/management/stats/push` | `observability::push_metric` | `MetricEvent` | `ManagementStatsResponse` | Yes |
| POST | `/api/management/logs/push` | `observability::push_log` | `LogEntry` | `ManagementLogResponse` | Yes |

**Legacy:** `/api/live-log`, `/api/log-history`, `/api/trace/settings/*` (5 endpoints)

**Source:** `crates/astrbot-web/src/management/observability.rs:481–773`

---

### III.G Personas (Bot Profiles / System Prompts)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| POST | `/api/management/personas` | `persona::catalog` | `ManagementPersonaListRequest` | `ManagementPersonaCatalogResponse` | Yes |
| POST | `/api/management/personas/upsert` | `persona::upsert` | `ManagementPersonaUpsertRequest` | `ManagementPersonaMutationResponse` | Yes |
| POST | `/api/management/personas/delete` | `persona::delete` | `ManagementPersonaUpsertRequest` | `ManagementPersonaMutationResponse` | Yes |
| POST | `/api/management/personas/move` | `persona::move_persona` | `ManagementPersonaUpsertRequest` | `ManagementPersonaMutationResponse` | Yes |
| POST | `/api/management/personas/clone` | `persona::clone_persona` | `ManagementPersonaUpsertRequest` | `ManagementPersonaMutationResponse` | Yes |
| POST | `/api/management/personas/reorder` | `persona::reorder` | `Value` | `ManagementPersonaCatalogResponse` | Yes |
| POST | `/api/management/personas/folders/upsert` | `persona::upsert_folder` | `ManagementPersonaFolderUpsertRequest` | `ManagementPersonaMutationResponse` | Yes |
| POST | `/api/management/personas/folders/delete` | `persona::delete_folder` | `ManagementPersonaUpsertRequest` | `ManagementPersonaMutationResponse` | Yes |
| POST | `/api/management/personas/folders/move` | `persona::move_folder` | `ManagementPersonaUpsertRequest` | `ManagementPersonaMutationResponse` | Yes |
| POST | `/api/management/personas/resolve` | `persona::resolve` | `ManagementPersonaResolveRequest` | `ManagementPersonaResolveResponse` | Yes |

**Legacy:** `/api/persona/*` (10 endpoints)

**Source:** `crates/astrbot-web/src/management/persona.rs`

---

### III.H Conversations & Chat Projects

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| POST | `/api/management/conversations` | `conversations::list` | `ManagementConversationListRequest` | `ManagementConversationCatalogResponse` | Yes |
| POST | `/api/management/conversations/get` | `conversations::get` | `ManagementConversationGetRequest` | `ManagementConversationResponse` | Yes |
| POST | `/api/management/conversations/upsert` | `conversations::upsert` | `ManagementConversationUpsertRequest` | `ManagementConversationMutationResponse` | Yes |
| POST | `/api/management/conversations/rename` | `conversations::rename` | `ManagementConversationRenameRequest` | `ManagementConversationMutationResponse` | Yes |
| POST | `/api/management/conversations/current` | `conversations::current` | `ManagementConversationCurrentRequest` | `ManagementConversationCurrentResponse` | Yes |
| POST | `/api/management/conversations/delete` | `conversations::delete` | `ManagementConversationDeleteRequest` | `ManagementConversationDeleteResponse` | Yes |
| POST | `/api/management/conversations/batch-delete` | `conversations::batch_delete` | `ManagementConversationBatchDeleteRequest` | `ManagementConversationBatchDeleteResponse` | Yes |
| POST | `/api/management/chat-projects` | `chat_projects::list` | — | `ManagementChatProjectCatalogResponse` | Yes |
| POST | `/api/management/chat-projects/create` | `chat_projects::create` | `ManagementChatProjectCreateRequest` | `ManagementChatProjectMutationResponse` | Yes |
| POST | `/api/management/chat-projects/get` | `chat_projects::get` | `ManagementChatProjectGetRequest` | `ManagementChatProjectResponse` | Yes |
| POST | `/api/management/chat-projects/update` | `chat_projects::update` | `ManagementChatProjectUpdateRequest` | `ManagementChatProjectMutationResponse` | Yes |
| POST | `/api/management/chat-projects/delete` | `chat_projects::delete` | `ManagementChatProjectGetRequest` | `ManagementChatProjectMutationResponse` | Yes |
| POST | `/api/management/chat-projects/sessions/upsert` | `chat_projects::upsert_session` | `ManagementChatProjectSessionUpsertRequest` | `ManagementChatProjectSessionResponse` | Yes |
| POST | `/api/management/chat-projects/sessions` | `chat_projects::sessions` | `ManagementChatProjectGetRequest` | `ManagementChatProjectSessionsResponse` | Yes |

**Legacy:** `/api/conversation/*`, `/api/chatui_project/*` (10 endpoints)

**Source:** `crates/astrbot-web/src/management/conversations.rs`, `chat_projects.rs`

---

### III.I Cron Jobs

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| POST | `/api/management/cron/jobs` | `cron::list` | `ManagementCronListRequest` | `ManagementCronCatalogResponse` | Yes |
| POST | `/api/management/cron/jobs/upsert` | `cron::upsert` | `ManagementCronUpsertRequest` | `ManagementCronMutationResponse` | Yes |
| POST | `/api/management/cron/jobs/run` | `cron::run` | `ManagementCronJobRequest` | `ManagementCronTickResponse` | Yes |
| POST | `/api/management/cron/tick` | `cron::tick` | `ManagementCronTickRequest` | `ManagementCronTickResponse` | Yes |
| POST | `/api/management/cron/jobs/delete` | `cron::delete` | `ManagementCronJobRequest` | `ManagementCronDeleteResponse` | Yes |
| POST | `/api/management/cron/start` | `cron::start` | — | `ManagementCronMutationResponse` | Yes |
| POST | `/api/management/cron/shutdown` | `cron::shutdown` | — | `ManagementCronMutationResponse` | Yes |

**Legacy:** `/api/cron/*` (6 endpoints)

**Source:** `crates/astrbot-web/src/management/cron.rs`

---

### III.J Session Rules & Groups

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/session-rules` | `session_rules::list_rules` | — | `ManagementSessionRuleState` | Yes |
| POST | `/api/management/session-rules/update` | `session_rules::update_rule` | `SessionRule` | response | Yes |
| POST | `/api/management/session-rules/delete` | `session_rules::delete_rule` | `SessionRule` | response | Yes |
| POST | `/api/management/session-rules/batch-service` | `session_rules::batch_update_service` | batch req | response | Yes |
| POST | `/api/management/session-rules/batch-provider` | `session_rules::batch_update_provider` | batch req | response | Yes |
| GET | `/api/management/session-rules/groups` | `session_rules::list_groups` | — | `Vec<Group>` | Yes |
| POST | `/api/management/session-rules/groups/upsert` | `session_rules::upsert_group` | `Group` | response | Yes |
| POST | `/api/management/session-rules/groups/patch` | `session_rules::patch_group` | `Group` | response | Yes |
| POST | `/api/management/session-rules/groups/delete` | `session_rules::delete_group` | id | response | Yes |

**Legacy:** `/api/session/*` (9 endpoints)

**Source:** `crates/astrbot-web/src/management/session_rules.rs`

---

### III.K API Keys & Authorization

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/api-keys` | `api_key::catalog` | — | `ManagementApiKeyCatalogResponse` | Yes |
| POST | `/api/management/api-keys/issue` | `api_key::issue` | `ManagementApiKeyIssueRequest` | `ManagementApiKeyIssueResponse` | Yes |
| POST | `/api/management/api-keys/revoke` | `api_key::revoke` | `ManagementApiKeyRevokeRequest` | `ManagementApiKeyRevokeResponse` | Yes |
| POST | `/api/management/api-keys/delete` | `api_key::delete` | `ManagementApiKeyDeleteRequest` | `ManagementApiKeyDeleteResponse` | Yes |

**Legacy:** `/api/v1/apikeys/*`, `/api/apikey/*` (8 endpoints)

**Source:** `crates/astrbot-web/src/management/api_key.rs`

---

### III.L Configuration (abconfs, Routes)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/config/current` | `config::current` | — | `ManagementConfigCurrentResponse` | Yes |
| GET | `/api/management/config/schema` | `config::schema` | — | `ManagementConfigSchemaResponse` | Yes |
| GET | `/api/management/config/abconfs` | `config::abconf_catalog` | — | `ManagementAbconfCatalogResponse` | Yes |
| POST | `/api/management/config/abconfs/create` | `config::abconf_create` | `ManagementAbconfCreateRequest` | `ManagementAbconfCreateResponse` | Yes |
| POST | `/api/management/config/abconfs/get` | `config::abconf_get` | `ManagementAbconfIdRequest` | `ManagementAbconfResponse` | Yes |
| POST | `/api/management/config/abconfs/update` | `config::abconf_update` | `ManagementAbconfUpdateRequest` | `ManagementAbconfResponse` | Yes |
| POST | `/api/management/config/abconfs/delete` | `config::abconf_delete` | `ManagementAbconfIdRequest` | `ManagementAbconfDeleteResponse` | Yes |
| GET | `/api/management/config/routes` | `config::route_catalog` | — | `ManagementConfigRouteCatalogResponse` | Yes |
| POST | `/api/management/config/routes/upsert` | `config::route_upsert` | `ManagementConfigRouteUpsertRequest` | `ManagementConfigRouteMutationResponse` | Yes |
| POST | `/api/management/config/routes/delete` | `config::route_delete` | `ManagementConfigRouteDeleteRequest` | `ManagementConfigRouteMutationResponse` | Yes |
| POST | `/api/management/config/routes/replace` | `config::route_replace` | `ManagementConfigRouteReplaceRequest` | `ManagementConfigRouteMutationResponse` | Yes |
| POST | `/api/management/config/routes/resolve` | `config::route_resolve` | `ManagementConfigRouteResolveRequest` | `ManagementConfigRouteResolveResponse` | Yes |
| POST | `/api/management/config/preview` | `config::preview_update` | `ManagementConfigMutationRequest` | response | Yes |
| POST | `/api/management/config/apply` | `config::apply_update` | `ManagementConfigApplyExecutionRequest` | response | Yes |

**Legacy:** `/api/config/*` (11 endpoints)

**Source:** `crates/astrbot-web/src/management/config.rs`

---

### III.M Tools, Commands, MCP Servers

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/tools` | `tools::catalog` | — | `ManagementToolCatalogResponse` | Yes |
| POST | `/api/management/tools/toggle` | `tools::toggle` | `ManagementToolToggleRequest` | `ManagementToolToggleResponse` | Yes |
| GET | `/api/management/commands` | `commands::catalog` | — | `ManagementCommandCatalogResponse` | Yes |
| POST | `/api/management/commands/update` | `commands::update` | `ManagementCommandUpdateRequest` | `ManagementCommandMutationResponse` | Yes |
| GET | `/api/management/mcp/servers` | `mcp::catalog` | — | `ManagementMcpCatalogResponse` | Yes |
| POST | `/api/management/mcp/servers/upsert` | `mcp::upsert` | `ManagementMcpUpsertRequest` | `ManagementMcpMutationResponse` | Yes |
| POST | `/api/management/mcp/servers/delete` | `mcp::delete` | `ManagementMcpDeleteRequest` | `ManagementMcpMutationResponse` | Yes |
| POST | `/api/management/mcp/servers/check` | `mcp::check` | `ManagementMcpCheckRequest` | `ManagementMcpCheckResponse` | Yes |
| POST | `/api/management/mcp/servers/sync` | `mcp::sync` | `ManagementMcpSyncRequest` | `ManagementMcpSyncResponse` | Yes |

**Legacy:** `/api/commands/*`, `/api/tools/*` (8 endpoints)

**Source:** `crates/astrbot-web/src/management/tools.rs`, `commands.rs`, `mcp.rs`

---

### III.N Skills (Capability Management)

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/skills` | `skills::catalog` | — | `ManagementSkillCatalogResponse` | Yes |
| POST | `/api/management/skills/activation` | `skills::set_active` | `ManagementSkillActivationRequest` | `ManagementSkillActivationResponse` | Yes |
| POST | `/api/management/skills/install-plan` | `skills::install_plan` | `ManagementSkillInstallPlanRequest` | `ManagementSkillInstallPlanResponse` | Yes |
| POST | `/api/management/skills/install` | `skills::install` | request | `ManagementSkillInstallResponse` | Yes |
| POST | `/api/management/skills/delete-plan` | `skills::delete_plan` | `ManagementSkillDeletePlanRequest` | `ManagementSkillDeletePlanResponse` | Yes |
| POST | `/api/management/skills/delete` | `skills::delete` | request | `ManagementSkillDeleteResponse` | Yes |

**Legacy:** `/api/skills/*` (7 endpoints)

**Source:** `crates/astrbot-web/src/management/skills.rs`

---

### III.O File Upload/Download & Backup

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| POST | `/api/management/files/upload` | `files::upload` | `Multipart` | `ManagementFileUploadResponse` | Yes |
| GET | `/api/management/files/{token}` | `files::download` | Path: `token` | binary | Yes |
| POST | `/api/management/backup/precheck` | `backup::precheck` | `ManagementBackupPrecheckRequest` | `ManagementBackupPrecheckResponse` | Yes |
| POST | `/api/management/backup/export` | `backup::export` | `ManagementBackupExportRequest` | `ManagementBackupJobResponse` | Yes |
| POST | `/api/management/backup/import` | `backup::import` | `ManagementBackupImportRequest` | `ManagementBackupJobResponse` | Yes |
| GET | `/api/management/backup/progress/{task_id}` | `backup::progress` | Path: `task_id` | `ManagementBackupProgressResponse` | Yes |
| GET | `/api/management/backup/progress` | `backup::progress_catalog` | — | `ManagementBackupProgressCatalogResponse` | Yes |
| GET | `/api/management/backup/files` | `backup::file_catalog` | — | file list | Yes |
| POST | `/api/management/backup/files/download` | `backup::file_download` | request | binary | Yes |
| POST | `/api/management/backup/files/restore` | `backup::file_restore` | request | response | Yes |
| POST | `/api/management/backup/upload/start` | `backup::upload_start` | `ManagementBackupUploadStartRequest` | `ManagementBackupUploadStartResponse` | Yes |
| POST | `/api/management/backup/upload/chunk` | `backup::upload_chunk` | `ManagementBackupChunkRequest` | `ManagementBackupChunkResponse` | Yes |
| POST | `/api/management/backup/upload/complete` | `backup::upload_complete` | `ManagementBackupCompleteRequest` | `ManagementBackupCompleteResponse` | Yes |
| POST | `/api/management/backup/upload/abort` | `backup::upload_abort` | `ManagementBackupAbortRequest` | `ManagementBackupAbortResponse` | Yes |

**Legacy:** `/api/backup/*` (10 endpoints)

**Source:** `crates/astrbot-web/src/management/files.rs`, `backup.rs`

---

### III.P Updates & Maintenance

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/update/check` | `update::check` | — | `MaintenanceCheckResponse` | Yes |
| GET | `/api/management/update/releases` | `update::releases` | — | `MaintenanceReleaseExecutor` | Yes |
| GET | `/api/management/update/changelog` | `update::changelog` | — | `MaintenanceChangelogResponse` | Yes |
| POST | `/api/management/update/project-plan` | `update::project_plan` | `ProjectUpdatePlanRequest` | plan | Yes |
| POST | `/api/management/update/package-plan` | `update::package_plan` | `MaintenancePackagePlanResponse` | plan | Yes |
| POST | `/api/management/update/package-run` | `update::package_run` | request | `MaintenanceOperationResponse` | Yes |
| POST | `/api/management/update/restart-plan` | `update::restart_plan` | `MaintenanceRestartRequest` | plan | Yes |
| POST | `/api/management/update/restart-run` | `update::restart_run` | `MaintenanceRestartRequest` | `MaintenanceRestartExecutor` | Yes |
| GET | `/api/management/update/migration-check` | `update::migration_check` | — | `MaintenanceMigrationCheckResponse` | Yes |
| POST | `/api/management/update/migration-plan` | `update::migration_plan` | request | plan | Yes |
| GET | `/api/management/update/operations` | `update::operation_catalog` | — | `MaintenanceOperationsResponse` | Yes |
| GET | `/api/management/update/operations/{operation_id}` | `update::operation` | Path: `op_id` | `MaintenanceOperationResponse` | Yes |
| POST | `/api/management/update/operations/run` | `update::run_operation` | `MaintenanceOperationRunRequest` | response | Yes |

**Legacy:** `/api/update/*`, `/api/stat/*` (12 endpoints)

**Source:** `crates/astrbot-web/src/management/update.rs`

---

### III.Q Text-to-Image Templates

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/t2i/templates` | `t2i_templates::list_templates` | — | template list | Yes |
| GET | `/api/t2i/templates/active` | `t2i_templates::active_template` | — | active template | Yes |
| POST | `/api/t2i/templates/create` | `t2i_templates::create_template` | request | template | Yes |
| POST | `/api/t2i/templates/reset_default` | `t2i_templates::reset_default_template` | — | template | Yes |
| POST | `/api/t2i/templates/set_active` | `t2i_templates::set_active_template` | request | response | Yes |
| GET/PUT/DELETE | `/api/t2i/templates/{name}` | `t2i_templates::{get,update,delete}` | Path: `name` + body | template | Yes |

**Source:** `crates/astrbot-web/src/management/t2i_templates.rs`

---

### III.R Plugin Market

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/plugin-market` | `plugin_market::catalog` | — | `PluginMarketCatalogResponse` | Yes |
| POST | `/api/management/plugin-market/install-plan` | `plugin_market::install_plan` | `PluginMarketPlanRequest` | `PluginMarketPlanResponse` | Yes |
| POST | `/api/management/plugin-market/install` | `plugin_market::install` | `PluginMarketExecuteResponse` | response | Yes |
| POST | `/api/management/plugin-market/update-plan` | `plugin_market::update_plan` | `PluginMarketPlanRequest` | `PluginMarketPlanResponse` | Yes |
| POST | `/api/management/plugin-market/update` | `plugin_market::update` | request | `PluginMarketExecuteResponse` | Yes |
| POST | `/api/management/plugin-market/uninstall-plan` | `plugin_market::uninstall_plan` | request | plan | Yes |
| POST | `/api/management/plugin-market/uninstall` | `plugin_market::uninstall` | request | response | Yes |
| GET | `/api/management/plugin-market/update-all-plan` | `plugin_market::update_all_plan` | — | `PluginMarketUpdateAllPlanResponse` | Yes |
| POST | `/api/management/plugin-market/update-all` | `plugin_market::update_all` | `PluginMarketUpdateAllRequest` | `PluginMarketUpdateAllExecuteResponse` | Yes |

**Source:** `crates/astrbot-web/src/management/plugin_market.rs`

---

### III.S Subagents & Additional

| HTTP Method | Path | Handler | Request DTO | Response DTO | Auth |
|---|---|---|---|---|---|
| GET | `/api/management/subagents` | `subagents::catalog` | — | `ManagementSubagentCatalogResponse` | Yes |
| POST | `/api/management/subagents/apply` | `subagents::apply` | `ManagementSubagentApplyRequest` | `ManagementSubagentApplyResponse` | Yes |
| POST | `/api/management/subagents/execute` | `subagents::execute` | `ManagementSubagentExecuteRequest` | `ManagementSubagentExecuteResponse` | Yes |
| GET | `/api/management/status` | `status` | — | `ManagementStatusResponse` | Yes |
| GET | `/api/management/dashboard/capabilities` | `dashboard::capabilities` | — | `DashboardCapabilitiesResponse` | Yes |

**Source:** `crates/astrbot-web/src/management/subagents.rs`, `dashboard.rs`, `mod.rs`

---

## IV. Core DTO Types for ts-rs Export

### Request DTOs (to be exported)

- `SubmitTextRequest` — webchat submit
- `OpenApiChatMessageRequest` — openapi chat
- `ManagementProviderUpsertRequest` — provider ops
- `ManagementPlatformUpsertRequest` — platform ops
- `ManagementKnowledgeBaseCreateRequest` — KB creation
- `ManagementPluginConfigRequest` — plugin config
- `ManagementConfigMutationRequest` — config updates
- `ManagementCronUpsertRequest` — cron schedule
- `ManagementConversationListRequest` — conversation queries
- `DashboardLoginRequest` — authentication
- `ManagementApiKeyIssueRequest` — API key generation
- `ManagementPersonaUpsertRequest` — persona CRUD
- `ManagementBackupExportRequest` — backup export
- `ManagementSkillInstallPlanRequest` — skill lifecycle
- And ~150 more (see `management/mod.rs:20–220` for full export list)

### Response DTOs (to be exported)

- `SubmitTextResponse` — event ID returned
- `OpenApiChatHttpResponse` — chat submission response
- `ManagementProviderCatalogResponse` — provider list
- `ManagementKnowledgeBaseCatalogResponse` — KB inventory
- `ManagementLogResponse` — logs snapshot
- `ManagementTraceResponse` — trace events
- `RealtimeSubscriptionCatalogResponse` — active subscriptions
- `RealtimeChatSubscriptionRecord` — subscription state
- `ManagementStatusResponse` — system health
- And ~200+ more (comprehensively listed in `management/mod.rs`)

**All exported structs use `#[derive(Serialize, Deserialize)]` and can use ts-rs with:**
```rust
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../frontend/src/api/dto/"))]
```

**Source:** `crates/astrbot-web/src/dto.rs:1–123`, `crates/astrbot-web/src/management/mod.rs:15–220`

---

## V. SSE & WebSocket Transport

### Server-Sent Events (SSE)

| Endpoint | Format | Use Case |
|---|---|---|
| `/api/management/logs/stream` | `Sse<ReceiverStream<Event>>` | Real-time log streaming (text/event-stream) |
| `/api/live-log` (legacy) | `Sse<...>` | Legacy log streaming |

Handler signature: Returns `Result<Sse<ReceiverStream<...>>, Error>`

**Source:** `crates/astrbot-web/src/management/observability.rs:499–558`

### WebSocket

| Endpoint | Path | Upgrade Handler | Message Type | Purpose |
|---|---|---|---|---|
| `/api/live_chat/ws` | routes | `live_chat_ws` | JSON (token required) | Legacy chat events |
| `/api/unified_chat/ws` | routes | `unified_chat_ws` | JSON | Unified chat protocol |
| `/api/v1/chat/ws` | openapi | `v1_chat_ws` | JSON | OpenAPI v1 chat realtime |

Handler signature: `WebSocketUpgrade` → `.on_upgrade(async fn(socket) { ... })`

**Source:** `crates/astrbot-web/src/routes.rs:391–521`, `crates/astrbot-web/src/openapi.rs:219–261`

---

## VI. Key Implementation Details

### Auth Middleware

- **Dashboard endpoints:** Use `require_management_auth` middleware (applied to all `/api/management/*`)
- **OpenAPI endpoints:** Bearer token validation via `extract_presented_api_key()` → check API key store scopes
- **WebChat endpoints:** No auth (public legacy endpoints)
- **Webhook endpoints:** No auth (POST/GET `/api/platform/webhook/{uuid}` accepts public payloads)

**Source:** `crates/astrbot-web/src/management/mod.rs:678–687`, `crates/astrbot-web/src/management/auth.rs`, `crates/astrbot-web/src/management/api_key.rs`

### Error Handling

All endpoints return `Result<Json<T>, (StatusCode, Json<ErrorResponse>)>` with:
- `ErrorResponse::{ error: String }`
- HTTP status codes: 400 (bad request), 401 (unauthorized), 403 (forbidden), 404 (not found), 500 (internal), 503 (unavailable)

**Source:** `crates/astrbot-web/src/error.rs`, `dto.rs:120–122`

### State Management

Endpoints access shared state via `State<ManagementApiState>` or `State<WebChatHttpState>` extractors containing:
- Provider/Platform/Plugin registries
- Config services
- Conversation storage
- Knowledge base managers
- Log buffers & trace stores

**Source:** `crates/astrbot-web/src/management/mod.rs:223–672`

---

## Summary Statistics

- **Total HTTP endpoints:** ~180 new + ~120 legacy = ~300
- **WebSocket endpoints:** 3 (`/api/live_chat/ws`, `/api/unified_chat/ws`, `/api/v1/chat/ws`)
- **SSE endpoints:** 2 (`/api/management/logs/stream`, `/api/live-log`)
- **DTO structs:** 300+ (req/response types across all modules)
- **Auth scopes:** `management`, `chat`, `openapi`, webhook (public)

---

## File Reference Index

| Module | Primary File | Line Range | Purpose |
|---|---|---|---|
| Routes | `routes.rs` | 67–102 | Webchat + WS router |
| OpenAPI | `openapi.rs` | 65–100 | Chat API + realtime |
| Management | `management/mod.rs` | 674–1598 | 180+ admin endpoints |
| Auth | `management/auth.rs` | — | Login + account edit |
| Providers | `management/providers.rs` | 341–840 | LLM/embedding provider CRUD |
| Platforms | `management/platforms.rs` | — | Discord/Telegram/QQ config |
| KB | `management/knowledge_base.rs` | 375–1440+ | RAG document management |
| Observability | `management/observability.rs` | 481–795 | Logs, traces, metrics (incl SSE) |
| DTOs | `dto.rs` | 1–123 | Core request/response types |

**Compiled:** 2026-05-19 | Target: Dashboard Frontend TS code generation via ts-rs
