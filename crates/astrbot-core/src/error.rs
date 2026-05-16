use thiserror::Error;

pub type Result<T> = std::result::Result<T, AstrbotError>;

#[derive(Debug, Error)]
pub enum AstrbotError {
    #[error("event channel closed")]
    EventChannelClosed,

    #[error("message is empty")]
    EmptyMessage,

    #[error("missing chat provider")]
    MissingChatProvider,

    #[error("platform error: {0}")]
    Platform(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("pipeline error: {0}")]
    Pipeline(String),
}
