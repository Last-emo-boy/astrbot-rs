use std::collections::{HashMap, VecDeque};

use astrbot_core::Result;
use async_trait::async_trait;
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlatformQueueDirection {
    Inbound,
    Outbound,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformQueueItem {
    pub route_key: String,
    pub direction: PlatformQueueDirection,
    pub payload: Vec<u8>,
    pub metadata: Vec<(String, String)>,
}

impl PlatformQueueItem {
    pub fn new(
        route_key: impl Into<String>,
        direction: PlatformQueueDirection,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            route_key: route_key.into(),
            direction,
            payload: payload.into(),
            metadata: Vec::new(),
        }
    }

    pub fn text(
        route_key: impl Into<String>,
        direction: PlatformQueueDirection,
        payload: impl Into<String>,
    ) -> Self {
        Self::new(route_key, direction, payload.into().into_bytes())
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push((key.into(), value.into()));
        self
    }

    pub fn metadata_value(&self, key: &str) -> Option<&str> {
        self.metadata
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingWebhookResponse {
    pub route_key: String,
    pub payload: Vec<u8>,
    pub metadata: Vec<(String, String)>,
}

impl PendingWebhookResponse {
    pub fn new(route_key: impl Into<String>, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            route_key: route_key.into(),
            payload: payload.into(),
            metadata: Vec::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push((key.into(), value.into()));
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlatformQueueStats {
    pub inbound_items: usize,
    pub outbound_items: usize,
    pub pending_responses: usize,
}

#[derive(Default)]
pub struct InMemoryPlatformQueueStore {
    queues: Mutex<HashMap<(String, PlatformQueueDirection), VecDeque<PlatformQueueItem>>>,
    pending_responses: Mutex<HashMap<String, PendingWebhookResponse>>,
}

#[async_trait]
pub trait PlatformCallbackQueue: Send + Sync {
    async fn enqueue(&self, callback: PlatformQueueItem) -> Result<()>;

    async fn dequeue(
        &self,
        route_key: &str,
        direction: PlatformQueueDirection,
    ) -> Result<Option<PlatformQueueItem>>;
}

impl InMemoryPlatformQueueStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn push(&self, item: PlatformQueueItem) {
        let key = (item.route_key.clone(), item.direction.clone());
        self.queues
            .lock()
            .await
            .entry(key)
            .or_default()
            .push_back(item);
    }

    pub async fn pop(
        &self,
        route_key: &str,
        direction: PlatformQueueDirection,
    ) -> Option<PlatformQueueItem> {
        self.queues
            .lock()
            .await
            .get_mut(&(route_key.to_string(), direction))
            .and_then(VecDeque::pop_front)
    }

    pub async fn set_pending_response(&self, response: PendingWebhookResponse) {
        self.pending_responses
            .lock()
            .await
            .insert(response.route_key.clone(), response);
    }

    pub async fn take_pending_response(&self, route_key: &str) -> Option<PendingWebhookResponse> {
        self.pending_responses.lock().await.remove(route_key)
    }

    pub async fn stats(&self) -> PlatformQueueStats {
        let queues = self.queues.lock().await;
        let inbound_items = queues
            .iter()
            .filter(|((_, direction), _)| direction == &PlatformQueueDirection::Inbound)
            .map(|(_, queue)| queue.len())
            .sum();
        let outbound_items = queues
            .iter()
            .filter(|((_, direction), _)| direction == &PlatformQueueDirection::Outbound)
            .map(|(_, queue)| queue.len())
            .sum();
        drop(queues);

        PlatformQueueStats {
            inbound_items,
            outbound_items,
            pending_responses: self.pending_responses.lock().await.len(),
        }
    }
}

#[async_trait]
impl PlatformCallbackQueue for InMemoryPlatformQueueStore {
    async fn enqueue(&self, callback: PlatformQueueItem) -> Result<()> {
        self.push(callback).await;
        Ok(())
    }

    async fn dequeue(
        &self,
        route_key: &str,
        direction: PlatformQueueDirection,
    ) -> Result<Option<PlatformQueueItem>> {
        Ok(self.pop(route_key, direction).await)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InMemoryPlatformQueueStore, PendingWebhookResponse, PlatformCallbackQueue,
        PlatformQueueDirection, PlatformQueueItem, PlatformQueueStats,
    };

    #[test]
    fn queue_item_tracks_route_direction_payload_and_metadata() {
        let item = PlatformQueueItem::text("session-1", PlatformQueueDirection::Inbound, "payload")
            .with_metadata("nonce", "n-1");

        assert_eq!(item.route_key, "session-1");
        assert_eq!(item.direction, PlatformQueueDirection::Inbound);
        assert_eq!(item.payload, b"payload".to_vec());
        assert_eq!(item.metadata_value("nonce"), Some("n-1"));
    }

    #[tokio::test]
    async fn queue_store_separates_inbound_outbound_and_pending_responses() {
        let store = InMemoryPlatformQueueStore::new();
        store
            .push(PlatformQueueItem::text(
                "session-1",
                PlatformQueueDirection::Inbound,
                "inbound",
            ))
            .await;
        store
            .push(PlatformQueueItem::text(
                "session-1",
                PlatformQueueDirection::Outbound,
                "outbound",
            ))
            .await;
        store
            .set_pending_response(
                PendingWebhookResponse::new("session-1", b"response".to_vec())
                    .with_metadata("timestamp", "1"),
            )
            .await;

        assert_eq!(
            store.stats().await,
            PlatformQueueStats {
                inbound_items: 1,
                outbound_items: 1,
                pending_responses: 1,
            }
        );
        assert_eq!(
            store
                .pop("session-1", PlatformQueueDirection::Inbound)
                .await
                .expect("inbound item should exist")
                .payload,
            b"inbound".to_vec()
        );
        assert_eq!(
            store
                .take_pending_response("session-1")
                .await
                .expect("pending response should exist")
                .payload,
            b"response".to_vec()
        );
    }

    #[tokio::test]
    async fn queue_trait_wraps_store_push_and_pop() {
        let queue = InMemoryPlatformQueueStore::new();
        let callback =
            PlatformQueueItem::text("session-1", PlatformQueueDirection::Inbound, "payload");

        queue
            .enqueue(callback.clone())
            .await
            .expect("callback should enqueue");

        assert_eq!(
            queue
                .dequeue("session-1", PlatformQueueDirection::Inbound)
                .await
                .expect("callback should dequeue"),
            Some(callback)
        );
    }
}
