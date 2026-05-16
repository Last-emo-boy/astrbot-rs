use astrbot_core::{MessageEvent, MessageSessionKind};

use super::EventFilter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageSessionKindFilter {
    kind: MessageSessionKind,
}

impl MessageSessionKindFilter {
    pub fn direct() -> Self {
        Self {
            kind: MessageSessionKind::Direct,
        }
    }

    pub fn group() -> Self {
        Self {
            kind: MessageSessionKind::Group,
        }
    }

    pub fn new(kind: MessageSessionKind) -> Self {
        Self { kind }
    }
}

impl EventFilter for MessageSessionKindFilter {
    fn matches(&self, event: &MessageEvent) -> bool {
        event.session.kind == self.kind
    }
}
