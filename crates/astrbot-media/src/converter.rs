//! Audio/video format converter abstraction.
//!
//! Plugins that bridge messaging platforms occasionally need to translate
//! between codecs the platforms accept: QQ's SILK encoding, WeChat's AMR,
//! Telegram's OPUS, etc. Performing those conversions requires bundling a
//! real codec (`ffmpeg`, `libamr`, custom SILK encoder), which is out of
//! scope for the host runtime.
//!
//! Instead, the host exposes a [`MediaConverter`] trait. The
//! [`PassthroughMediaConverter`] is the default implementation: it returns
//! the input bytes unchanged, accepting only formats the destination
//! already understands. Integrators wire in a process-out converter (e.g.
//! ffmpeg via [`astrbot_computer::SubprocessSpec`]) for richer support.

use astrbot_core::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Codec families we know how to reason about. Adding a variant is a
/// breaking change to integrators that match exhaustively.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioCodec {
    Silk,
    Amr,
    Mp3,
    Opus,
    Wav,
    Aac,
    Other,
}

impl AudioCodec {
    pub fn as_str(self) -> &'static str {
        match self {
            AudioCodec::Silk => "silk",
            AudioCodec::Amr => "amr",
            AudioCodec::Mp3 => "mp3",
            AudioCodec::Opus => "opus",
            AudioCodec::Wav => "wav",
            AudioCodec::Aac => "aac",
            AudioCodec::Other => "other",
        }
    }

    pub fn from_extension(extension: &str) -> Self {
        match extension.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
            "silk" => AudioCodec::Silk,
            "amr" => AudioCodec::Amr,
            "mp3" => AudioCodec::Mp3,
            "opus" | "ogg" => AudioCodec::Opus,
            "wav" => AudioCodec::Wav,
            "aac" | "m4a" => AudioCodec::Aac,
            _ => AudioCodec::Other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaConvertRequest {
    pub source: AudioCodec,
    pub target: AudioCodec,
    pub bytes: Vec<u8>,
}

impl MediaConvertRequest {
    pub fn new(source: AudioCodec, target: AudioCodec, bytes: Vec<u8>) -> Self {
        Self {
            source,
            target,
            bytes,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaConvertResponse {
    pub codec: AudioCodec,
    pub bytes: Vec<u8>,
}

#[async_trait]
pub trait MediaConverter: Send + Sync {
    async fn convert(&self, request: MediaConvertRequest) -> Result<MediaConvertResponse>;
}

/// Default converter: returns the input untouched when the source codec is
/// already what the caller wants, errors otherwise. Useful as a placeholder
/// until a real ffmpeg-backed converter is wired in.
#[derive(Clone, Debug, Default)]
pub struct PassthroughMediaConverter;

#[async_trait]
impl MediaConverter for PassthroughMediaConverter {
    async fn convert(&self, request: MediaConvertRequest) -> Result<MediaConvertResponse> {
        if request.source == request.target {
            return Ok(MediaConvertResponse {
                codec: request.target,
                bytes: request.bytes,
            });
        }
        Err(astrbot_core::AstrbotError::Pipeline(format!(
            "media converter cannot transcode {} -> {} without ffmpeg backend",
            request.source.as_str(),
            request.target.as_str()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_round_trips_via_extension() {
        for codec in [
            AudioCodec::Silk,
            AudioCodec::Amr,
            AudioCodec::Mp3,
            AudioCodec::Opus,
            AudioCodec::Wav,
            AudioCodec::Aac,
        ] {
            let ext = codec.as_str();
            assert_eq!(AudioCodec::from_extension(ext), codec, "{ext}");
        }
    }

    #[test]
    fn from_extension_handles_alternate_spellings() {
        assert_eq!(AudioCodec::from_extension(".ogg"), AudioCodec::Opus);
        assert_eq!(AudioCodec::from_extension("m4a"), AudioCodec::Aac);
        assert_eq!(AudioCodec::from_extension("UNKNOWN"), AudioCodec::Other);
    }

    #[tokio::test]
    async fn passthrough_returns_bytes_unchanged_when_same_codec() {
        let converter = PassthroughMediaConverter;
        let request = MediaConvertRequest::new(
            AudioCodec::Mp3,
            AudioCodec::Mp3,
            vec![0x49, 0x44, 0x33],
        );
        let response = converter.convert(request).await.unwrap();
        assert_eq!(response.codec, AudioCodec::Mp3);
        assert_eq!(response.bytes, vec![0x49, 0x44, 0x33]);
    }

    #[tokio::test]
    async fn passthrough_errors_on_cross_codec_conversion() {
        let converter = PassthroughMediaConverter;
        let request =
            MediaConvertRequest::new(AudioCodec::Silk, AudioCodec::Mp3, vec![0u8; 16]);
        assert!(converter.convert(request).await.is_err());
    }
}
