pub mod common;
mod console;
mod mock;
mod onebot;
pub(crate) mod wave1;
mod webchat;

pub use common::{
    AxumWebhookServer, DecodedWebhookPayload, EmbeddedQuoteParser, EncryptedWebhookEnvelope,
    IdentityPermissionResolver, InMemoryPlatformQueueStore, LongConnectionClient,
    LongConnectionCommand, LongConnectionEndpoint, LongConnectionFrame,
    LongConnectionReconnectPolicy, LongConnectionState, LongConnectionWaiters, NoopTransport,
    PendingWebhookResponse, PlainWebhookPayloadCodec, PlatformApiClient, PlatformApiError,
    PlatformApiErrorKind, PlatformApiMethod, PlatformApiRequest, PlatformApiResponse,
    PlatformCallbackQueue, PlatformGroupIdentityInput, PlatformIdentityNormalizer,
    PlatformMediaKind, PlatformMediaReference, PlatformMediaSource, PlatformMediaUpload,
    PlatformMediaUploadClient, PlatformOutboundRoute, PlatformOutboundRoutingState,
    PlatformPathMapping, PlatformPathMappingRules, PlatformPermission, PlatformPermissionResolver,
    PlatformQueueDirection, PlatformQueueItem, PlatformQueueStats, PlatformQuoteParser,
    PlatformQuoteRequest, PlatformQuoteResolution, PlatformRateLimit, PlatformReplyTarget,
    PlatformRetryDecision, PlatformRetryPolicy, PlatformRetryReason, PlatformRouteTargetKind,
    PlatformSenderBinding, PlatformSessionScene, PlatformTransport, PlatformTransportKind,
    PlatformTransportState, PlatformVoiceConversionRequest, PlatformVoiceMediaConverter,
    PlatformVoiceTargetFormat, PlatformVoiceUploadPreparer, ProactiveSendReadiness,
    ProactiveSendRequirement, ReqwestPlatformApiClient, RichEventMedia, RichEventPart,
    RichEventReaction, RichEventThread, RichPlatformEvent, Sha1SortedFieldsVerifier,
    TungsteniteLongConnectionClient, UnsupportedPlatformVoiceMediaConverter,
    WebhookCallbackHandler, WebhookDuplicateStatus, WebhookEndpoint, WebhookEventDeduplicator,
    WebhookHttpMethod, WebhookPayloadCodec, WebhookRequest, WebhookResponse, WebhookRoute,
    WebhookServer, WebhookServerState, WebhookSignatureInput, WebhookSignatureVerdict,
    WebhookSignatureVerifier, detect_platform_voice_format, permission_allows, platform_member,
};
pub use console::{ConsolePlatform, ConsoleSink};
pub use mock::MockPlatform;
pub use onebot::{
    OneBotForwardParseResult, OneBotForwardParser, OneBotPlatform, OneBotSession,
    OneBotSessionKind, OneBotTransport, OneBotTransportMode,
};
pub use webchat::WebChatPlatform;
