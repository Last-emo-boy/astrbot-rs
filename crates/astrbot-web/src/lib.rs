mod attachment;
mod dto;
mod error;
mod history;
mod management;
mod message_parts;
mod realtime;
mod routes;
mod server;

pub use attachment::{AttachmentDescriptor, AttachmentService, PassthroughAttachmentService};
pub use dto::{
    ErrorResponse, SubmitTextRequest, SubmitTextResponse, WebChatMessagePart,
    WebChatMessageResponse, WebChatMessagesResponse,
};
pub use management::{
    ApiKeyAuthDecision, ApiKeyIssuer, ApiKeyRejectionReason, AuthRejectionReason,
    DashboardAuthDecision, DashboardAuthPolicy, IssuedApiKey, ManagementApiState,
    ManagementAuthState, ManagementBackupAbortRequest, ManagementBackupAbortResponse,
    ManagementBackupChunkRequest, ManagementBackupChunkResponse, ManagementBackupCompleteRequest,
    ManagementBackupCompleteResponse, ManagementBackupExportRequest, ManagementBackupImportRequest,
    ManagementBackupJobResponse, ManagementBackupPrecheckRequest, ManagementBackupPrecheckResponse,
    ManagementBackupProgressResponse, ManagementBackupState, ManagementBackupUploadStartRequest,
    ManagementBackupUploadStartResponse, ManagementChatProjectActorRequest,
    ManagementChatProjectCatalogResponse, ManagementChatProjectCreateRequest,
    ManagementChatProjectDescriptor, ManagementChatProjectGetRequest,
    ManagementChatProjectMembershipRequest, ManagementChatProjectMutationResponse,
    ManagementChatProjectResponse, ManagementChatProjectSessionsResponse,
    ManagementChatProjectState, ManagementChatProjectUpdateRequest,
    ManagementConfigMutationRequest, ManagementConfigMutationResponse,
    ManagementConfigSchemaResponse, ManagementFileDownloadState,
    ManagementPlatformSessionDescriptor, ManagementSessionRuleState,
    ManagementSkillActivationRequest, ManagementSkillActivationResponse,
    ManagementSkillCatalogResponse, ManagementSkillDeletePlanRequest,
    ManagementSkillDeletePlanResponse, ManagementSkillDescriptor,
    ManagementSkillInstallPlanRequest, ManagementSkillInstallPlanResponse, ManagementSkillState,
    ManagementStatusResponse, ManagementToolCatalogResponse, ManagementToolDescriptor,
    ManagementToolState, ManagementToolToggleRequest, ManagementToolToggleResponse, OpenApiScope,
    OpenApiScopeSet, PlatformManagementResponse, PluginHandlerManagementResponse,
    PluginManagementResponse, PluginMarketCatalogResponse, PluginMarketManagementState,
    PluginMarketPlanRequest, PluginMarketPlanResponse, PresentedApiKey, ProviderManagementResponse,
    ScopedDownloadError, ScopedDownloadFile, authorize_api_key, extract_bearer_token,
    extract_presented_api_key, hash_api_key, management_router, management_router_with_auth,
};
pub use realtime::{
    DEFAULT_LIVE_AUDIO_FORMAT, LiveAudioBuffer, LiveAudioError, LiveAudioFormat, LiveAudioWavFile,
    OpenApiChatAuthContext, OpenApiChatEnqueuePlan, OpenApiChatGateway, OpenApiChatGatewayError,
    OpenApiChatGatewayRequest, OpenApiChatMessagePart, OpenApiChatMessageRequest,
    OpenApiChatResponseMode, OpenApiChatSubscriptionPlan, RealtimeConnectionSession,
    RealtimeProcessingState, RealtimeSubscription, required_openapi_chat_scopes,
};
pub use routes::webchat_router;
pub use server::{
    serve_management, serve_management_with_auth, serve_management_with_auth_and_shutdown,
    serve_management_with_shutdown, serve_webchat, serve_webchat_with_shutdown,
};

#[cfg(test)]
mod tests;
