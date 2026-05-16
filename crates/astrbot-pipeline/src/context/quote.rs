use astrbot_core::{MessageComponent, MessageEvent, ProviderContentPart, Result};
use async_trait::async_trait;

#[async_trait]
pub trait QuoteContextPolicy: Send + Sync {
    async fn quote_content_parts(&self, event: &MessageEvent) -> Result<Vec<ProviderContentPart>>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectedTextQuoteContextPolicy {
    include_sender_id: bool,
}

impl SelectedTextQuoteContextPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn include_sender_id(mut self, include_sender_id: bool) -> Self {
        self.include_sender_id = include_sender_id;
        self
    }

    fn quote_text(&self, component: &MessageComponent) -> Option<String> {
        let MessageComponent::Reply {
            selected_text,
            sender_id,
            ..
        } = component
        else {
            return None;
        };

        let selected_text = selected_text.trim();
        if selected_text.is_empty() {
            return None;
        }

        let sender_prefix = if self.include_sender_id {
            sender_id
                .as_deref()
                .map(str::trim)
                .filter(|sender_id| !sender_id.is_empty())
                .map(|sender_id| format!("({sender_id}): "))
                .unwrap_or_default()
        } else {
            String::new()
        };

        Some(format!(
            "<Quoted Message>\n{sender_prefix}{selected_text}\n</Quoted Message>"
        ))
    }
}

#[async_trait]
impl QuoteContextPolicy for SelectedTextQuoteContextPolicy {
    async fn quote_content_parts(&self, event: &MessageEvent) -> Result<Vec<ProviderContentPart>> {
        Ok(event
            .message
            .components()
            .iter()
            .find_map(|component| self.quote_text(component))
            .map(ProviderContentPart::text)
            .into_iter()
            .collect())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoQuoteContextPolicy;

#[async_trait]
impl QuoteContextPolicy for NoQuoteContextPolicy {
    async fn quote_content_parts(&self, _event: &MessageEvent) -> Result<Vec<ProviderContentPart>> {
        Ok(Vec::new())
    }
}
