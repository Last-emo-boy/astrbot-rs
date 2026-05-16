use astrbot_core::MessageEvent;

use super::EventFilter;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlatformFilter {
    platform_ids: Vec<String>,
    platform_names: Vec<String>,
}

impl PlatformFilter {
    pub fn new(platform_id: impl Into<String>) -> Self {
        Self::default().with_platform_id(platform_id)
    }

    pub fn with_platform_id(mut self, platform_id: impl Into<String>) -> Self {
        push_unique_normalized(&mut self.platform_ids, platform_id);
        self
    }

    pub fn with_platform_name(mut self, platform_name: impl Into<String>) -> Self {
        push_unique_normalized(&mut self.platform_names, platform_name);
        self
    }
}

impl EventFilter for PlatformFilter {
    fn matches(&self, event: &MessageEvent) -> bool {
        self.platform_ids
            .iter()
            .any(|known| known == &event.platform_id)
            || self
                .platform_names
                .iter()
                .any(|known| known == &event.platform_name)
    }
}

fn push_unique_normalized(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into().trim().to_string();
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}
