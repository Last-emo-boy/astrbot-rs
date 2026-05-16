use astrbot_core::{AstrbotError, Result};
use serde::Serialize;
use serde_json::Value;

use crate::TextToSpeechRequest;
use crate::streaming::sse_data_lines;

const ERROR_TEXT_MAX_CHARS: usize = 4096;

pub(crate) struct MiniMaxTtsRequestOptions<'a> {
    pub model: &'a str,
    pub language_boost: &'a str,
    pub is_timber_weight: bool,
    pub timber_weights: &'a Value,
    pub voice_speed: f32,
    pub voice_volume: f32,
    pub voice_pitch: f32,
    pub voice_id: &'a str,
    pub voice_emotion: Option<&'a str>,
    pub voice_latex_read: bool,
    pub voice_english_normalization: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct MiniMaxTtsRequest {
    model: String,
    text: String,
    stream: bool,
    language_boost: String,
    voice_setting: MiniMaxVoiceSetting,
    audio_setting: MiniMaxAudioSetting,
    #[serde(skip_serializing_if = "Option::is_none")]
    timber_weights: Option<Value>,
}

#[derive(Debug, Serialize)]
struct MiniMaxVoiceSetting {
    speed: f32,
    vol: f32,
    pitch: f32,
    voice_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    emotion: Option<String>,
    latex_read: bool,
    english_normalization: bool,
}

#[derive(Debug, Serialize)]
struct MiniMaxAudioSetting {
    sample_rate: u32,
    bitrate: u32,
    format: &'static str,
}

pub(crate) fn build_minimax_tts_request(
    request: &TextToSpeechRequest,
    options: MiniMaxTtsRequestOptions<'_>,
) -> Result<MiniMaxTtsRequest> {
    if request.text.trim().is_empty() {
        return Err(AstrbotError::Provider(
            "text-to-speech request must contain text".to_string(),
        ));
    }

    Ok(MiniMaxTtsRequest {
        model: options.model.to_string(),
        text: request.text.clone(),
        stream: true,
        language_boost: options.language_boost.to_string(),
        voice_setting: MiniMaxVoiceSetting {
            speed: options.voice_speed,
            vol: options.voice_volume,
            pitch: options.voice_pitch,
            voice_id: if options.is_timber_weight {
                String::new()
            } else {
                options.voice_id.to_string()
            },
            emotion: options.voice_emotion.map(str::to_string),
            latex_read: options.voice_latex_read,
            english_normalization: options.voice_english_normalization,
        },
        audio_setting: MiniMaxAudioSetting {
            sample_rate: 32000,
            bitrate: 128000,
            format: "mp3",
        },
        timber_weights: options
            .is_timber_weight
            .then(|| options.timber_weights.clone()),
    })
}

pub(crate) fn collect_minimax_sse_audio(body: &str) -> Result<Vec<u8>> {
    let mut audio = Vec::new();

    for data in sse_data_lines(body) {
        if data == "[DONE]" {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if value.get("extra_info").is_some() {
            continue;
        }

        let Some(audio_hex) = value
            .get("data")
            .and_then(|data| data.get("audio"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|audio| !audio.is_empty())
        else {
            continue;
        };

        audio.extend(decode_minimax_hex_audio(audio_hex)?);
    }

    if audio.is_empty() {
        return Err(AstrbotError::Provider(
            "MiniMax TTS API returned empty audio data. Please verify the group_id and voice configuration.".to_string(),
        ));
    }

    Ok(audio)
}

pub(crate) fn decode_minimax_hex_audio(input: &str) -> Result<Vec<u8>> {
    let hex = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if hex.len() % 2 != 0 {
        return Err(AstrbotError::Provider(
            "invalid MiniMax TTS audio hex data".to_string(),
        ));
    }

    hex.chunks_exact(2)
        .map(|chunk| {
            let high = hex_value(chunk[0])?;
            let low = hex_value(chunk[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

pub(crate) fn extract_minimax_error_message(body: &str) -> String {
    let fallback = truncate(body.trim());
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return fallback;
    };

    let extracted = value
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string);

    extracted.unwrap_or(fallback)
}

fn hex_value(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(AstrbotError::Provider(
            "invalid MiniMax TTS audio hex data".to_string(),
        )),
    }
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::{collect_minimax_sse_audio, decode_minimax_hex_audio};

    #[test]
    fn collects_minimax_sse_audio_chunks() {
        let body = concat!(
            "data: {\"extra_info\":{}}\n\n",
            "data: {\"data\":{\"audio\":\"68656c\"}}\n\n",
            "data: {\"data\":{\"audio\":\"6c6f\"}}\n\n"
        );

        let audio = collect_minimax_sse_audio(body).expect("audio should parse");

        assert_eq!(audio, b"hello");
    }

    #[test]
    fn decodes_hex_audio_with_whitespace() {
        let audio = decode_minimax_hex_audio("68 65\n6c6c 6f").expect("audio should decode");

        assert_eq!(audio, b"hello");
    }
}
