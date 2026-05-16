use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RealtimeProcessingState {
    #[default]
    Idle,
    Processing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealtimeSubscription {
    pub chat_session_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealtimeConnectionSession {
    session_id: String,
    username: String,
    conversation_id: Option<String>,
    processing_state: RealtimeProcessingState,
    should_interrupt: bool,
    subscriptions: BTreeMap<String, RealtimeSubscription>,
}

impl RealtimeConnectionSession {
    pub fn new(session_id: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            username: username.into(),
            conversation_id: None,
            processing_state: RealtimeProcessingState::Idle,
            should_interrupt: false,
            subscriptions: BTreeMap::new(),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation_id.as_deref()
    }

    pub fn bind_conversation(&mut self, conversation_id: impl Into<String>) {
        self.conversation_id = non_empty_string(conversation_id);
    }

    pub fn processing_state(&self) -> RealtimeProcessingState {
        self.processing_state
    }

    pub fn is_processing(&self) -> bool {
        self.processing_state == RealtimeProcessingState::Processing
    }

    pub fn start_processing(&mut self) {
        self.processing_state = RealtimeProcessingState::Processing;
        self.should_interrupt = false;
    }

    pub fn finish_processing(&mut self) {
        self.processing_state = RealtimeProcessingState::Idle;
        self.should_interrupt = false;
    }

    pub fn request_interrupt(&mut self) -> bool {
        self.should_interrupt = true;
        self.is_processing()
    }

    pub fn should_interrupt(&self) -> bool {
        self.should_interrupt
    }

    pub fn take_interrupt(&mut self) -> bool {
        let should_interrupt = self.should_interrupt;
        self.should_interrupt = false;
        should_interrupt
    }

    pub fn bind_subscription(
        &mut self,
        chat_session_id: impl Into<String>,
        request_id: impl Into<String>,
    ) -> Option<RealtimeSubscription> {
        let chat_session_id = non_empty_string(chat_session_id)?;
        let request_id = non_empty_string(request_id)?;
        let subscription = RealtimeSubscription {
            chat_session_id: chat_session_id.clone(),
            request_id,
        };
        self.subscriptions
            .insert(chat_session_id, subscription.clone());
        Some(subscription)
    }

    pub fn subscription(&self, chat_session_id: &str) -> Option<&RealtimeSubscription> {
        self.subscriptions.get(chat_session_id.trim())
    }

    pub fn subscriptions(&self) -> impl Iterator<Item = &RealtimeSubscription> {
        self.subscriptions.values()
    }

    pub fn remove_subscription(&mut self, chat_session_id: &str) -> Option<RealtimeSubscription> {
        self.subscriptions.remove(chat_session_id.trim())
    }

    pub fn clear_subscriptions(&mut self) {
        self.subscriptions.clear();
    }
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
