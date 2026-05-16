#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundTaskPolicy {
    pub wake_on_complete: bool,
    pub max_seconds: Option<u64>,
    pub note: Option<String>,
}

impl BackgroundTaskPolicy {
    pub fn new() -> Self {
        Self {
            wake_on_complete: true,
            max_seconds: None,
            note: None,
        }
    }

    pub fn detached() -> Self {
        Self {
            wake_on_complete: false,
            max_seconds: None,
            note: None,
        }
    }

    pub fn with_max_seconds(mut self, max_seconds: u64) -> Self {
        self.max_seconds = Some(max_seconds.max(1));
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        let note = note.into();
        self.note = (!note.trim().is_empty()).then_some(note);
        self
    }
}

impl Default for BackgroundTaskPolicy {
    fn default() -> Self {
        Self::new()
    }
}
