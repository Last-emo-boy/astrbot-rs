use astrbot_core::{MessageEvent, ProviderContentPart, ProviderRequest, Result};
use async_trait::async_trait;

use crate::ProviderRequestDecorator;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderModalitySupport {
    pub image: bool,
    pub tool_use: bool,
}

impl ProviderModalitySupport {
    pub fn all() -> Self {
        Self {
            image: true,
            tool_use: true,
        }
    }

    pub fn chat_only() -> Self {
        Self {
            image: false,
            tool_use: false,
        }
    }

    pub fn without_image(mut self) -> Self {
        self.image = false;
        self
    }

    pub fn without_tool_use(mut self) -> Self {
        self.tool_use = false;
        self
    }
}

impl Default for ProviderModalitySupport {
    fn default() -> Self {
        Self::all()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModalityFallbackPolicy {
    pub image_placeholder: String,
    pub sanitize_contexts: bool,
}

impl ModalityFallbackPolicy {
    pub fn new(image_placeholder: impl Into<String>) -> Self {
        Self {
            image_placeholder: image_placeholder.into(),
            sanitize_contexts: true,
        }
    }

    pub fn without_context_sanitization(mut self) -> Self {
        self.sanitize_contexts = false;
        self
    }
}

impl Default for ModalityFallbackPolicy {
    fn default() -> Self {
        Self::new("[image]")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModalityFilterOutcome {
    pub replaced_request_images: usize,
    pub removed_extra_images: usize,
    pub removed_context_images: usize,
    pub removed_tool_placeholders: usize,
    pub removed_tool_results: usize,
}

pub struct ModalityFilterRequestDecorator {
    support: ProviderModalitySupport,
    policy: ModalityFallbackPolicy,
}

impl ModalityFilterRequestDecorator {
    pub fn new(support: ProviderModalitySupport) -> Self {
        Self {
            support,
            policy: ModalityFallbackPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: ModalityFallbackPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn filter_request(&self, request: &mut ProviderRequest) -> ModalityFilterOutcome {
        let mut outcome = ModalityFilterOutcome::default();

        if !self.support.image {
            outcome.replaced_request_images = request
                .image_urls
                .iter()
                .filter(|url| !url.trim().is_empty())
                .count();
            if outcome.replaced_request_images > 0 {
                prepend_image_placeholders(
                    request,
                    &self.policy.image_placeholder,
                    outcome.replaced_request_images,
                );
                request.image_urls.clear();
            }

            outcome.removed_extra_images =
                remove_image_parts(&mut request.extra_user_content_parts);
            if self.policy.sanitize_contexts {
                outcome.removed_context_images = request
                    .contexts
                    .iter_mut()
                    .map(|message| remove_image_parts(&mut message.parts))
                    .sum();
                request.contexts.retain(|message| !message.parts.is_empty());
            }
        }

        if !self.support.tool_use {
            outcome.removed_tool_placeholders = request.tool_placeholders.len();
            outcome.removed_tool_results = request.tool_call_results.len();
            request.tool_placeholders.clear();
            request.tool_call_results.clear();
        }

        outcome
    }
}

#[async_trait]
impl ProviderRequestDecorator for ModalityFilterRequestDecorator {
    async fn decorate(&self, _event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        self.filter_request(request);
        Ok(())
    }
}

fn prepend_image_placeholders(
    request: &mut ProviderRequest,
    placeholder: &str,
    image_count: usize,
) {
    let placeholder = placeholder.trim();
    if placeholder.is_empty() || image_count == 0 {
        return;
    }

    let placeholders = std::iter::repeat_n(placeholder, image_count)
        .collect::<Vec<_>>()
        .join(" ");
    request.prompt = match request.prompt.take() {
        Some(prompt) if !prompt.trim().is_empty() => Some(format!("{placeholders} {prompt}")),
        _ => Some(placeholders),
    };
}

fn remove_image_parts(parts: &mut Vec<ProviderContentPart>) -> usize {
    let before = parts.len();
    parts.retain(|part| !matches!(part, ProviderContentPart::ImageUrl { .. }));
    before - parts.len()
}

#[cfg(test)]
mod tests {
    use astrbot_core::{ProviderContextMessage, ProviderRequest, ProviderToolPlaceholder};

    use super::{ModalityFallbackPolicy, ModalityFilterRequestDecorator, ProviderModalitySupport};

    #[test]
    fn replaces_unsupported_request_images_with_prompt_placeholders() {
        let filter =
            ModalityFilterRequestDecorator::new(ProviderModalitySupport::all().without_image())
                .with_policy(ModalityFallbackPolicy::new("[img]"));
        let mut request = ProviderRequest::new("describe this", "session-1")
            .with_image_url("file-a.png")
            .with_image_url("file-b.png");

        let outcome = filter.filter_request(&mut request);

        assert_eq!(outcome.replaced_request_images, 2);
        assert_eq!(request.prompt.as_deref(), Some("[img] [img] describe this"));
        assert!(request.image_urls.is_empty());
    }

    #[test]
    fn removes_unsupported_image_parts_from_extra_parts_and_contexts() {
        let filter =
            ModalityFilterRequestDecorator::new(ProviderModalitySupport::all().without_image());
        let mut request = ProviderRequest::new("hello", "session-1")
            .with_extra_user_content_part(astrbot_core::ProviderContentPart::image_url("a.png"))
            .with_extra_user_content_part(astrbot_core::ProviderContentPart::text("keep"));
        request.contexts = vec![ProviderContextMessage::new(
            "user",
            vec![
                astrbot_core::ProviderContentPart::text("old"),
                astrbot_core::ProviderContentPart::image_url("old.png"),
            ],
        )];

        let outcome = filter.filter_request(&mut request);

        assert_eq!(outcome.removed_extra_images, 1);
        assert_eq!(outcome.removed_context_images, 1);
        assert_eq!(
            request.extra_user_content_parts,
            vec![astrbot_core::ProviderContentPart::text("keep")]
        );
        assert_eq!(
            request.contexts[0].parts,
            vec![astrbot_core::ProviderContentPart::text("old")]
        );
    }

    #[test]
    fn clears_unsupported_tool_placeholders_and_results() {
        let filter =
            ModalityFilterRequestDecorator::new(ProviderModalitySupport::all().without_tool_use());
        let mut request = ProviderRequest::new("hello", "session-1")
            .with_tool_placeholder(ProviderToolPlaceholder::new("search"))
            .with_tool_call_result(astrbot_core::ProviderToolCallResult::new(
                "call-1", "search", "result",
            ));

        let outcome = filter.filter_request(&mut request);

        assert_eq!(outcome.removed_tool_placeholders, 1);
        assert_eq!(outcome.removed_tool_results, 1);
        assert!(request.tool_placeholders.is_empty());
        assert!(request.tool_call_results.is_empty());
    }
}
