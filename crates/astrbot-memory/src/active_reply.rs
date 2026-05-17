use astrbot_core::MessageSessionKind;

use crate::MemorySessionKey;

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveReplyPolicy {
    pub enabled: bool,
    pub method: ActiveReplyMethod,
    pub probability: f32,
    pub whitelist: Vec<String>,
    pub group_only: bool,
}

impl ActiveReplyPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            method: ActiveReplyMethod::Probability,
            probability: 0.0,
            whitelist: Vec::new(),
            group_only: true,
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

    pub fn should_reply(&self, check: &ActiveReplyCheck) -> bool {
        if !self.enabled {
            return false;
        }
        if self.group_only && check.session_kind != MessageSessionKind::Group {
            return false;
        }
        if check.is_at_or_wake_command {
            return false;
        }
        if !self.whitelist.is_empty()
            && !self.whitelist.iter().any(|item| {
                item == &check.session.origin() || item == &check.session.conversation_id
            })
        {
            return false;
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
}
