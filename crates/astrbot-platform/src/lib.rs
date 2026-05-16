mod adapters;
mod built;
mod core;
mod manager;
mod registry;

#[cfg(test)]
mod tests;

pub use adapters::{
    ConsolePlatform, ConsoleSink, DecodedWebhookPayload, EncryptedWebhookEnvelope,
    InMemoryPlatformQueueStore, LongConnectionClient, LongConnectionCommand,
    LongConnectionEndpoint, LongConnectionFrame, LongConnectionReconnectPolicy,
    LongConnectionState, LongConnectionWaiters, MockPlatform, NoopTransport, OneBotPlatform,
    OneBotSession, OneBotSessionKind, OneBotTransport, OneBotTransportMode, PendingWebhookResponse,
    PlainWebhookPayloadCodec, PlatformApiClient, PlatformApiError, PlatformApiErrorKind,
    PlatformApiMethod, PlatformApiRequest, PlatformApiResponse, PlatformCallbackQueue,
    PlatformMediaKind, PlatformMediaReference, PlatformMediaSource, PlatformMediaUpload,
    PlatformMediaUploadClient, PlatformQueueDirection, PlatformQueueItem, PlatformQueueStats,
    PlatformRateLimit, PlatformRetryDecision, PlatformRetryPolicy, PlatformRetryReason,
    PlatformTransport, PlatformTransportKind, PlatformTransportState, RichEventMedia,
    RichEventPart, RichEventReaction, RichEventThread, RichPlatformEvent, Sha1SortedFieldsVerifier,
    WebChatPlatform, WebhookCallbackHandler, WebhookDuplicateStatus, WebhookEndpoint,
    WebhookEventDeduplicator, WebhookHttpMethod, WebhookPayloadCodec, WebhookRequest,
    WebhookResponse, WebhookRoute, WebhookServer, WebhookServerState, WebhookSignatureInput,
    WebhookSignatureVerdict, WebhookSignatureVerifier,
};
pub use built::BuiltPlatform;
pub use core::{
    CONSOLE_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, MessageRecorder, ONEBOT_PLATFORM_TYPE,
    PlatformAdapter, PlatformBuildContext, PlatformConfig, RecordingSink, SentMessage,
    StreamedMessage, WEBCHAT_PLATFORM_TYPE,
};
pub use manager::PlatformManager;
pub use registry::PlatformRegistry;

pub(crate) use core::validate_platform_id;
