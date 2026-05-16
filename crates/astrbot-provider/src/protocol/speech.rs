use astrbot_core::{AstrbotError, Result};
use reqwest::multipart;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OpenAiSpeechToTextResponse {
    text: String,
}

pub(crate) fn build_openai_stt_form(audio: Vec<u8>, model: &str) -> Result<multipart::Form> {
    let file = multipart::Part::bytes(audio)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|err| {
            AstrbotError::Provider(format!("failed to build audio multipart field: {err}"))
        })?;

    Ok(multipart::Form::new()
        .text("model", model.to_string())
        .part("file", file))
}

pub(crate) fn parse_openai_stt_text(body: &str) -> Result<String> {
    let payload: OpenAiSpeechToTextResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    if payload.text.trim().is_empty() {
        return Err(AstrbotError::Provider(
            "provider response did not contain transcription text".to_string(),
        ));
    }

    Ok(payload.text)
}

#[cfg(test)]
mod tests {
    use super::parse_openai_stt_text;

    #[test]
    fn openai_stt_response_parser_rejects_empty_text() {
        assert_eq!(
            parse_openai_stt_text(r#"{"text":"hello"}"#).expect("text should parse"),
            "hello"
        );
        assert!(parse_openai_stt_text(r#"{"text":" "}"#).is_err());
    }
}
