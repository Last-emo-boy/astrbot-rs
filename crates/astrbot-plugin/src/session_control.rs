use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use astrbot_core::{AstrbotError, MessageChain, MessageComponent, MessageEvent, Result};
use tokio::sync::{Mutex, oneshot};
use tokio::time;

#[derive(Default)]
pub struct SessionWaiterRegistry {
    next_waiter_id: AtomicU64,
    waiters: Mutex<HashMap<String, Vec<SessionWaiter>>>,
}

struct SessionWaiter {
    id: u64,
    sender: oneshot::Sender<MessageEvent>,
}

impl SessionWaiterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn wait_for_next(
        &self,
        session_id: impl Into<String>,
        timeout: Duration,
    ) -> Result<Option<MessageEvent>> {
        let session_id = session_id.into();
        let (waiter_id, receiver) = self.register(session_id.clone()).await;
        match time::timeout(timeout.max(Duration::from_millis(1)), receiver).await {
            Ok(Ok(event)) => Ok(Some(event)),
            Ok(Err(_)) => Err(AstrbotError::EventChannelClosed),
            Err(_) => {
                self.unregister(&session_id, waiter_id).await;
                Ok(None)
            }
        }
    }

    pub async fn trigger(&self, event: &MessageEvent) -> bool {
        let waiters = self
            .waiters
            .lock()
            .await
            .remove(&event.session.conversation_id)
            .unwrap_or_default();
        let mut delivered = false;
        for waiter in waiters {
            delivered |= waiter.sender.send(event.clone()).is_ok();
        }
        delivered
    }

    pub async fn waiter_count(&self) -> usize {
        self.waiters.lock().await.values().map(Vec::len).sum()
    }

    async fn register(
        &self,
        session_id: impl Into<String>,
    ) -> (u64, oneshot::Receiver<MessageEvent>) {
        let waiter_id = self.next_waiter_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        self.waiters
            .lock()
            .await
            .entry(session_id.into())
            .or_default()
            .push(SessionWaiter {
                id: waiter_id,
                sender,
            });
        (waiter_id, receiver)
    }

    async fn unregister(&self, session_id: &str, waiter_id: u64) {
        let mut waiters = self.waiters.lock().await;
        let Some(session_waiters) = waiters.get_mut(session_id) else {
            return;
        };
        session_waiters.retain(|waiter| waiter.id != waiter_id);
        if session_waiters.is_empty() {
            waiters.remove(session_id);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EmptyMentionPolicy {
    wake_prefixes: Vec<String>,
}

impl EmptyMentionPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_wake_prefix(mut self, wake_prefix: impl Into<String>) -> Self {
        let wake_prefix = wake_prefix.into().trim().to_string();
        if !wake_prefix.is_empty() && !self.wake_prefixes.iter().any(|known| known == &wake_prefix)
        {
            self.wake_prefixes.push(wake_prefix);
        }
        self
    }

    pub fn should_wait_for_followup(&self, event: &MessageEvent) -> bool {
        self.is_self_mention_only(event) || self.is_wake_prefix_only(event)
    }

    pub fn redispatch_followup(&self, event: &MessageEvent) -> MessageEvent {
        let mut next = event.clone();
        if let Some(self_id) = event.self_id()
            && !next.message.mentions_user(self_id)
        {
            let mut components = vec![MessageComponent::mention(self_id)];
            components.extend(next.message.components().iter().cloned());
            next.message = MessageChain::new(components);
            next.mark_wake(true);
        }
        next
    }

    fn is_self_mention_only(&self, event: &MessageEvent) -> bool {
        let Some(self_id) = event.self_id() else {
            return false;
        };
        let components = event.message.components();
        components.len() == 1
            && matches!(
                &components[0],
                MessageComponent::Mention { user_id } if user_id == self_id
            )
    }

    fn is_wake_prefix_only(&self, event: &MessageEvent) -> bool {
        let plain = event.message.plain_text();
        let plain = plain.trim();
        !plain.is_empty()
            && self
                .wake_prefixes
                .iter()
                .any(|prefix| prefix.as_str() == plain)
    }
}
