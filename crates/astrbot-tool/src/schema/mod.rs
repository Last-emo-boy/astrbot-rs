mod anthropic;
mod gemini;
mod openai;

use serde_json::Value;

use crate::ToolDescriptor;

pub use anthropic::AnthropicToolSchemaSerializer;
pub use gemini::GeminiToolSchemaSerializer;
pub use openai::OpenAiToolSchemaSerializer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderToolSchemaFormat {
    OpenAi,
    Anthropic,
    Gemini,
}

pub trait ToolSchemaSerializer {
    fn serialize_tools(&self, tools: &[ToolDescriptor]) -> Value;
}

pub fn serializer_for(format: ProviderToolSchemaFormat) -> Box<dyn ToolSchemaSerializer> {
    match format {
        ProviderToolSchemaFormat::OpenAi => Box::new(OpenAiToolSchemaSerializer::default()),
        ProviderToolSchemaFormat::Anthropic => Box::new(AnthropicToolSchemaSerializer),
        ProviderToolSchemaFormat::Gemini => Box::new(GeminiToolSchemaSerializer),
    }
}
