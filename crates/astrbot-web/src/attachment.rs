use astrbot_core::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentDescriptor {
    pub source_url: String,
}

impl AttachmentDescriptor {
    pub fn new(source_url: impl Into<String>) -> Self {
        Self {
            source_url: source_url.into(),
        }
    }
}

pub trait AttachmentService: Send + Sync {
    fn resolve(&self, attachment: &AttachmentDescriptor) -> Result<String>;
}

#[derive(Clone, Debug, Default)]
pub struct PassthroughAttachmentService;

impl AttachmentService for PassthroughAttachmentService {
    fn resolve(&self, attachment: &AttachmentDescriptor) -> Result<String> {
        Ok(attachment.source_url.clone())
    }
}
