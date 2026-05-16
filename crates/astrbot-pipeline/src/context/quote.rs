use astrbot_core::{
    MessageComponent, MessageEvent, ProviderContentPart, QuotedImageReference, QuotedMessage,
    Result,
};
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

    pub fn quote_content_parts_from_quote(
        &self,
        quote: &QuotedMessage,
    ) -> Vec<ProviderContentPart> {
        self.format_quote(quote)
            .map(ProviderContentPart::text)
            .into_iter()
            .chain(
                quote
                    .image_refs()
                    .iter()
                    .map(|image_ref| ProviderContentPart::text(format_image_ref(image_ref))),
            )
            .collect()
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

    fn format_quote(&self, quote: &QuotedMessage) -> Option<String> {
        let quoted_text = quote.text.as_deref().unwrap_or("[Empty Text]").trim();
        if quoted_text.is_empty()
            && quote.image_refs().is_empty()
            && quote.forward_refs().is_empty()
        {
            return None;
        }

        let sender_prefix = if self.include_sender_id {
            quote
                .sender_name
                .as_deref()
                .or(quote.sender_id.as_deref())
                .map(str::trim)
                .filter(|sender| !sender.is_empty())
                .map(|sender| format!("({sender}): "))
                .unwrap_or_default()
        } else {
            String::new()
        };

        Some(format!(
            "<Quoted Message>\n{sender_prefix}{quoted_text}\n</Quoted Message>"
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

fn format_image_ref(image_ref: &QuotedImageReference) -> String {
    format!(
        "[Image Attachment in quoted message: path {}]",
        image_ref.value
    )
}

#[cfg(test)]
mod tests {
    use astrbot_core::{
        ForwardMessageReference, ProviderContentPart, QuotedImageReference, QuotedMessage,
    };

    use super::SelectedTextQuoteContextPolicy;

    #[test]
    fn selected_text_policy_formats_normalized_quote_data() {
        let quote = QuotedMessage::new()
            .with_sender_name("Alice")
            .with_text("quoted text")
            .with_image_ref(QuotedImageReference::url("quoted.png"))
            .with_forward_ref(ForwardMessageReference::new("forward-1"));
        let policy = SelectedTextQuoteContextPolicy::new().include_sender_id(true);

        let parts = policy.quote_content_parts_from_quote(&quote);

        assert_eq!(
            parts,
            vec![
                ProviderContentPart::text(
                    "<Quoted Message>\n(Alice): quoted text\n</Quoted Message>"
                ),
                ProviderContentPart::text("[Image Attachment in quoted message: path quoted.png]"),
            ]
        );
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
