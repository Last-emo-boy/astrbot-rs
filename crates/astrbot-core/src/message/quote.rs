#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuotedMessage {
    pub message_id: Option<String>,
    pub sender_id: Option<String>,
    pub sender_name: Option<String>,
    pub text: Option<String>,
    image_refs: Vec<QuotedImageReference>,
    forward_refs: Vec<ForwardMessageReference>,
}

impl QuotedMessage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = non_empty_string(message_id);
        self
    }

    pub fn with_sender_id(mut self, sender_id: impl Into<String>) -> Self {
        self.sender_id = non_empty_string(sender_id);
        self
    }

    pub fn with_sender_name(mut self, sender_name: impl Into<String>) -> Self {
        self.sender_name = non_empty_string(sender_name);
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = non_empty_string(text);
        self
    }

    pub fn push_image_ref(&mut self, image_ref: QuotedImageReference) {
        if !image_ref.is_empty() && !self.image_refs.contains(&image_ref) {
            self.image_refs.push(image_ref);
        }
    }

    pub fn with_image_ref(mut self, image_ref: QuotedImageReference) -> Self {
        self.push_image_ref(image_ref);
        self
    }

    pub fn push_forward_ref(&mut self, forward_ref: ForwardMessageReference) {
        if !forward_ref.forward_id.trim().is_empty() && !self.forward_refs.contains(&forward_ref) {
            self.forward_refs.push(forward_ref);
        }
    }

    pub fn with_forward_ref(mut self, forward_ref: ForwardMessageReference) -> Self {
        self.push_forward_ref(forward_ref);
        self
    }

    pub fn image_refs(&self) -> &[QuotedImageReference] {
        &self.image_refs
    }

    pub fn forward_refs(&self) -> &[ForwardMessageReference] {
        &self.forward_refs
    }

    pub fn image_ref_values(&self) -> Vec<String> {
        self.image_refs
            .iter()
            .map(|image_ref| image_ref.value.clone())
            .collect()
    }

    pub fn has_content(&self) -> bool {
        self.text.is_some() || !self.image_refs.is_empty() || !self.forward_refs.is_empty()
    }

    pub fn merge(mut self, other: QuotedMessage) -> Self {
        if self.message_id.is_none() {
            self.message_id = other.message_id;
        }
        if self.sender_id.is_none() {
            self.sender_id = other.sender_id;
        }
        if self.sender_name.is_none() {
            self.sender_name = other.sender_name;
        }
        self.text = merge_text(self.text, other.text);
        for image_ref in other.image_refs {
            self.push_image_ref(image_ref);
        }
        for forward_ref in other.forward_refs {
            self.push_forward_ref(forward_ref);
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotedImageReferenceKind {
    Url,
    File,
    Path,
    MediaId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuotedImageReference {
    pub kind: QuotedImageReferenceKind,
    pub value: String,
}

impl QuotedImageReference {
    pub fn new(kind: QuotedImageReferenceKind, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }

    pub fn url(value: impl Into<String>) -> Self {
        Self::new(QuotedImageReferenceKind::Url, value)
    }

    pub fn file(value: impl Into<String>) -> Self {
        Self::new(QuotedImageReferenceKind::File, value)
    }

    pub fn path(value: impl Into<String>) -> Self {
        Self::new(QuotedImageReferenceKind::Path, value)
    }

    pub fn media_id(value: impl Into<String>) -> Self {
        Self::new(QuotedImageReferenceKind::MediaId, value)
    }

    pub fn is_empty(&self) -> bool {
        self.value.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardMessageReference {
    pub forward_id: String,
    pub preview_text: Option<String>,
}

impl ForwardMessageReference {
    pub fn new(forward_id: impl Into<String>) -> Self {
        Self {
            forward_id: forward_id.into(),
            preview_text: None,
        }
    }

    pub fn with_preview_text(mut self, preview_text: impl Into<String>) -> Self {
        self.preview_text = non_empty_string(preview_text);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardMessageNode {
    pub sender_id: Option<String>,
    pub sender_name: Option<String>,
    pub quote: QuotedMessage,
}

impl ForwardMessageNode {
    pub fn new(quote: QuotedMessage) -> Self {
        Self {
            sender_id: None,
            sender_name: None,
            quote,
        }
    }

    pub fn with_sender_id(mut self, sender_id: impl Into<String>) -> Self {
        self.sender_id = non_empty_string(sender_id);
        self
    }

    pub fn with_sender_name(mut self, sender_name: impl Into<String>) -> Self {
        self.sender_name = non_empty_string(sender_name);
        self
    }
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn merge_text(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) if !right.trim().is_empty() => Some(format!("{left}\n{right}")),
        (Some(left), _) => Some(left),
        (None, right) => right,
    }
}
