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
    ManagementApiState, ManagementStatusResponse, PlatformManagementResponse,
    PluginHandlerManagementResponse, PluginManagementResponse, ProviderManagementResponse,
    management_router,
};
pub use routes::webchat_router;
pub use server::{
    serve_management, serve_management_with_shutdown, serve_webchat, serve_webchat_with_shutdown,
};

#[cfg(test)]
mod tests;
