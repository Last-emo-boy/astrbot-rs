use astrbot_core::Result;
use astrbot_media::MediaInput;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentDescriptor {
    pub source_url: String,
    pub attachment_id: Option<String>,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

impl AttachmentDescriptor {
    pub fn new(source_url: impl Into<String>) -> Self {
        Self {
            source_url: source_url.into(),
            attachment_id: None,
            filename: None,
            content_type: None,
        }
    }

    pub fn attachment_id(attachment_id: impl Into<String>) -> Self {
        Self {
            source_url: String::new(),
            attachment_id: Some(attachment_id.into()),
            filename: None,
            content_type: None,
        }
    }

    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn to_media_input(&self) -> MediaInput {
        let mut media = if let Some(attachment_id) = &self.attachment_id {
            MediaInput::attachment(attachment_id.clone())
        } else if self.source_url.starts_with("data:") {
            MediaInput::data_url(self.source_url.clone())
        } else {
            MediaInput::url(self.source_url.clone())
        };
        if let Some(filename) = &self.filename {
            media = media.with_filename(filename.clone());
        }
        if let Some(content_type) = &self.content_type {
            media = media.with_content_type(content_type.clone());
        }
        media
    }
}

pub trait AttachmentService: Send + Sync {
    fn resolve(&self, attachment: &AttachmentDescriptor) -> Result<String>;
    fn resolve_media_input(&self, attachment: &AttachmentDescriptor) -> Result<MediaInput> {
        Ok(attachment.to_media_input())
    }
}

#[derive(Clone, Debug, Default)]
pub struct PassthroughAttachmentService;

impl AttachmentService for PassthroughAttachmentService {
    fn resolve(&self, attachment: &AttachmentDescriptor) -> Result<String> {
        Ok(attachment.source_url.clone())
    }
}

#[cfg(test)]
mod tests {
    use astrbot_media::MediaInputSource;

    use super::AttachmentDescriptor;

    #[test]
    fn attachment_descriptor_exports_media_input_for_shared_resolution() {
        let descriptor = AttachmentDescriptor::attachment_id("att-1")
            .with_filename("image.png")
            .with_content_type("image/png");

        let media = descriptor.to_media_input();

        assert_eq!(
            media.source,
            MediaInputSource::Attachment {
                id: "att-1".to_string()
            }
        );
        assert_eq!(media.filename.as_deref(), Some("image.png"));
        assert_eq!(media.content_type.as_deref(), Some("image/png"));
    }

    #[test]
    fn data_url_attachment_descriptor_stays_provider_neutral() {
        let descriptor = AttachmentDescriptor::new("data:image/png;base64,iVBORw0KGgo=");

        let media = descriptor.to_media_input();

        assert_eq!(
            media.source,
            MediaInputSource::DataUrl("data:image/png;base64,iVBORw0KGgo=".to_string())
        );
    }
}
