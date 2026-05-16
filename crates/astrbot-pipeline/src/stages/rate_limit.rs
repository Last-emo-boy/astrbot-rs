use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::{PipelineContext, PipelineControl, PipelineStage, RateLimitStrategy};

#[derive(Default)]
pub struct RateLimitStage {
    event_timestamps: Mutex<HashMap<String, VecDeque<Instant>>>,
}

#[async_trait]
impl PipelineStage for RateLimitStage {
    fn name(&self) -> &str {
        "rate_limit"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        let config = ctx.rate_limit();
        if !config.enabled || config.max_events == 0 || config.window.is_zero() {
            return Ok(PipelineControl::Continue);
        }

        let session_key = rate_limit_session_key(event);
        loop {
            let now = Instant::now();
            let wait = {
                let mut all_timestamps = self.event_timestamps.lock().await;
                let timestamps = all_timestamps.entry(session_key.clone()).or_default();
                remove_expired_timestamps(timestamps, now, config.window);

                if timestamps.len() < config.max_events {
                    timestamps.push_back(now);
                    return Ok(PipelineControl::Continue);
                }

                match config.strategy {
                    RateLimitStrategy::Discard => {
                        event.stop();
                        return Ok(PipelineControl::Stop);
                    }
                    RateLimitStrategy::Stall => timestamps
                        .front()
                        .map(|oldest| {
                            let elapsed = now.duration_since(*oldest);
                            config.window.saturating_sub(elapsed)
                        })
                        .unwrap_or(Duration::ZERO),
                }
            };

            if wait.is_zero() {
                continue;
            }
            sleep(wait).await;
        }
    }
}

fn rate_limit_session_key(event: &MessageEvent) -> String {
    format!(
        "{}:{}",
        event.session.platform_id, event.session.conversation_id
    )
}

fn remove_expired_timestamps(timestamps: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while timestamps
        .front()
        .is_some_and(|timestamp| now.duration_since(*timestamp) >= window)
    {
        timestamps.pop_front();
    }
}
