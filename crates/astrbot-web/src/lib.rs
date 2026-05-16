mod attachment;
mod dto;
mod error;
mod history;
mod management;
mod message_parts;
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
    ManagementAuthState, ManagementConfigMutationRequest, ManagementConfigMutationResponse,
    ManagementConfigSchemaResponse, ManagementStatusResponse, OpenApiScope, OpenApiScopeSet,
    PlatformManagementResponse, PluginHandlerManagementResponse, PluginManagementResponse,
    PluginMarketCatalogResponse, PluginMarketManagementState, PluginMarketPlanRequest,
    PluginMarketPlanResponse, PresentedApiKey, ProviderManagementResponse, authorize_api_key,
    extract_bearer_token, extract_presented_api_key, hash_api_key, management_router,
    management_router_with_auth,
};
pub use routes::webchat_router;
pub use server::{
    serve_management, serve_management_with_auth, serve_management_with_auth_and_shutdown,
    serve_management_with_shutdown, serve_webchat, serve_webchat_with_shutdown,
};

#[cfg(test)]
mod tests;
