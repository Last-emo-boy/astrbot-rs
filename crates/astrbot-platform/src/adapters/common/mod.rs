mod api_client;
mod identity;
mod long_connection;
mod media;
mod outbound;
mod path_mapping;
mod permission;
mod queue;
mod quote;
mod retry;
mod rich_event;
mod security;
mod transport;
mod webhook;

pub use api_client::{
    PlatformApiClient, PlatformApiError, PlatformApiErrorKind, PlatformApiMethod,
    PlatformApiRequest, PlatformApiResponse, ReqwestPlatformApiClient,
};
pub use identity::{PlatformGroupIdentityInput, PlatformIdentityNormalizer, platform_member};
pub use long_connection::{
    LongConnectionClient, LongConnectionCommand, LongConnectionEndpoint, LongConnectionFrame,
    LongConnectionReconnectPolicy, LongConnectionState, LongConnectionWaiters,
    TungsteniteLongConnectionClient,
};
pub use media::{
    PlatformMediaKind, PlatformMediaReference, PlatformMediaSource, PlatformMediaUpload,
    PlatformMediaUploadClient, PlatformVoiceConversionRequest, PlatformVoiceMediaConverter,
    PlatformVoiceTargetFormat, PlatformVoiceUploadPreparer, UnsupportedPlatformVoiceMediaConverter,
    detect_platform_voice_format,
};
pub use outbound::{
    PlatformOutboundRoute, PlatformOutboundRoutingState, PlatformReplyTarget,
    PlatformRouteTargetKind, PlatformSenderBinding, PlatformSessionScene, ProactiveSendReadiness,
    ProactiveSendRequirement,
};
pub use path_mapping::{PlatformPathMapping, PlatformPathMappingRules};
pub use permission::{
    IdentityPermissionResolver, PlatformPermission, PlatformPermissionResolver, permission_allows,
};
pub use queue::{
    InMemoryPlatformQueueStore, PendingWebhookResponse, PlatformCallbackQueue,
    PlatformQueueDirection, PlatformQueueItem, PlatformQueueStats,
};
pub use quote::{
    EmbeddedQuoteParser, PlatformQuoteParser, PlatformQuoteRequest, PlatformQuoteResolution,
};
pub use retry::{
    PlatformRateLimit, PlatformRetryDecision, PlatformRetryPolicy, PlatformRetryReason,
};
pub use rich_event::{
    RichEventMedia, RichEventPart, RichEventReaction, RichEventThread, RichPlatformEvent,
};
pub use security::{
    DecodedWebhookPayload, EncryptedWebhookEnvelope, PlainWebhookPayloadCodec,
    Sha1SortedFieldsVerifier, WebhookPayloadCodec, WebhookSignatureInput, WebhookSignatureVerdict,
    WebhookSignatureVerifier,
};
pub use transport::{
    NoopTransport, PlatformTransport, PlatformTransportKind, PlatformTransportState,
};
pub use webhook::{
    AxumWebhookServer, WebhookCallbackHandler, WebhookDuplicateStatus, WebhookEndpoint,
    WebhookEventDeduplicator, WebhookHttpMethod, WebhookRequest, WebhookResponse, WebhookRoute,
    WebhookServer, WebhookServerState,
};
