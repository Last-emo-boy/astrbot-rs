use std::sync::Arc;

use astrbot_core::{
    EventExecutor, MessageChain, MessageEvent, MessageEventResult, MessageSender, MessageSession,
    Result,
};
use astrbot_pipeline::{
    BaiduAipContentSafetyStrategy, ContentSafetyConfig, KeywordContentSafetyStrategy,
    PipelineContext, PipelineScheduler,
    stages::{ContentSafetyCheckStage, PluginStage, ProviderStage, RespondStage},
};
use astrbot_platform::RecordingSink;
use astrbot_plugin::{
    CommandFilter, HandlerMetadata, PluginControl, PluginEventType, PluginHandler, PluginRegistry,
    RegisteredHandler,
};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[tokio::test]
async fn blocked_wake_message_sends_rejection_and_skips_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_content_safety(
            keyword_safety(["bad"]).with_rejection_message("blocked by content safety"),
        ),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);
    let mut event = direct_event("bad request", sink.clone());
    event.mark_wake(true);

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert_eq!(
        sink.messages().await[0].chain.plain_text(),
        "blocked by content safety"
    );
}

#[tokio::test]
async fn blocked_non_wake_message_stops_silently() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_content_safety(keyword_safety(["bad"])),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("bad request", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn safe_message_reaches_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_content_safety(keyword_safety(["bad"])),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("ordinary request", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert_eq!(provider.requests.lock().await.len(), 1);
    assert_eq!(sink.messages().await[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn baidu_aip_strategy_allows_clean_content_from_fake_server() {
    let base_url = serve_baidu_sequence(
        r#"{"access_token":"test-token"}"#,
        r#"{"conclusionType":1,"conclusion":"合规"}"#,
    )
    .await;
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_content_safety(
            baidu_safety(&base_url).with_rejection_message("blocked by baidu"),
        ),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("ordinary request", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert_eq!(provider.requests.lock().await.len(), 1);
    assert_eq!(sink.messages().await[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn baidu_aip_strategy_blocks_wake_content_from_fake_server() {
    let base_url = serve_baidu_sequence(
        r#"{"access_token":"test-token"}"#,
        r#"{"conclusionType":2,"conclusion":"不合规","data":[{"msg":"违规词"}]}"#,
    )
    .await;
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_content_safety(
            baidu_safety(&base_url).with_rejection_message("blocked by baidu"),
        ),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);
    let mut event = direct_event("unsafe request", sink.clone());
    event.mark_wake(true);

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert_eq!(
        sink.messages().await[0].chain.plain_text(),
        "blocked by baidu"
    );
}

#[tokio::test]
async fn content_safety_result_prevents_plugin_override() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let mut plugins = PluginRegistry::new();
    plugins.register_handler(
        RegisteredHandler::new(
            HandlerMetadata::new("builtin", "bad", PluginEventType::AdapterMessage),
            Arc::new(StaticReplyHandler {
                reply: "plugin-response",
            }),
        )
        .with_filter(CommandFilter::new("bad")),
    );
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_content_safety(
                keyword_safety(["bad"]).with_rejection_message("blocked by content safety"),
            )
            .with_plugin_registry(Arc::new(plugins)),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(PluginStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);
    let mut event = direct_event("/bad", sink.clone());
    event.mark_wake(true);

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert_eq!(
        sink.messages().await[0].chain.plain_text(),
        "blocked by content safety"
    );
}

fn keyword_safety<I, S>(keywords: I) -> ContentSafetyConfig
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    ContentSafetyConfig::default()
        .with_strategy(Arc::new(KeywordContentSafetyStrategy::new(keywords)))
}

fn baidu_safety(base_url: &str) -> ContentSafetyConfig {
    ContentSafetyConfig::default().with_strategy(Arc::new(
        BaiduAipContentSafetyStrategy::new("app-id", "api-key", "secret-key").with_endpoints(
            format!("{base_url}/oauth/2.0/token"),
            format!("{base_url}/rest/2.0/solution/v1/text_censor/v2/user_defined"),
        ),
    ))
}

fn direct_event(text: impl Into<String>, sink: Arc<RecordingSink>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::plain(text),
        sink,
    )
}

#[derive(Default)]
struct CapturingProvider {
    requests: Mutex<Vec<ChatRequest>>,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.requests.lock().await.push(request);
        Ok(ChatResponse::text("mock-response"))
    }
}

struct StaticReplyHandler {
    reply: &'static str,
}

#[async_trait]
impl PluginHandler for StaticReplyHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_result(MessageEventResult::general(self.reply));
        Ok(PluginControl::Continue)
    }
}

async fn serve_baidu_sequence(token_body: &'static str, censor_body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test server should bind");
    let addr = listener.local_addr().expect("test server should have addr");
    tokio::spawn(async move {
        for body in [token_body, censor_body] {
            let (mut stream, _) = listener.accept().await.expect("server should accept");
            read_http_request(&mut stream).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("server should respond");
        }
    });
    format!("http://{addr}")
}

async fn read_http_request(stream: &mut TcpStream) {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream.read(&mut buffer).await.expect("server should read");
        assert_ne!(read, 0, "client closed before sending request");
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let header_end = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("request should contain headers")
        + 4;
    let content_length = content_length(&request);
    while request.len() < header_end + content_length {
        let read = stream
            .read(&mut buffer)
            .await
            .expect("server should read body");
        assert_ne!(read, 0, "client closed before sending body");
        request.extend_from_slice(&buffer[..read]);
    }
}

fn content_length(request: &[u8]) -> usize {
    String::from_utf8_lossy(request)
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
        .unwrap_or(0)
}
