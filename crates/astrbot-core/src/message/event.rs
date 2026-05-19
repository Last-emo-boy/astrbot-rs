use std::sync::Arc;

use crate::Result;

use super::chain::MessageChain;
use super::identity::PlatformIdentity;
use super::provider_request::ProviderRequest;
use super::result::{MessageEventResult, MessageStream};
use super::session::{MessageSender, MessageSession};
use super::sink::MessageSink;

#[derive(Clone)]
pub struct MessageEvent {
    pub id: String,
    pub platform_id: String,
    pub platform_name: String,
    pub session: MessageSession,
    pub sender: MessageSender,
    pub identity: Option<PlatformIdentity>,
    pub message: MessageChain,
    self_id: Option<String>,
    is_wake: bool,
    is_at_or_wake_command: bool,
    result: Option<MessageEventResult>,
    provider_request: Option<ProviderRequest>,
    stopped: bool,
    streaming_finished: bool,
    sink: Arc<dyn MessageSink>,
}

impl MessageEvent {
    pub fn new(
        id: impl Into<String>,
        platform_id: impl Into<String>,
        platform_name: impl Into<String>,
        session: MessageSession,
        sender: MessageSender,
        message: MessageChain,
        sink: Arc<dyn MessageSink>,
    ) -> Self {
        Self {
            id: id.into(),
            platform_id: platform_id.into(),
            platform_name: platform_name.into(),
            session,
            sender,
            identity: None,
            message,
            self_id: None,
            is_wake: false,
            is_at_or_wake_command: false,
            result: None,
            provider_request: None,
            stopped: false,
            streaming_finished: false,
            sink,
        }
    }

    pub fn with_self_id(mut self, self_id: impl Into<String>) -> Self {
        self.self_id = Some(self_id.into());
        self
    }

    pub fn with_identity(mut self, identity: PlatformIdentity) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn identity(&self) -> Option<&PlatformIdentity> {
        self.identity.as_ref()
    }

    pub fn identity_mut(&mut self) -> Option<&mut PlatformIdentity> {
        self.identity.as_mut()
    }

    pub fn set_identity(&mut self, identity: PlatformIdentity) {
        self.identity = Some(identity);
    }

    pub fn take_identity(&mut self) -> Option<PlatformIdentity> {
        self.identity.take()
    }

    pub fn self_id(&self) -> Option<&str> {
        self.self_id.as_deref()
    }

    pub fn mark_wake(&mut self, is_at_or_wake_command: bool) {
        self.is_wake = true;
        self.is_at_or_wake_command = is_at_or_wake_command;
    }

    pub fn is_wake(&self) -> bool {
        self.is_wake
    }

    pub fn is_at_or_wake_command(&self) -> bool {
        self.is_at_or_wake_command
    }

    pub fn message_outline(&self) -> String {
        self.message.plain_text()
    }

    pub fn result(&self) -> Option<&MessageEventResult> {
        self.result.as_ref()
    }

    pub fn result_mut(&mut self) -> Option<&mut MessageEventResult> {
        self.result.as_mut()
    }

    pub fn set_result(&mut self, result: MessageEventResult) {
        self.result = Some(result);
    }

    pub fn take_result(&mut self) -> Option<MessageEventResult> {
        self.result.take()
    }

    pub fn provider_request(&self) -> Option<&ProviderRequest> {
        self.provider_request.as_ref()
    }

    pub fn set_provider_request(&mut self, provider_request: ProviderRequest) {
        self.provider_request = Some(provider_request);
    }

    pub fn take_provider_request(&mut self) -> Option<ProviderRequest> {
        self.provider_request.take()
    }

    pub fn clear_provider_request(&mut self) {
        self.provider_request = None;
    }

    pub fn stop(&mut self) {
        self.stopped = true;
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn mark_streaming_finished(&mut self) {
        self.streaming_finished = true;
    }

    pub fn is_streaming_finished(&self) -> bool {
        self.streaming_finished
    }

    pub async fn send(&self, chain: MessageChain) -> Result<()> {
        self.sink.send(&self.session, chain).await
    }

    pub async fn send_streaming(&self, stream: MessageStream) -> Result<()> {
        self.sink.send_streaming(&self.session, stream).await
    }
}
