use ::regex::Regex;
use astrbot_core::MessageEvent;

use super::EventFilter;

#[derive(Clone, Debug)]
pub struct RegexFilter {
    regex: Regex,
}

impl RegexFilter {
    pub fn new(pattern: &str) -> std::result::Result<Self, ::regex::Error> {
        Ok(Self {
            regex: Regex::new(pattern)?,
        })
    }

    pub fn pattern(&self) -> &str {
        self.regex.as_str()
    }
}

impl EventFilter for RegexFilter {
    fn matches(&self, event: &MessageEvent) -> bool {
        self.regex.is_match(&event.message.plain_text())
    }
}
