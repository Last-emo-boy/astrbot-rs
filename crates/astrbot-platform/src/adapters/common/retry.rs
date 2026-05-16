use std::time::Duration;

use super::api_client::PlatformApiErrorKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformRetryReason {
    Connection,
    RateLimited,
    ServerError,
    WebSocketClosed,
}

impl PlatformRetryReason {
    pub fn from_api_error_kind(kind: PlatformApiErrorKind) -> Option<Self> {
        match kind {
            PlatformApiErrorKind::Connection => Some(Self::Connection),
            PlatformApiErrorKind::RateLimited => Some(Self::RateLimited),
            PlatformApiErrorKind::Server => Some(Self::ServerError),
            PlatformApiErrorKind::WebSocket => Some(Self::WebSocketClosed),
            PlatformApiErrorKind::Authentication
            | PlatformApiErrorKind::NotFound
            | PlatformApiErrorKind::InvalidResponse
            | PlatformApiErrorKind::Unknown => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformRetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl PlatformRetryPolicy {
    pub fn new(max_attempts: u32, initial_delay: Duration, max_delay: Duration) -> Self {
        let initial_delay = initial_delay.max(Duration::from_millis(1));
        Self {
            max_attempts: max_attempts.max(1),
            initial_delay,
            max_delay: max_delay.max(initial_delay),
        }
    }

    pub fn delay_after_failure(
        &self,
        failed_attempt: u32,
        reason: PlatformRetryReason,
    ) -> Option<Duration> {
        let failed_attempt = failed_attempt.max(1);
        if failed_attempt >= self.max_attempts {
            return None;
        }

        let exponent = failed_attempt.saturating_sub(1).min(20);
        let multiplier = match reason {
            PlatformRetryReason::RateLimited => 3_u32.saturating_pow(exponent),
            PlatformRetryReason::Connection
            | PlatformRetryReason::ServerError
            | PlatformRetryReason::WebSocketClosed => {
                1_u32.checked_shl(exponent).unwrap_or(u32::MAX)
            }
        };

        Some(
            self.initial_delay
                .saturating_mul(multiplier)
                .min(self.max_delay),
        )
    }

    pub fn decision_after_failure(
        &self,
        failed_attempt: u32,
        reason: PlatformRetryReason,
    ) -> PlatformRetryDecision {
        let delay = self.delay_after_failure(failed_attempt, reason);
        PlatformRetryDecision {
            should_retry: delay.is_some(),
            reason,
            delay,
        }
    }
}

impl Default for PlatformRetryPolicy {
    fn default() -> Self {
        Self::new(3, Duration::from_secs(1), Duration::from_secs(30))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformRateLimit {
    pub key: String,
    pub remaining: Option<u32>,
    pub reset_after: Option<Duration>,
}

impl PlatformRateLimit {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            remaining: None,
            reset_after: None,
        }
    }

    pub fn with_remaining(mut self, remaining: u32) -> Self {
        self.remaining = Some(remaining);
        self
    }

    pub fn with_reset_after(mut self, reset_after: Duration) -> Self {
        self.reset_after = Some(reset_after);
        self
    }

    pub fn exhausted(&self) -> bool {
        self.remaining == Some(0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformRetryDecision {
    pub should_retry: bool,
    pub reason: PlatformRetryReason,
    pub delay: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{PlatformRateLimit, PlatformRetryPolicy, PlatformRetryReason};
    use crate::PlatformApiErrorKind;

    #[test]
    fn retry_policy_uses_different_backoff_for_rate_limits() {
        let policy =
            PlatformRetryPolicy::new(3, Duration::from_millis(100), Duration::from_secs(10));

        assert_eq!(
            policy.delay_after_failure(1, PlatformRetryReason::Connection),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            policy.delay_after_failure(2, PlatformRetryReason::Connection),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            policy.delay_after_failure(2, PlatformRetryReason::RateLimited),
            Some(Duration::from_millis(300))
        );
        assert_eq!(
            policy.delay_after_failure(3, PlatformRetryReason::RateLimited),
            None
        );
    }

    #[test]
    fn retry_reason_does_not_retry_authentication_or_not_found_errors() {
        assert_eq!(
            PlatformRetryReason::from_api_error_kind(PlatformApiErrorKind::RateLimited),
            Some(PlatformRetryReason::RateLimited)
        );
        assert_eq!(
            PlatformRetryReason::from_api_error_kind(PlatformApiErrorKind::Authentication),
            None
        );
        assert_eq!(
            PlatformRetryReason::from_api_error_kind(PlatformApiErrorKind::NotFound),
            None
        );
    }

    #[test]
    fn rate_limit_state_reports_exhausted_buckets() {
        let bucket = PlatformRateLimit::new("telegram:send")
            .with_remaining(0)
            .with_reset_after(Duration::from_secs(2));

        assert!(bucket.exhausted());
        assert_eq!(bucket.reset_after.expect("reset after").as_secs(), 2);
    }
}
