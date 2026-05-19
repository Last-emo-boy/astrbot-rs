mod adapters;
mod built;
mod core;
mod manager;
mod registry;

#[cfg(test)]
mod tests;

pub use adapters::{
    AxumWebhookServer, ConsolePlatform, ConsoleSink, DecodedWebhookPayload, EmbeddedQuoteParser,
    EncryptedWebhookEnvelope, IdentityPermissionResolver, InMemoryPlatformQueueStore,
    LongConnectionClient, LongConnectionCommand, LongConnectionEndpoint, LongConnectionFrame,
    LongConnectionReconnectPolicy, LongConnectionState, LongConnectionWaiters, MockPlatform,
    NoopTransport, OneBotForwardParseResult, OneBotForwardParser, OneBotPlatform, OneBotSession,
    OneBotSessionKind, OneBotTransport, OneBotTransportMode, PendingWebhookResponse,
    PlainWebhookPayloadCodec, PlatformApiClient, PlatformApiError, PlatformApiErrorKind,
    PlatformApiMethod, PlatformApiRequest, PlatformApiResponse, PlatformCallbackQueue,
    PlatformGroupIdentityInput, PlatformIdentityNormalizer, PlatformMediaKind,
    PlatformMediaReference, PlatformMediaSource, PlatformMediaUpload, PlatformMediaUploadClient,
    PlatformOutboundRoute, PlatformOutboundRoutingState, PlatformPathMapping,
    PlatformPathMappingRules, PlatformPermission, PlatformPermissionResolver,
    PlatformQueueDirection, PlatformQueueItem, PlatformQueueStats, PlatformQuoteParser,
    PlatformQuoteRequest, PlatformQuoteResolution, PlatformRateLimit, PlatformReplyTarget,
    PlatformRetryDecision, PlatformRetryPolicy, PlatformRetryReason, PlatformRouteTargetKind,
    PlatformSenderBinding, PlatformSessionScene, PlatformTransport, PlatformTransportKind,
    PlatformTransportState, PlatformVoiceConversionRequest, PlatformVoiceMediaConverter,
    PlatformVoiceTargetFormat, PlatformVoiceUploadPreparer, ProactiveSendReadiness,
    ProactiveSendRequirement, ReqwestPlatformApiClient, RichEventMedia, RichEventPart,
    RichEventReaction, RichEventThread, RichPlatformEvent, Sha1SortedFieldsVerifier,
    TungsteniteLongConnectionClient, UnsupportedPlatformVoiceMediaConverter, WebChatPlatform,
    WebhookCallbackHandler, WebhookDuplicateStatus, WebhookEndpoint, WebhookEventDeduplicator,
    WebhookHttpMethod, WebhookPayloadCodec, WebhookRequest, WebhookResponse, WebhookRoute,
    WebhookServer, WebhookServerState, WebhookSignatureInput, WebhookSignatureVerdict,
    WebhookSignatureVerifier, detect_platform_voice_format, permission_allows, platform_member,
};
pub use built::BuiltPlatform;
pub use core::{
    AIOCQHTTP_PLATFORM_TYPE, CONSOLE_PLATFORM_TYPE, DINGTALK_PLATFORM_TYPE, DISCORD_PLATFORM_TYPE,
    KOOK_PLATFORM_TYPE, LARK_PLATFORM_TYPE, LINE_PLATFORM_TYPE, MISSKEY_PLATFORM_TYPE,
    MOCK_PLATFORM_TYPE, MessageRecorder, ONEBOT_PLATFORM_TYPE, PlatformAdapter,
    PlatformBuildContext, PlatformConfig, QQ_OFFICIAL_PLATFORM_TYPE,
    QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE, RecordingSink, SATORI_PLATFORM_TYPE, SLACK_PLATFORM_TYPE,
    SentMessage, StreamedMessage, TELEGRAM_PLATFORM_TYPE, WEBCHAT_PLATFORM_TYPE,
    WECOM_AI_BOT_PLATFORM_TYPE, WECOM_KF_PLATFORM_TYPE, WECOM_PLATFORM_TYPE,
    WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
};
pub use manager::PlatformManager;
pub use registry::PlatformRegistry;

pub(crate) use core::validate_platform_id;
