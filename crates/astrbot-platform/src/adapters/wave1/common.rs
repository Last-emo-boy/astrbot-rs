use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use astrbot_core::{
    AstrbotError, MessageChain, MessageComponent, MessageEvent, MessageSender, MessageSession,
    MessageSink, MessageStream, Result,
};
use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::{
    AxumWebhookServer, BuiltPlatform, LongConnectionClient, LongConnectionEndpoint,
    LongConnectionFrame, MessageRecorder, PlatformAdapter, PlatformApiClient, PlatformApiMethod,
    PlatformApiRequest, PlatformBuildContext, PlatformConfig, PlatformGroupIdentityInput,
    PlatformIdentityNormalizer, RecordingSink, ReqwestPlatformApiClient, Sha1SortedFieldsVerifier,
    TungsteniteLongConnectionClient, WebhookCallbackHandler, WebhookEndpoint, WebhookRequest,
    WebhookResponse, WebhookRoute, WebhookServer, WebhookSignatureInput, WebhookSignatureVerdict,
    WebhookSignatureVerifier,
};

#[derive(Clone, Copy)]
pub(crate) struct Wave1PlatformSpec {
    pub platform_type: &'static str,
    pub default_name: &'static str,
    pub streaming_supported: bool,
    pub connection_mode_option: Option<&'static str>,
    pub default_connection_mode: &'static str,
    pub api_base_url_option: &'static str,
    pub default_api_base_url: &'static str,
    pub webhook_host_option: &'static str,
    pub webhook_port_option: &'static str,
    pub webhook_path_option: &'static str,
    pub default_webhook_path: &'static str,
    pub socket_url_option: &'static str,
    pub signature: Wave1SignatureSpec,
}

