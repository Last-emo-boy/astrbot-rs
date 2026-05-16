use std::sync::Arc;

use astrbot_core::{MessageEvent, ProviderContentPart, ProviderRequest, Result};
use astrbot_provider::{ChatProvider, ChatRequest};
use async_trait::async_trait;

use crate::ProviderRequestDecorator;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageCaptionConfig {
    pub prompt: String,
    pub tag_name: String,
    pub clear_images_after_caption: bool,
}

impl ImageCaptionConfig {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            ..Self::default()
        }
    }

    pub fn with_tag_name(mut self, tag_name: impl Into<String>) -> Self {
        self.tag_name = tag_name.into();
        self
    }

    pub fn keep_images_after_caption(mut self) -> Self {
        self.clear_images_after_caption = false;
        self
    }
}

impl Default for ImageCaptionConfig {
    fn default() -> Self {
        Self {
            prompt: "Describe the image content for the chat model.".to_string(),
            tag_name: "image_caption".to_string(),
            clear_images_after_caption: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageCaptionRequest {
    pub prompt: String,
    pub session_id: Option<String>,
    pub image_urls: Vec<String>,
}

#[async_trait]
pub trait ImageCaptioner: Send + Sync {
    async fn caption_images(&self, request: ImageCaptionRequest) -> Result<Option<String>>;
}

pub struct ImageCaptionRequestDecorator {
    captioner: Arc<dyn ImageCaptioner>,
    config: ImageCaptionConfig,
}

impl ImageCaptionRequestDecorator {
    pub fn new(captioner: Arc<dyn ImageCaptioner>) -> Self {
        Self {
            captioner,
            config: ImageCaptionConfig::default(),
        }
    }

    pub fn with_config(mut self, config: ImageCaptionConfig) -> Self {
        self.config = config;
        self
    }

    pub async fn apply_caption(&self, request: &mut ProviderRequest) -> Result<Option<String>> {
        let image_urls = request
            .image_urls
            .iter()
            .filter(|url| !url.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>();
        if image_urls.is_empty() {
            return Ok(None);
        }

        let caption = self
            .captioner
            .caption_images(ImageCaptionRequest {
                prompt: self.config.prompt.clone(),
                session_id: request.session_id.clone(),
                image_urls,
            })
            .await?
            .map(|caption| caption.trim().to_string())
            .filter(|caption| !caption.is_empty());

        if let Some(caption) = caption {
            request
                .extra_user_content_parts
                .push(ProviderContentPart::text(wrap_caption(
                    &self.config.tag_name,
                    &caption,
                )));
            if self.config.clear_images_after_caption {
                request.image_urls.clear();
            }
            return Ok(Some(caption));
        }

        Ok(None)
    }
}

#[async_trait]
impl ProviderRequestDecorator for ImageCaptionRequestDecorator {
    async fn decorate(&self, _event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        self.apply_caption(request).await?;
        Ok(())
    }
}

pub struct ChatProviderImageCaptioner {
    provider: Arc<dyn ChatProvider>,
}

impl ChatProviderImageCaptioner {
    pub fn new(provider: Arc<dyn ChatProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ImageCaptioner for ChatProviderImageCaptioner {
    async fn caption_images(&self, request: ImageCaptionRequest) -> Result<Option<String>> {
        let session_id = request.session_id.unwrap_or_default();
        let response = self
            .provider
            .chat(ChatRequest::new(request.prompt, session_id).with_image_urls(request.image_urls))
            .await?;
        let caption = response.chain.plain_text().trim().to_string();
        Ok((!caption.is_empty()).then_some(caption))
    }
}

fn wrap_caption(tag_name: &str, caption: &str) -> String {
    let tag_name = tag_name.trim();
    if tag_name.is_empty() {
        return caption.to_string();
    }
    format!("<{tag_name}>{caption}</{tag_name}>")
}

#[cfg(test)]
mod tests {
    use astrbot_core::ProviderRequest;
    use async_trait::async_trait;

    use super::{
        ImageCaptionConfig, ImageCaptionRequest, ImageCaptionRequestDecorator, ImageCaptioner,
    };

    struct StaticCaptioner;

    #[async_trait]
    impl ImageCaptioner for StaticCaptioner {
        async fn caption_images(
            &self,
            request: ImageCaptionRequest,
        ) -> astrbot_core::Result<Option<String>> {
            assert_eq!(request.image_urls, vec!["a.png", "b.png"]);
            Ok(Some("two images".to_string()))
        }
    }

    #[tokio::test]
    async fn inserts_caption_part_and_clears_images_by_default() {
        let decorator = ImageCaptionRequestDecorator::new(std::sync::Arc::new(StaticCaptioner))
            .with_config(ImageCaptionConfig::new("caption them"));
        let mut request = ProviderRequest::new("hello", "session-1")
            .with_image_url("a.png")
            .with_image_url("b.png");

        let caption = decorator
            .apply_caption(&mut request)
            .await
            .expect("caption should apply");

        assert_eq!(caption.as_deref(), Some("two images"));
        assert!(request.image_urls.is_empty());
        assert_eq!(
            request.extra_user_content_parts,
            vec![astrbot_core::ProviderContentPart::text(
                "<image_caption>two images</image_caption>"
            )]
        );
    }

    #[tokio::test]
    async fn can_keep_images_after_caption_for_vision_providers() {
        let decorator = ImageCaptionRequestDecorator::new(std::sync::Arc::new(StaticCaptioner))
            .with_config(ImageCaptionConfig::default().keep_images_after_caption());
        let mut request = ProviderRequest::new("hello", "session-1")
            .with_image_url("a.png")
            .with_image_url("b.png");

        decorator
            .apply_caption(&mut request)
            .await
            .expect("caption should apply");

        assert_eq!(request.image_urls, vec!["a.png", "b.png"]);
    }
}
