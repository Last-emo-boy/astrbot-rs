use astrbot_core::MessageSessionKind;

use crate::MemorySessionKey;

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveReplyPolicy {
    pub enabled: bool,
    pub method: ActiveReplyMethod,
    pub probability: f32,
    pub whitelist: Vec<String>,
    pub group_only: bool,
    pub min_messages_in_window: Option<usize>,
    pub window_seconds: Option<u64>,
    pub min_seconds_since_last_reply: Option<u64>,
    pub reply_on_at_or_wake: bool,
}

impl ActiveReplyPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            method: ActiveReplyMethod::Probability,
            probability: 0.0,
            whitelist: Vec::new(),
            group_only: true,
            min_messages_in_window: None,
            window_seconds: None,
            min_seconds_since_last_reply: None,
            reply_on_at_or_wake: false,
        }
    }

    pub fn probability(probability: f32) -> Self {
        Self {
            enabled: true,
            probability,
            ..Self::disabled()
        }
    }

    pub fn with_whitelist(
        mut self,
        whitelist: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.whitelist = whitelist
            .into_iter()
            .map(Into::into)
            .filter(|item| !item.trim().is_empty())
            .collect();
        self
    }

    pub fn allow_direct_messages(mut self) -> Self {
        self.group_only = false;
        self
    }

    pub fn with_density_window(mut self, min_messages: usize, window_seconds: u64) -> Self {
        self.min_messages_in_window = Some(min_messages.max(1));
        self.window_seconds = Some(window_seconds.max(1));
        self
    }

    pub fn with_min_seconds_since_last_reply(mut self, seconds: u64) -> Self {
        self.min_seconds_since_last_reply = Some(seconds);
        self
    }

    pub fn reply_on_at_or_wake(mut self) -> Self {
        self.reply_on_at_or_wake = true;
        self
    }

    pub fn should_reply(&self, check: &ActiveReplyCheck) -> bool {
        if !self.enabled {
            return false;
        }
        if self.group_only && check.session_kind != MessageSessionKind::Group {
            return false;
        }
        if check.is_at_or_wake_command {
            return self.reply_on_at_or_wake;
        }
        if !self.whitelist.is_empty()
            && !self.whitelist.iter().any(|item| {
                item == &check.session.origin() || item == &check.session.conversation_id
            })
        {
            return false;
        }
        if let Some(min_seconds) = self.min_seconds_since_last_reply {
            if check
                .seconds_since_last_reply
                .is_some_and(|seconds| seconds < min_seconds)
            {
                return false;
            }
        }
        if let Some(min_messages) = self.min_messages_in_window {
            if check.recent_message_count < min_messages {
                return false;
            }
            if let Some(window_seconds) = self.window_seconds {
                if check
                    .window_seconds
                    .is_some_and(|seconds| seconds > window_seconds)
                {
                    return false;
                }
            }
        }

        match self.method {
            ActiveReplyMethod::Probability => check.roll < self.probability.clamp(0.0, 1.0),
        }
    }
}

impl Default for ActiveReplyPolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveReplyMethod {
    Probability,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveReplyCheck {
    pub session: MemorySessionKey,
    pub session_kind: MessageSessionKind,
    pub is_at_or_wake_command: bool,
    pub roll: f32,
    pub recent_message_count: usize,
    pub window_seconds: Option<u64>,
    pub seconds_since_last_reply: Option<u64>,
}

#[cfg(test)]
mod tests {
    use astrbot_core::MessageSessionKind;

    use super::{ActiveReplyCheck, ActiveReplyPolicy};
    use crate::MemorySessionKey;

    #[test]
    fn active_reply_policy_checks_group_whitelist_and_probability_without_adapter() {
        let policy = ActiveReplyPolicy::probability(0.25).with_whitelist(["room-1"]);
        let mut check = ActiveReplyCheck {
            session: MemorySessionKey::new("webchat", "room-1"),
            session_kind: MessageSessionKind::Group,
            is_at_or_wake_command: false,
            roll: 0.20,
            recent_message_count: 1,
            window_seconds: None,
            seconds_since_last_reply: None,
        };

        assert!(policy.should_reply(&check));
        check.roll = 0.30;
        assert!(!policy.should_reply(&check));
        check.roll = 0.20;
        check.is_at_or_wake_command = true;
        assert!(!policy.should_reply(&check));
        check.is_at_or_wake_command = false;
        check.session_kind = MessageSessionKind::Direct;
        assert!(!policy.should_reply(&check));
    }

    #[test]
    fn active_reply_policy_checks_density_time_window_and_wake_strategy() {
        let policy = ActiveReplyPolicy::probability(1.0)
            .with_density_window(3, 30)
            .with_min_seconds_since_last_reply(10);
        let mut check = ActiveReplyCheck {
            session: MemorySessionKey::new("webchat", "room-1"),
            session_kind: MessageSessionKind::Group,
            is_at_or_wake_command: false,
            roll: 0.0,
            recent_message_count: 2,
            window_seconds: Some(20),
            seconds_since_last_reply: Some(11),
        };

        assert!(!policy.should_reply(&check));
        check.recent_message_count = 3;
        assert!(policy.should_reply(&check));
        check.window_seconds = Some(31);
        assert!(!policy.should_reply(&check));
        check.window_seconds = Some(20);
        check.seconds_since_last_reply = Some(9);
        assert!(!policy.should_reply(&check));

        let wake_policy = ActiveReplyPolicy::probability(0.0).reply_on_at_or_wake();
        check.is_at_or_wake_command = true;
        assert!(wake_policy.should_reply(&check));
    }
}