#[derive(Clone, Copy)]
pub(crate) enum Wave1SignatureSpec {
    None,
    Sha1SortedFields { secret_key: &'static str },
    LineHmacSha256 { secret_key: &'static str },
    SlackHmacSha256 { secret_key: &'static str },
    DingTalkHmacSha256 { secret_key: &'static str },
}

pub(crate) fn build_wave1_platform(
    config: &PlatformConfig,
    ctx: &PlatformBuildContext,
    spec: Wave1PlatformSpec,
) -> Result<BuiltPlatform> {
    let api_client = Arc::new(ReqwestPlatformApiClient::new());
    let platform = Wave1Platform::new(config, ctx, spec, api_client)?;
    Ok(BuiltPlatform::with_recording_sink(
        platform.clone(),
        platform.recorder(),
    ))
}

pub(crate) fn required_option_str(config: &PlatformConfig, key: &str) -> Result<()> {
    let value = config.option_str(key).unwrap_or_default().trim();
    if value.is_empty() {
        return Err(AstrbotError::Platform(format!(
            "platform {} ({}) requires option {key}",
            config.id, config.platform_type
        )));
    }
    Ok(())
}

pub(crate) fn required_secret_or_option(config: &PlatformConfig, key: &str) -> Result<()> {
    let value = config.secret_or_option_str(key).unwrap_or_default().trim();
    if value.is_empty() {
        return Err(AstrbotError::Platform(format!(
            "platform {} ({}) requires secret {key}",
            config.id, config.platform_type
        )));
    }
    Ok(())
}

pub(crate) fn required_u16(config: &PlatformConfig, key: &str) -> Result<()> {
    if config.option_u16(key).is_none() {
        return Err(AstrbotError::Platform(format!(
            "platform {} ({}) requires numeric option {key}",
            config.id, config.platform_type
        )));
    }
    Ok(())
}

pub(crate) struct Wave1Platform {
    id: String,
    name: String,
    dispatcher: Wave1Dispatcher,
    sink: Arc<Wave1Sink>,
    webhook_server: Option<Arc<AxumWebhookServer>>,
    webhook_route: Option<WebhookRoute>,
    long_connection: Option<Arc<TungsteniteLongConnectionClient>>,
}

impl Wave1Platform {
    pub(crate) fn new(
        config: &PlatformConfig,
        ctx: &PlatformBuildContext,
        spec: Wave1PlatformSpec,
        api_client: Arc<dyn PlatformApiClient>,
    ) -> Result<Arc<Self>> {
        let name = config
            .name
            .clone()
            .unwrap_or_else(|| spec.default_name.to_string());
        let recorder = Arc::new(RecordingSink::default());
        let sink = Arc::new(Wave1Sink::new(config, spec, recorder, api_client)?);
        let dispatcher = Wave1Dispatcher {
            id: config.id.clone(),
            name: name.clone(),
            spec,
            event_sender: ctx.event_sender(),
            sink: sink.clone(),
            event_counter: Arc::new(AtomicU64::new(1)),
        };
        let signature_verifier = signature_verifier(config, spec.signature)?;
        let webhook_server = webhook_server(config, spec)?;
        let webhook_route = webhook_server.as_ref().map(|_| {
            let handler = Arc::new(Wave1WebhookHandler {
                dispatcher: dispatcher.clone(),
            });
            let mut route = WebhookRoute::new(
                WebhookEndpoint::post(webhook_path(config, spec).to_string()),
                handler,
            );
            if let Some(verifier) = signature_verifier {
                route = route.with_signature_verifier(verifier);
            }
            route
        });
        let long_connection = long_connection(config, spec)?;

        Ok(Arc::new(Self {
            id: config.id.clone(),
            name,
            dispatcher,
            sink,
            webhook_server,
            webhook_route,
            long_connection,
        }))
    }

    pub(crate) fn recorder(&self) -> Arc<dyn MessageRecorder> {
        self.sink.recorder()
    }
}

#[async_trait]
impl PlatformAdapter for Wave1Platform {
    async fn run(&self) -> Result<()> {
        if let Some(long_connection) = &self.long_connection {
            let mut frames = long_connection.subscribe_frames();
            let dispatcher = self.dispatcher.clone();
            let processor: JoinHandle<()> = tokio::spawn(async move {
                while let Ok(frame) = frames.recv().await {
                    if let LongConnectionFrame::Callback { payload, .. } = frame {
                        let _ = dispatcher.dispatch_bytes(payload).await;
                    }
                }
            });
            let result = long_connection.run().await;
            processor.abort();
            return result;
        }

        if let (Some(server), Some(route)) = (&self.webhook_server, &self.webhook_route) {
            return server.run(vec![route.clone()]).await;
        }

        Ok(())
    }

    async fn terminate(&self) -> Result<()> {
        if let Some(long_connection) = &self.long_connection {
            long_connection.terminate().await?;
        }
        if let Some(server) = &self.webhook_server {
            server.terminate().await?;
        }
        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone)]
struct Wave1Dispatcher {
    id: String,
    name: String,
    spec: Wave1PlatformSpec,
    event_sender: mpsc::Sender<MessageEvent>,
    sink: Arc<Wave1Sink>,
    event_counter: Arc<AtomicU64>,
}

impl Wave1Dispatcher {
    async fn dispatch_bytes(&self, payload: Vec<u8>) -> Result<Vec<String>> {
        let value = serde_json::from_slice::<Value>(&payload).map_err(|err| {
            AstrbotError::Platform(format!(
                "{} inbound payload must be JSON: {err}",
                self.spec.platform_type
            ))
        })?;
        self.dispatch_json(value).await
    }

    async fn dispatch_json(&self, payload: Value) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for value in inbound_events(&payload) {
            let event = self.event_from_value(value)?;
            let id = event.id.clone();
            self.event_sender
                .send(event)
                .await
                .map_err(|_| AstrbotError::EventChannelClosed)?;
            ids.push(id);
        }
        Ok(ids)
    }

    fn event_from_value(&self, value: &Value) -> Result<MessageEvent> {
        let event_id = find_string(
            value,
            &[
                "event_id",
                "eventId",
                "message_id",
                "message.message_id",
                "message.id",
                "body.id",
                "body.message.id",
                "event.message_id",
                "msg_id",
                "msgId",
                "id",
                "event_id",
                "update_id",
                "replyToken",
                "MsgId",
            ],
        )
        .unwrap_or_else(|| {
            format!(
                "{}-event-{}",
                self.id,
                self.event_counter.fetch_add(1, Ordering::Relaxed)
            )
        });
        let sender_id = sender_id(value);
        let sender_name = find_string(
            value,
            &[
                "sender.name",
                "sender.sender_name",
                "message.from.first_name",
                "message.from.username",
                "message.author.username",
                "message.author.display_name",
                "from.first_name",
                "from.username",
                "author.username",
                "author.nickname",
                "body.user.name",
                "user.name",
                "user.username",
                "extra.author.nickname",
                "extra.author.username",
                "source.userName",
                "senderNick",
                "FromUserName",
            ],
        );
        let sender = MessageSender::new(sender_id.clone(), sender_name.clone());
        let (session, group_identity) = session_from_value(&self.id, value, &sender_id);
        let message = message_chain_from_value(value);
        if message.is_empty() {
            return Err(AstrbotError::EmptyMessage);
        }
        let mut event = MessageEvent::new(
            event_id,
            self.id.clone(),
            self.name.clone(),
            session,
            sender,
            message,
            self.sink.clone(),
        );
        let identity =
            PlatformIdentityNormalizer::normalize_identity(sender_id, sender_name, group_identity);
        event.set_identity(identity);
        if let Some(self_id) = find_string(
            value,
            &[
                "self_id",
                "bot_id",
                "authorizations.0.user_id",
                "robotCode",
                "ToUserName",
                "body.self.id",
            ],
        ) {
            event = event.with_self_id(self_id);
        }
        Ok(event)
    }
}

struct Wave1WebhookHandler {
    dispatcher: Wave1Dispatcher,
}

#[async_trait]
impl WebhookCallbackHandler for Wave1WebhookHandler {
    async fn handle(&self, request: WebhookRequest) -> Result<WebhookResponse> {
        let payload = serde_json::from_slice::<Value>(&request.body).map_err(|err| {
            AstrbotError::Platform(format!(
                "{} webhook body must be JSON: {err}",
                self.dispatcher.spec.platform_type
            ))
        })?;
        if let Some(challenge) = find_string(&payload, &["challenge"]) {
            return Ok(WebhookResponse::ok_text(challenge));
        }
        self.dispatcher.dispatch_json(payload).await?;
        Ok(WebhookResponse::ok_text("ok"))
    }
}

struct Wave1Sink {
    platform_id: String,
    platform_type: String,
    streaming_supported: bool,
    api_base_url: String,
    auth_headers: Vec<(String, String)>,
    recorder: Arc<RecordingSink>,
    api_client: Arc<dyn PlatformApiClient>,
}

impl Wave1Sink {
    fn new(
        config: &PlatformConfig,
        spec: Wave1PlatformSpec,
        recorder: Arc<RecordingSink>,
        api_client: Arc<dyn PlatformApiClient>,
    ) -> Result<Self> {
        let api_base_url = config
            .option_str(spec.api_base_url_option)
            .or_else(|| config.option_str("api_base_url"))
            .unwrap_or(spec.default_api_base_url)
            .trim()
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            platform_id: config.id.clone(),
            platform_type: spec.platform_type.to_string(),
            streaming_supported: spec.streaming_supported,
            api_base_url,
            auth_headers: auth_headers(config, spec.platform_type),
            recorder,
            api_client,
        })
    }

    fn recorder(&self) -> Arc<dyn MessageRecorder> {
        self.recorder.clone()
    }

    fn outbound_request(
        &self,
        session: &MessageSession,
        action: &str,
        payload: Value,
        streaming: bool,
    ) -> PlatformApiRequest {
        let mut body = json!({
            "platform_id": self.platform_id,
            "session_id": session.conversation_id,
            "session_kind": if session.is_group() { "group" } else { "direct" },
            "action": action,
            "streaming": streaming,
            "payload": payload,
        });
        if streaming && !self.streaming_supported {
            body["streaming_fallback"] = Value::String("disabled".to_string());
        }
        let endpoint = outbound_endpoint(&self.platform_type, &self.api_base_url, action);
        let mut request = PlatformApiRequest::new(
            self.platform_type.clone(),
            PlatformApiMethod::Post,
            endpoint,
        )
        .with_header("content-type", "application/json")
        .with_body(body.to_string().into_bytes())
        .with_rate_limit_key(format!("{}:{action}", self.platform_id));
        for (name, value) in &self.auth_headers {
            request = request.with_header(name.clone(), value.clone());
        }
        request
    }

    fn requests_for_chain(
        &self,
        session: &MessageSession,
        chain: &MessageChain,
        streaming: bool,
    ) -> Vec<PlatformApiRequest> {
        let mut requests = Vec::new();
        let mut text_fallback = Vec::new();
        for component in chain.components() {
            match component {
                MessageComponent::Plain { text } if !text.trim().is_empty() => {
                    requests.push(self.outbound_request(
                        session,
                        "text",
                        json!({ "text": text }),
                        streaming,
                    ));
                }
                MessageComponent::Image { url } if !url.trim().is_empty() => {
                    requests.push(self.outbound_request(
                        session,
                        "image",
                        json!({ "url": url }),
                        streaming,
                    ));
                }
                MessageComponent::File { name, url } if !url.trim().is_empty() => {
                    requests.push(self.outbound_request(
                        session,
                        "file",
                        json!({ "name": name, "url": url }),
                        streaming,
                    ));
                }
                MessageComponent::Record { url } if !url.trim().is_empty() => {
                    requests.push(self.outbound_request(
                        session,
                        "record",
                        json!({ "url": url }),
                        streaming,
                    ));
                }
                MessageComponent::Video { url } if !url.trim().is_empty() => {
                    requests.push(self.outbound_request(
                        session,
                        "file",
                        json!({ "name": "video", "url": url, "fallback_from": "video" }),
                        streaming,
                    ));
                }
                MessageComponent::Mention { user_id } if !user_id.trim().is_empty() => {
                    text_fallback.push(format!("<@{user_id}>"));
                }
                MessageComponent::MentionAll => {
                    text_fallback.push("@all".to_string());
                }
                MessageComponent::Reply { message_id, .. } if !message_id.trim().is_empty() => {
                    text_fallback.push(format!("reply to {message_id}"));
                }
                _ => {}
            }
        }
        if requests.is_empty() && !text_fallback.is_empty() {
            requests.push(self.outbound_request(
                session,
                "text",
                json!({ "text": text_fallback.join(" "), "fallback_from": "metadata" }),
                streaming,
            ));
        }
        requests
    }
}

#[async_trait]
impl MessageSink for Wave1Sink {
    async fn send(&self, session: &MessageSession, chain: MessageChain) -> Result<()> {
        self.recorder.send(session, chain.clone()).await?;
        for request in self.requests_for_chain(session, &chain, false) {
            self.api_client.execute(request).await?;
        }
        Ok(())
    }

