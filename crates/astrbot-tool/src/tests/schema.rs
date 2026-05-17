use serde_json::json;

use crate::ToolSchemaSerializer;
use crate::schema::{
    AnthropicToolSchemaSerializer, GeminiToolSchemaSerializer, OpenAiToolSchemaSerializer,
};

use super::weather_tool;

#[test]
fn openai_anthropic_and_gemini_serializers_are_provider_specific() {
    let tools = vec![weather_tool()];

    let openai = OpenAiToolSchemaSerializer::default().serialize_tools(&tools);
    let anthropic = AnthropicToolSchemaSerializer.serialize_tools(&tools);
    let gemini = GeminiToolSchemaSerializer.serialize_tools(&tools);

    assert_eq!(openai[0]["type"], "function");
    assert_eq!(openai[0]["function"]["name"], "weather");
    assert_eq!(
        openai[0]["function"]["parameters"]["required"],
        json!(["city"])
    );

    assert_eq!(anthropic[0]["name"], "weather");
    assert_eq!(
        anthropic[0]["input_schema"]["properties"]["city"]["type"],
        "string"
    );

    assert_eq!(gemini["function_declarations"][0]["name"], "weather");
    assert_eq!(
        gemini["function_declarations"][0]["parameters"]["properties"]["city"]["type"],
        "string"
    );
}
