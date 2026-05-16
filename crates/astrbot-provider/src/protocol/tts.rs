use astrbot_core::{AstrbotError, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::TextToSpeechRequest;

const ERROR_TEXT_MAX_CHARS: usize = 4096;

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiTextToSpeechRequest {
    model: String,
    voice: String,
    input: String,
    response_format: String,
}

pub(crate) fn build_openai_tts_request(
    request: &TextToSpeechRequest,
    model: &str,
    voice: &str,
    response_format: &str,
) -> Result<OpenAiTextToSpeechRequest> {
    ensure_tts_text(request)?;
    Ok(OpenAiTextToSpeechRequest {
        model: model.to_string(),
        voice: voice.to_string(),
        input: request.text.clone(),
        response_format: response_format.to_string(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiTtsRequest {
    contents: Vec<GeminiContent>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    response_modalities: Vec<&'static str>,
    speech_config: GeminiSpeechConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiSpeechConfig {
    voice_config: GeminiVoiceConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiVoiceConfig {
    prebuilt_voice_config: GeminiPrebuiltVoiceConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiPrebuiltVoiceConfig {
    voice_name: String,
}

#[derive(Debug, Deserialize)]
struct GeminiTtsResponse {
    #[serde(default)]
    candidates: Vec<GeminiTtsCandidate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTtsCandidate {
    content: Option<GeminiTtsContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiTtsContent {
    #[serde(default)]
    parts: Vec<GeminiTtsPart>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTtsPart {
    inline_data: Option<GeminiInlineData>,
}

#[derive(Debug, Deserialize)]
struct GeminiInlineData {
    data: String,
}

pub(crate) fn gemini_tts_generate_content_url(api_base: &str, model: &str) -> String {
    let api_base = api_base.trim_end_matches('/');
    let model = model.trim_start_matches("models/");
    if api_base.ends_with("/v1beta") {
        format!("{api_base}/models/{model}:generateContent")
    } else {
        format!("{api_base}/v1beta/models/{model}:generateContent")
    }
}

pub(crate) fn build_gemini_tts_request(
    request: &TextToSpeechRequest,
    voice: &str,
    prompt_prefix: Option<&str>,
) -> Result<GeminiTtsRequest> {
    ensure_tts_text(request)?;
    let prompt = match prompt_prefix {
        Some(prefix) => format!("{prefix}: {}", request.text),
        None => request.text.clone(),
    };

    Ok(GeminiTtsRequest {
        contents: vec![GeminiContent {
            parts: vec![GeminiPart { text: prompt }],
        }],
        generation_config: GeminiGenerationConfig {
            response_modalities: vec!["AUDIO"],
            speech_config: GeminiSpeechConfig {
                voice_config: GeminiVoiceConfig {
                    prebuilt_voice_config: GeminiPrebuiltVoiceConfig {
                        voice_name: voice.to_string(),
                    },
                },
            },
        },
    })
}

pub(crate) fn parse_gemini_tts_audio(body: &str) -> Result<Vec<u8>> {
    let payload: GeminiTtsResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    let inline_data = payload
        .candidates
        .into_iter()
        .find_map(|candidate| {
            candidate
                .content
                .and_then(|content| content.parts.into_iter().find_map(|part| part.inline_data))
        })
        .ok_or_else(|| {
            AstrbotError::Provider("No audio content returned from Gemini TTS API".to_string())
        })?;

    base64::engine::general_purpose::STANDARD
        .decode(inline_data.data.trim())
        .map_err(|err| AstrbotError::Provider(format!("invalid Gemini TTS audio data: {err}")))
}

pub(crate) fn gemini_tts_wav_bytes(pcm_audio: &[u8]) -> Result<Vec<u8>> {
    let data_len = u32::try_from(pcm_audio.len()).map_err(|_| {
        AstrbotError::Provider("Gemini TTS audio is too large to write as WAV".to_string())
    })?;
    let riff_len = 36_u32
        .checked_add(data_len)
        .ok_or_else(|| AstrbotError::Provider("Gemini TTS WAV size overflow".to_string()))?;
    let sample_rate = 24_000_u32;
    let channels = 1_u16;
    let bits_per_sample = 16_u16;
    let block_align = channels * bits_per_sample / 8;
    let byte_rate = sample_rate * u32::from(block_align);

    let mut wav = Vec::with_capacity(44 + pcm_audio.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm_audio);

    Ok(wav)
}

#[derive(Debug, Serialize)]
pub(crate) struct VolcengineTtsRequest {
    app: VolcengineApp,
    user: VolcengineUser,
    audio: VolcengineAudio,
    request: VolcengineRequest,
}

#[derive(Debug, Serialize)]
struct VolcengineApp {
    appid: String,
    token: String,
    cluster: String,
}

#[derive(Debug, Serialize)]
struct VolcengineUser {
    uid: String,
}

#[derive(Debug, Serialize)]
struct VolcengineAudio {
    voice_type: String,
    encoding: &'static str,
    speed_ratio: f32,
    volume_ratio: f32,
    pitch_ratio: f32,
}

#[derive(Debug, Serialize)]
struct VolcengineRequest {
    reqid: String,
    text: String,
    text_type: &'static str,
    operation: &'static str,
    with_frontend: u8,
    frontend_type: &'static str,
}

#[derive(Debug, Deserialize)]
struct VolcengineTtsResponse {
    data: Option<String>,
    message: Option<String>,
}

pub(crate) struct VolcengineTtsRequestOptions<'a> {
    pub appid: &'a str,
    pub token: &'a str,
    pub cluster: &'a str,
    pub voice_type: &'a str,
    pub speed_ratio: f32,
    pub uid: String,
    pub reqid: String,
}

pub(crate) fn build_volcengine_tts_request(
    request: &TextToSpeechRequest,
    options: VolcengineTtsRequestOptions<'_>,
) -> Result<VolcengineTtsRequest> {
    ensure_tts_text(request)?;

    Ok(VolcengineTtsRequest {
        app: VolcengineApp {
            appid: options.appid.to_string(),
            token: options.token.to_string(),
            cluster: options.cluster.to_string(),
        },
        user: VolcengineUser { uid: options.uid },
        audio: VolcengineAudio {
            voice_type: options.voice_type.to_string(),
            encoding: "mp3",
            speed_ratio: options.speed_ratio,
            volume_ratio: 1.0,
            pitch_ratio: 1.0,
        },
        request: VolcengineRequest {
            reqid: options.reqid,
            text: request.text.clone(),
            text_type: "plain",
            operation: "query",
            with_frontend: 1,
            frontend_type: "unitTson",
        },
    })
}

pub(crate) fn parse_volcengine_tts_audio(body: &str) -> Result<Vec<u8>> {
    let payload: VolcengineTtsResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    let data = payload
        .data
        .as_deref()
        .map(str::trim)
        .filter(|data| !data.is_empty())
        .ok_or_else(|| {
            AstrbotError::Provider(format!(
                "Volcengine TTS provider returned no audio data: {}",
                payload
                    .message
                    .unwrap_or_else(|| "missing data".to_string())
            ))
        })?;

    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|err| AstrbotError::Provider(format!("invalid Volcengine TTS audio data: {err}")))
}

pub(crate) fn extract_gemini_tts_error_message(body: &str) -> String {
    extract_message_or_error(body)
}

pub(crate) fn extract_volcengine_tts_error_message(body: &str) -> String {
    extract_message_or_error(body)
}

fn ensure_tts_text(request: &TextToSpeechRequest) -> Result<()> {
    if request.text.trim().is_empty() {
        return Err(AstrbotError::Provider(
            "text-to-speech request must contain text".to_string(),
        ));
    }

    Ok(())
}

fn extract_message_or_error(body: &str) -> String {
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

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;

    use crate::TextToSpeechRequest;

    use super::{
        VolcengineTtsRequestOptions, build_gemini_tts_request, build_openai_tts_request,
        build_volcengine_tts_request, gemini_tts_generate_content_url, gemini_tts_wav_bytes,
        parse_gemini_tts_audio, parse_volcengine_tts_audio,
    };

    #[test]
    fn tts_request_builders_match_provider_wire_shapes() {
        let request = TextToSpeechRequest::new("hello");
        let openai = build_openai_tts_request(&request, "tts-1", "alloy", "wav")
            .expect("OpenAI TTS request should build");
        let gemini = build_gemini_tts_request(&request, "Kore", Some("Say warmly"))
            .expect("Gemini TTS request should build");
        let volcengine = build_volcengine_tts_request(
            &request,
            VolcengineTtsRequestOptions {
                appid: "appid",
                token: "token",
                cluster: "cluster",
                voice_type: "voice",
                speed_ratio: 1.25,
                uid: "uid".to_string(),
                reqid: "reqid".to_string(),
            },
        )
        .expect("Volcengine TTS request should build");

        assert_eq!(
            serde_json::to_value(openai).expect("request should serialize"),
            serde_json::json!({"model":"tts-1","voice":"alloy","input":"hello","response_format":"wav"})
        );
        assert_eq!(
            serde_json::to_value(gemini).expect("request should serialize"),
            serde_json::json!({
                "contents":[{"parts":[{"text":"Say warmly: hello"}]}],
                "generationConfig":{
                    "responseModalities":["AUDIO"],
                    "speechConfig":{"voiceConfig":{"prebuiltVoiceConfig":{"voiceName":"Kore"}}}
                }
            })
        );
        assert_eq!(
            serde_json::to_value(volcengine).expect("request should serialize"),
            serde_json::json!({
                "app":{"appid":"appid","token":"token","cluster":"cluster"},
                "user":{"uid":"uid"},
                "audio":{"voice_type":"voice","encoding":"mp3","speed_ratio":1.25,"volume_ratio":1.0,"pitch_ratio":1.0},
                "request":{"reqid":"reqid","text":"hello","text_type":"plain","operation":"query","with_frontend":1,"frontend_type":"unitTson"}
            })
        );
    }

    #[test]
    fn tts_response_parsers_extract_audio_bytes() {
        let gemini_audio = base64::engine::general_purpose::STANDARD.encode(b"pcm");
        let volcengine_audio = base64::engine::general_purpose::STANDARD.encode(b"mp3");

        assert_eq!(
            parse_gemini_tts_audio(&format!(
                r#"{{"candidates":[{{"content":{{"parts":[{{"inlineData":{{"data":"{gemini_audio}"}}}}]}}}}]}}"#
            ))
            .expect("Gemini TTS response should parse"),
            b"pcm"
        );
        assert_eq!(
            parse_volcengine_tts_audio(&format!(r#"{{"data":"{volcengine_audio}"}}"#))
                .expect("Volcengine TTS response should parse"),
            b"mp3"
        );
    }

    #[test]
    fn gemini_tts_url_and_wav_bytes_keep_protocol_details_outside_adapter() {
        assert_eq!(
            gemini_tts_generate_content_url("https://generativelanguage.googleapis.com", "tts"),
            "https://generativelanguage.googleapis.com/v1beta/models/tts:generateContent"
        );
        let wav = gemini_tts_wav_bytes(b"\x01\x02").expect("wav should build");
        assert!(wav.starts_with(b"RIFF"));
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[wav.len() - 2..], b"\x01\x02");
    }
}
