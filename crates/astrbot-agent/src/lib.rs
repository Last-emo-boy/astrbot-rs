mod context;
mod fallback;
mod multimodal;
mod persona;
mod request_decorator;
mod runner;
mod tool_loop;

pub use context::{
    AgentContextCompressor, AgentContextWindow, AgentTokenCounter, ApproximateTokenCounter,
    ContextTokenBudget, ContextTruncationPolicy, ContextWindowManager,
    ContextWindowRequestDecorator, NoopContextCompressor,
};
pub use fallback::AgentFallbackPolicy;
pub use multimodal::{
    ChatProviderImageCaptioner, ImageCaptionConfig, ImageCaptionRequest,
    ImageCaptionRequestDecorator, ImageCaptioner, ModalityFallbackPolicy, ModalityFilterOutcome,
    ModalityFilterRequestDecorator, ProviderModalitySupport, QuotedImageAttachmentPolicy,
    QuotedImageAttachmentResult,
};
pub use persona::{AgentPersona, PersonaPromptDecorator};
pub use request_decorator::{
    AgentProviderPreferencePort, AgentQuoteContextPort, AgentSessionContextPort,
    CompositeProviderRequestDecorator, NoopProviderRequestDecorator,
    ProviderPreferenceRequestDecorator, ProviderRequestDecorator, ProviderRequestEnvelope,
    QuoteContextRequestDecorator, SessionContextRequestDecorator,
};
pub use runner::{AgentRunOutcome, AgentRunner, ChatAgentRunner};
pub use tool_loop::{
    ToolLoopOutcome, ToolLoopPolicy, ToolLoopState, ToolLoopStep, ToolLoopStrategy,
};

#[cfg(test)]
mod tests;
