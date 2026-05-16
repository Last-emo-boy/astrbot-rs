use std::collections::HashSet;

use astrbot_core::{ProviderContentPart, ProviderRequest};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuotedImageAttachmentPolicy {
    pub max_images: usize,
    pub text_prefix: String,
}

impl QuotedImageAttachmentPolicy {
    pub fn new(max_images: usize) -> Self {
        Self {
            max_images,
            ..Self::default()
        }
    }

    pub fn with_text_prefix(mut self, text_prefix: impl Into<String>) -> Self {
        self.text_prefix = text_prefix.into();
        self
    }

    pub fn append_images<I, S>(
        &self,
        request: &mut ProviderRequest,
        image_refs: I,
    ) -> QuotedImageAttachmentResult
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut result = QuotedImageAttachmentResult::default();
        let mut seen = request.image_urls.iter().cloned().collect::<HashSet<_>>();

        for image_ref in image_refs {
            let image_ref = image_ref.into();
            if image_ref.trim().is_empty() {
                continue;
            }
            if !seen.insert(image_ref.clone()) {
                result.skipped_duplicates += 1;
                continue;
            }
            if result.added >= self.max_images {
                result.truncated += 1;
                continue;
            }

            request.image_urls.push(image_ref.clone());
            request
                .extra_user_content_parts
                .push(ProviderContentPart::text(format!(
                    "[{}: path {}]",
                    self.text_prefix, image_ref
                )));
            result.added += 1;
        }

        result
    }
}

impl Default for QuotedImageAttachmentPolicy {
    fn default() -> Self {
        Self {
            max_images: 20,
            text_prefix: "Image Attachment in quoted message".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuotedImageAttachmentResult {
    pub added: usize,
    pub skipped_duplicates: usize,
    pub truncated: usize,
}

#[cfg(test)]
mod tests {
    use astrbot_core::{ProviderContentPart, ProviderRequest};

    use super::QuotedImageAttachmentPolicy;

    #[test]
    fn appends_quoted_image_refs_as_images_and_text_attachments() {
        let policy = QuotedImageAttachmentPolicy::new(2);
        let mut request = ProviderRequest::new("hello", "session-1").with_image_url("existing.png");

        let result = policy.append_images(
            &mut request,
            vec![
                "existing.png",
                "quoted-a.png",
                "quoted-b.png",
                "quoted-c.png",
            ],
        );

        assert_eq!(result.added, 2);
        assert_eq!(result.skipped_duplicates, 1);
        assert_eq!(result.truncated, 1);
        assert_eq!(
            request.image_urls,
            vec!["existing.png", "quoted-a.png", "quoted-b.png"]
        );
        assert_eq!(
            request.extra_user_content_parts,
            vec![
                ProviderContentPart::text(
                    "[Image Attachment in quoted message: path quoted-a.png]"
                ),
                ProviderContentPart::text(
                    "[Image Attachment in quoted message: path quoted-b.png]"
                ),
            ]
        );
    }
}
