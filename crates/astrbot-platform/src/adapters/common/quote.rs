use astrbot_core::{QuotedMessage, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformQuoteRequest {
    pub platform_type: String,
    pub platform_id: String,
    pub session_id: String,
    pub message_id: String,
    pub embedded_quote: Option<QuotedMessage>,
}

impl PlatformQuoteRequest {
    pub fn new(
        platform_type: impl Into<String>,
        platform_id: impl Into<String>,
        session_id: impl Into<String>,
        message_id: impl Into<String>,
    ) -> Self {
        Self {
            platform_type: platform_type.into(),
            platform_id: platform_id.into(),
            session_id: session_id.into(),
            message_id: message_id.into(),
            embedded_quote: None,
        }
    }

    pub fn with_embedded_quote(mut self, embedded_quote: QuotedMessage) -> Self {
        self.embedded_quote = Some(embedded_quote);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlatformQuoteResolution {
    pub quote: Option<QuotedMessage>,
    pub looked_up_forward_refs: Vec<String>,
    pub warnings: Vec<String>,
}

impl PlatformQuoteResolution {
    pub fn resolved(quote: QuotedMessage) -> Self {
        Self {
            quote: Some(quote),
            looked_up_forward_refs: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_forward_lookup(mut self, forward_id: impl Into<String>) -> Self {
        let forward_id = forward_id.into();
        if !forward_id.trim().is_empty() && !self.looked_up_forward_refs.contains(&forward_id) {
            self.looked_up_forward_refs.push(forward_id);
        }
        self
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        let warning = warning.into();
        if !warning.trim().is_empty() {
            self.warnings.push(warning);
        }
        self
    }
}

#[async_trait]
pub trait PlatformQuoteParser: Send + Sync {
    async fn resolve_quote(&self, request: PlatformQuoteRequest)
    -> Result<PlatformQuoteResolution>;
}

#[derive(Clone, Debug, Default)]
pub struct EmbeddedQuoteParser;

#[async_trait]
impl PlatformQuoteParser for EmbeddedQuoteParser {
    async fn resolve_quote(
        &self,
        request: PlatformQuoteRequest,
    ) -> Result<PlatformQuoteResolution> {
        Ok(request
            .embedded_quote
            .filter(QuotedMessage::has_content)
            .map(PlatformQuoteResolution::resolved)
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use astrbot_core::{ForwardMessageReference, QuotedMessage};

    use super::{EmbeddedQuoteParser, PlatformQuoteParser, PlatformQuoteRequest};

    #[tokio::test]
    async fn embedded_quote_parser_returns_normalized_quote_without_pipeline_state() {
        let request = PlatformQuoteRequest::new("onebot", "onebot-1", "group:1", "msg-1")
            .with_embedded_quote(
                QuotedMessage::new()
                    .with_text("quoted")
                    .with_forward_ref(ForwardMessageReference::new("forward-1")),
            );

        let resolution = EmbeddedQuoteParser
            .resolve_quote(request)
            .await
            .expect("quote should resolve");

        let quote = resolution.quote.expect("quote should be present");
        assert_eq!(quote.text.as_deref(), Some("quoted"));
        assert_eq!(quote.forward_refs()[0].forward_id, "forward-1");
    }
}