    async fn send_streaming(&self, session: &MessageSession, stream: MessageStream) -> Result<()> {
        self.recorder
            .send_streaming(session, stream.clone())
            .await?;
        for chunk in stream.chunks() {
            for request in self.requests_for_chain(session, chunk, true) {
                self.api_client.execute(request).await?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum HmacWebhookMode {
    Line,
    Slack,
    DingTalk,
}

struct HmacSha256WebhookVerifier {
    secret: String,
    mode: HmacWebhookMode,
}

impl HmacSha256WebhookVerifier {
    fn new(secret: impl Into<String>, mode: HmacWebhookMode) -> Result<Self> {
        let secret = secret.into().trim().to_string();
        if secret.is_empty() {
            return Err(AstrbotError::Platform(
                "webhook HMAC secret cannot be empty".to_string(),
            ));
        }
        Ok(Self { secret, mode })
    }
}

impl WebhookSignatureVerifier for HmacSha256WebhookVerifier {
    fn sign(&self, input: &WebhookSignatureInput) -> Result<String> {
        let signing_payload = match self.mode {
            HmacWebhookMode::Line => input.payload.clone(),
            HmacWebhookMode::Slack => {
                format!("v0:{}:{}", input.timestamp, input.payload)
            }
            HmacWebhookMode::DingTalk => format!("{}\n{}", input.timestamp, self.secret),
        };
        let mut mac = Hmac::<Sha256>::new_from_slice(self.secret.as_bytes()).map_err(|err| {
            AstrbotError::Platform(format!("create webhook HMAC verifier: {err}"))
        })?;
        mac.update(signing_payload.as_bytes());
        let digest = mac.finalize().into_bytes();
        Ok(match self.mode {
            HmacWebhookMode::Slack => format!("v0={}", hex_lower(&digest)),
            HmacWebhookMode::Line | HmacWebhookMode::DingTalk => {
                base64::engine::general_purpose::STANDARD.encode(digest)
            }
        })
    }

    fn verify(
        &self,
        input: &WebhookSignatureInput,
        received_signature: &str,
    ) -> Result<WebhookSignatureVerdict> {
        let expected = self.sign(input)?;
        if constant_time_eq(expected.as_bytes(), received_signature.as_bytes()) {
            Ok(WebhookSignatureVerdict::Match)
        } else {
            Ok(WebhookSignatureVerdict::Mismatch)
        }
    }
}

fn signature_verifier(
    config: &PlatformConfig,
    spec: Wave1SignatureSpec,
) -> Result<Option<Arc<dyn WebhookSignatureVerifier>>> {
    let verifier: Option<Arc<dyn WebhookSignatureVerifier>> = match spec {
        Wave1SignatureSpec::None => None,
        Wave1SignatureSpec::Sha1SortedFields { secret_key } => {
            let Some(secret) = config.secret_or_option_str(secret_key) else {
                return Ok(None);
            };
            Some(Arc::new(Sha1SortedFieldsVerifier::new(secret)?))
        }
        Wave1SignatureSpec::LineHmacSha256 { secret_key } => config
            .secret_or_option_str(secret_key)
            .map(|secret| {
                HmacSha256WebhookVerifier::new(secret, HmacWebhookMode::Line)
                    .map(|verifier| Arc::new(verifier) as Arc<dyn WebhookSignatureVerifier>)
            })
            .transpose()?,
        Wave1SignatureSpec::SlackHmacSha256 { secret_key } => config
            .secret_or_option_str(secret_key)
            .map(|secret| {
                HmacSha256WebhookVerifier::new(secret, HmacWebhookMode::Slack)
                    .map(|verifier| Arc::new(verifier) as Arc<dyn WebhookSignatureVerifier>)
            })
            .transpose()?,
        Wave1SignatureSpec::DingTalkHmacSha256 { secret_key } => config
            .secret_or_option_str(secret_key)
            .map(|secret| {
                HmacSha256WebhookVerifier::new(secret, HmacWebhookMode::DingTalk)
                    .map(|verifier| Arc::new(verifier) as Arc<dyn WebhookSignatureVerifier>)
            })
            .transpose()?,
    };
    Ok(verifier)
}

fn webhook_server(
    config: &PlatformConfig,
    spec: Wave1PlatformSpec,
) -> Result<Option<Arc<AxumWebhookServer>>> {
    let wants_webhook = connection_mode(config, spec) == "webhook"
        || config.option_u16(spec.webhook_port_option).is_some();
    if !wants_webhook {
        return Ok(None);
    }
    let host = config
        .option_str(spec.webhook_host_option)
        .unwrap_or("0.0.0.0");
    let port = config.option_u16(spec.webhook_port_option).unwrap_or(0);
    let bind_addr = format!("{host}:{port}")
        .parse::<SocketAddr>()
        .map_err(|err| {
            AstrbotError::Platform(format!(
                "platform {} invalid webhook bind address {host}:{port}: {err}",
                config.id
            ))
        })?;
    Ok(Some(Arc::new(AxumWebhookServer::new(bind_addr))))
}

fn long_connection(
    config: &PlatformConfig,
    spec: Wave1PlatformSpec,
) -> Result<Option<Arc<TungsteniteLongConnectionClient>>> {
    let Some(socket_url) = config.option_str(spec.socket_url_option) else {
        return Ok(None);
    };
    let socket_url = socket_url.trim();
    if socket_url.is_empty() {
        return Ok(None);
    }
    Ok(Some(Arc::new(TungsteniteLongConnectionClient::new(
        LongConnectionEndpoint::new(socket_url),
    ))))
}

fn webhook_path(config: &PlatformConfig, spec: Wave1PlatformSpec) -> &str {
    config
        .option_str(spec.webhook_path_option)
        .unwrap_or(spec.default_webhook_path)
}

fn connection_mode(config: &PlatformConfig, spec: Wave1PlatformSpec) -> &str {
    spec.connection_mode_option
        .and_then(|key| config.option_str(key))
        .unwrap_or(spec.default_connection_mode)
}

fn auth_headers(config: &PlatformConfig, platform_type: &str) -> Vec<(String, String)> {
    match platform_type {
        "telegram" => config
            .secret_or_option_str("telegram_token")
            .map(|token| vec![("x-telegram-token".to_string(), token.to_string())])
            .unwrap_or_default(),
        "slack" => bearer_header(config.secret_or_option_str("bot_token")),
        "lark" => config
            .secret_or_option_str("app_secret")
            .map(|secret| {
                vec![
                    (
                        "x-lark-app-id".to_string(),
                        config.option_str("app_id").unwrap_or_default().to_string(),
                    ),
                    ("x-lark-app-secret".to_string(), secret.to_string()),
                ]
            })
            .unwrap_or_default(),
        "line" => bearer_header(config.secret_or_option_str("channel_access_token")),
        "wecom" => config
            .secret_or_option_str("secret")
            .map(|secret| {
                vec![
                    (
                        "x-wecom-corp-id".to_string(),
                        config
                            .secret_or_option_str("corpid")
                            .unwrap_or_default()
                            .to_string(),
                    ),
                    ("x-wecom-secret".to_string(), secret.to_string()),
                ]
            })
            .unwrap_or_default(),
        "wecom_ai_bot" => config
            .secret_or_option_str("wecomaibot_token")
            .or_else(|| config.secret_or_option_str("wecomaibot_ws_secret"))
            .map(|secret| vec![("x-wecom-ai-bot-token".to_string(), secret.to_string())])
            .unwrap_or_default(),
        "dingtalk" => config
            .secret_or_option_str("client_secret")
            .map(|secret| {
                vec![
                    (
                        "x-dingtalk-client-id".to_string(),
                        config
                            .option_str("client_id")
                            .unwrap_or_default()
                            .to_string(),
                    ),
                    ("x-dingtalk-client-secret".to_string(), secret.to_string()),
                ]
            })
            .unwrap_or_default(),
        "discord" => bearer_header(config.secret_or_option_str("discord_token")),
        "kook" => bearer_header(config.secret_or_option_str("kook_bot_token")),
        "misskey" => bearer_header(config.secret_or_option_str("misskey_token")),
        "satori" => bearer_header(config.secret_or_option_str("satori_token")),
        "qq_official" | "qq_official_webhook" => {
            app_secret_headers(config, "x-qq-app-id", "appid", "x-qq-secret", "secret")
        }
        "wecom_kf" => app_secret_headers(
            config,
            "x-wecom-corp-id",
            "corpid",
            "x-wecom-secret",
            "secret",
        ),
        "weixin_official_account" => app_secret_headers(
            config,
            "x-weixin-app-id",
            "appid",
            "x-weixin-secret",
            "secret",
        ),
        _ => Vec::new(),
    }
}

fn app_secret_headers(
    config: &PlatformConfig,
    id_header: &str,
    id_key: &str,
    secret_header: &str,
    secret_key: &str,
) -> Vec<(String, String)> {
    config
        .secret_or_option_str(secret_key)
        .map(|secret| {
            vec![
                (
                    id_header.to_string(),
                    config
                        .secret_or_option_str(id_key)
                        .unwrap_or_default()
                        .to_string(),
                ),
                (secret_header.to_string(), secret.to_string()),
            ]
        })
        .unwrap_or_default()
}

fn bearer_header(token: Option<&str>) -> Vec<(String, String)> {
    token
        .map(|token| vec![("authorization".to_string(), format!("Bearer {token}"))])
        .unwrap_or_default()
}

fn outbound_endpoint(platform_type: &str, base_url: &str, action: &str) -> String {
    match (platform_type, action) {
        ("telegram", "text") => format!("{base_url}/sendMessage"),
        ("telegram", "image") => format!("{base_url}/sendPhoto"),
        ("telegram", "file") => format!("{base_url}/sendDocument"),
        ("telegram", "record") => format!("{base_url}/sendVoice"),
        ("slack", "text") => format!("{base_url}/chat.postMessage"),
        ("slack", "image" | "file" | "record") => format!("{base_url}/files.upload"),
        ("lark", _) => format!("{base_url}/im/v1/messages"),
        ("line", _) => format!("{base_url}/v2/bot/message/push"),
        ("wecom", _) => format!("{base_url}/cgi-bin/message/send"),
        ("wecom_ai_bot", _) => format!("{base_url}/cgi-bin/webhook/send"),
        ("dingtalk", "text") => format!("{base_url}/v1.0/robot/messages/send"),
        ("dingtalk", _) => format!("{base_url}/v1.0/robot/groupMessages/send"),
        ("discord", _) => format!("{base_url}/channels/messages"),
        ("kook", "text") => format!("{base_url}/message/create"),
        ("kook", _) => format!("{base_url}/asset/create"),
        ("misskey", _) => format!("{base_url}/notes/create"),
        ("satori", _) => format!("{base_url}/message.create"),
        ("qq_official", _) => format!("{base_url}/messages"),
        ("qq_official_webhook", _) => format!("{base_url}/webhook/messages"),
        ("wecom_kf", _) => format!("{base_url}/kf/send_msg"),
        ("weixin_official_account", _) => format!("{base_url}/message/custom/send"),
        _ => format!("{base_url}/{action}"),
    }
}

fn inbound_events(payload: &Value) -> Vec<&Value> {
    if let Some(events) = payload.get("events").and_then(Value::as_array) {
        return events.iter().collect();
    }
    if let Some(event) = payload.get("event").filter(|value| value.is_object()) {
        return vec![event];
    }
    if let Some(data) = payload.get("data").filter(|value| value.is_object()) {
        return vec![data];
    }
    if let Some(body) = payload.get("body").filter(|value| value.is_object()) {
        return vec![body];
    }
    if let Some(d) = payload.get("d").filter(|value| value.is_object()) {
        return vec![d];
    }
    vec![payload]
}

fn session_from_value(
    platform_id: &str,
    value: &Value,
    sender_id: &str,
) -> (MessageSession, Option<PlatformGroupIdentityInput>) {
    let group_id = find_string(
        value,
        &[
            "group_id",
            "message.chat.id",
            "message.chat_id",
            "chat.id",
            "channel",
            "channel_id",
            "message.channel_id",
            "message.guild_id",
            "guild_id",
            "body.guild.id",
            "body.channel.id",
            "d.channel_id",
            "target_id",
            "group_openid",
            "open_kfid",
            "open_conversation_id",
            "conversation_id",
            "source.groupId",
            "conversationType",
        ],
    )
    .filter(|group| group != sender_id && !group.eq_ignore_ascii_case("1"));
    let direct_id = find_string(
        value,
        &[
            "chat_id",
            "user_id",
            "sender_id",
            "author_id",
            "author.id",
            "message.author.id",
            "body.user.id",
            "d.author_id",
            "extra.author.id",
            "extra.author_id",
            "message.from.id",
            "from.id",
            "user",
            "user.id",
            "source.userId",
            "senderStaffId",
            "sender.staff_id",
            "openid",
            "open_id",
            "external_userid",
            "FromUserName",
        ],
    )
    .unwrap_or_else(|| sender_id.to_string());
    if let Some(group_id) = group_id {
        let session = MessageSession::group(platform_id, format!("group:{group_id}"));
        let group = PlatformGroupIdentityInput::new(group_id).with_member(
            PlatformIdentityNormalizer::normalize_sender(sender_id.to_string(), None),
        );
        (session, Some(group))
    } else {
        (
            MessageSession::new(platform_id, format!("private:{direct_id}")),
            None,
        )
    }
}

fn sender_id(value: &Value) -> String {
    find_string(
        value,
        &[
            "sender_id",
            "sender.sender_id",
            "sender.senderId",
            "senderStaffId",
            "author_id",
            "author.id",
            "message.author.id",
            "body.user.id",
            "d.author_id",
            "extra.author.id",
            "extra.author_id",
            "message.from.id",
            "from.id",
            "user_id",
            "user",
            "user.id",
            "source.userId",
            "userId",
            "operator_id",
            "openid",
            "open_id",
            "external_userid",
            "FromUserName",
        ],
    )
    .unwrap_or_else(|| "unknown".to_string())
}

fn message_chain_from_value(value: &Value) -> MessageChain {
    let mut components = Vec::new();
    push_text_components(value, &mut components);
    push_reply_components(value, &mut components);
    push_media_components(value, &mut components);
    push_line_message(value, &mut components);
    push_lark_content(value, &mut components);
    push_satori_content(value, &mut components);
    push_slack_files(value, &mut components);
    MessageChain::new(components)
}

fn push_text_components(value: &Value, components: &mut Vec<MessageComponent>) {
    for path in [
        "text",
        "message.text",
        "content.text",
        "text.content",
        "content",
        "message.content.text",
        "message.content",
        "body.text",
        "body.message.content",
        "d.content",
        "Content",
    ] {
        if let Some(text) = find_string(value, &[path])
            && !looks_like_json_object(&text)
        {
            components.push(MessageComponent::plain(text));
            return;
        }
    }
}

fn push_reply_components(value: &Value, components: &mut Vec<MessageComponent>) {
    if let Some(message_id) = find_string(
        value,
        &[
            "reply_to_message_id",
            "reply.message_id",
            "replyToken",
            "thread_ts",
            "message.reply_to_message.message_id",
            "message_reference.message_id",
            "message.message_reference.message_id",
            "body.message.quote.id",
            "body.message.quote.message.id",
            "body.replyId",
            "replyId",
        ],
    ) {
        components.push(MessageComponent::reply(message_id, ""));
    }
}

fn push_media_components(value: &Value, components: &mut Vec<MessageComponent>) {
    if let Some(url) = find_string(
        value,
        &[
            "image_url",
            "image.url",
            "picUrl",
            "PicUrl",
            "attachments.0.url",
            "attachments.0.file.url",
            "message.attachments.0.url",
            "message.attachments.0.proxy_url",
            "files.0.url",
            "files.0.url_private",
            "body.files.0.url",
            "photo.0.file_id",
            "message.photo.0.file_id",
        ],
    ) {
        components.push(MessageComponent::image(url));
    }
    if let Some(url) = find_string(
        value,
        &[
            "voice_url",
            "audio_url",
            "record.url",
            "audio.url",
            "attachments.0.audio_url",
            "voice.media_id",
            "MediaId",
            "message.voice.file_id",
        ],
    ) {
        components.push(MessageComponent::record(url));
    }
    if let Some(url) = find_string(
        value,
        &[
            "file_url",
            "file.url",
            "attachment.url",
            "attachments.0.url",
            "message.attachments.0.url",
            "files.0.url",
            "body.files.0.url",
            "document.file_id",
            "file.file_id",
            "message.document.file_id",
        ],
    ) {
        let name = find_string(
            value,
            &[
                "file_name",
                "file.name",
                "filename",
                "attachment.name",
                "attachments.0.filename",
                "message.attachments.0.filename",
                "files.0.name",
                "body.files.0.name",
                "document.file_name",
                "message.document.file_name",
            ],
        )
        .unwrap_or_else(|| "file".to_string());
        components.push(MessageComponent::file(name, url));
    }
}

fn push_line_message(value: &Value, components: &mut Vec<MessageComponent>) {
    let Some(message) = value.get("message") else {
        return;
    };
    let message_type = message
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let id = message
        .get("id")
        .and_then(value_as_string)
        .unwrap_or_else(|| "line-media".to_string());
    match message_type {
        "text" => {
            if let Some(text) = message.get("text").and_then(value_as_string) {
                push_plain_once(components, text);
            }
        }
        "image" => components.push(MessageComponent::image(id)),
        "audio" => components.push(MessageComponent::record(id)),
        "file" => {
            let name = message
                .get("fileName")
                .and_then(value_as_string)
                .unwrap_or_else(|| "file".to_string());
            components.push(MessageComponent::file(name, id));
        }
        _ => {}
    }
}

fn push_plain_once(components: &mut Vec<MessageComponent>, text: String) {
    if components.iter().any(|component| {
        matches!(
            component,
            MessageComponent::Plain { text: known } if known == &text
        )
    }) {
        return;
    }
    components.push(MessageComponent::plain(text));
}

fn push_lark_content(value: &Value, components: &mut Vec<MessageComponent>) {
    let Some(content) = find_string(value, &["message.content", "content"]) else {
        return;
    };
    let Ok(parsed) = serde_json::from_str::<Value>(&content) else {
        return;
    };
    if let Some(text) = find_string(&parsed, &["text", "content"]) {
        components.push(MessageComponent::plain(text));
    }
    if let Some(image) = find_string(&parsed, &["image_key", "image.url"]) {
        components.push(MessageComponent::image(image));
    }
    if let Some(file) = find_string(&parsed, &["file_key", "file.url"]) {
        let name =
            find_string(&parsed, &["file_name", "file.name"]).unwrap_or_else(|| "file".to_string());
        components.push(MessageComponent::file(name, file));
    }
    if let Some(audio) = find_string(&parsed, &["audio_key", "voice_key"]) {
        components.push(MessageComponent::record(audio));
    }
}

fn push_satori_content(value: &Value, components: &mut Vec<MessageComponent>) {
    let Some(content) = find_string(value, &["message.content", "body.message.content"]) else {
        return;
    };
    if !content.contains('<') {
        return;
    }
    if let Some(text) = tag_attr(&content, "text", "content") {
        components.push(MessageComponent::plain(text));
    }
    if let Some(src) = tag_attr(&content, "img", "src") {
        components.push(MessageComponent::image(src));
    }
    if let Some(src) = tag_attr(&content, "audio", "src") {
        components.push(MessageComponent::record(src));
    }
    if let Some(src) = tag_attr(&content, "file", "src") {
        let name = tag_attr(&content, "file", "title").unwrap_or_else(|| "file".to_string());
        components.push(MessageComponent::file(name, src));
    }
    if let Some(id) = tag_attr(&content, "quote", "id") {
        components.push(MessageComponent::reply(id, ""));
    }
}

fn push_slack_files(value: &Value, components: &mut Vec<MessageComponent>) {
    let Some(files) = value.get("files").and_then(Value::as_array) else {
        return;
    };
    for file in files {
        let url = find_string(file, &["url_private", "url_private_download", "permalink"]);
        let Some(url) = url else {
            continue;
        };
        let mimetype = find_string(file, &["mimetype"]).unwrap_or_default();
        let name = find_string(file, &["name", "title"]).unwrap_or_else(|| "file".to_string());
        if mimetype.starts_with("image/") {
            components.push(MessageComponent::image(url));
        } else if mimetype.starts_with("audio/") {
            components.push(MessageComponent::record(url));
        } else {
            components.push(MessageComponent::file(name, url));
        }
    }
}

fn tag_attr(content: &str, tag: &str, attr: &str) -> Option<String> {
    let start = content.find(&format!("<{tag}"))?;
    let after_tag = &content[start..];
    let end = after_tag.find('>').unwrap_or(after_tag.len());
    let head = &after_tag[..end];
    let marker = format!("{attr}=\"");
    let attr_start = head.find(&marker)? + marker.len();
    let attr_rest = &head[attr_start..];
    let attr_end = attr_rest.find('"')?;
    let value = attr_rest[..attr_end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn find_string(value: &Value, paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|path| {
        let mut cursor = value;
        for segment in path.split('.') {
            if let Ok(index) = segment.parse::<usize>() {
                cursor = cursor.get(index)?;
            } else {
                cursor = cursor.get(segment)?;
            }
        }
        value_as_string(cursor)
    })
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn looks_like_json_object(value: &str) -> bool {
    let value = value.trim_start();
    value.starts_with('{') || value.starts_with('[')
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        let lhs = left.get(index).copied().unwrap_or_default();
        let rhs = right.get(index).copied().unwrap_or_default();
        diff |= usize::from(lhs ^ rhs);
    }
    diff == 0
}
