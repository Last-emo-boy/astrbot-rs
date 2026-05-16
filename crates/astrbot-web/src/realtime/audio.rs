use std::fmt;
use std::path::{Path, PathBuf};

pub const DEFAULT_LIVE_AUDIO_FORMAT: LiveAudioFormat = LiveAudioFormat {
    sample_rate: 16_000,
    channels: 1,
    bits_per_sample: 16,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveAudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

impl LiveAudioFormat {
    pub fn new(sample_rate: u32, channels: u16, bits_per_sample: u16) -> Self {
        Self {
            sample_rate,
            channels,
            bits_per_sample,
        }
    }

    pub fn pcm_16k_mono() -> Self {
        DEFAULT_LIVE_AUDIO_FORMAT
    }

    pub fn block_align(&self) -> u16 {
        self.channels * (self.bits_per_sample / 8)
    }

    pub fn byte_rate(&self) -> u32 {
        self.sample_rate * u32::from(self.block_align())
    }
}

impl Default for LiveAudioFormat {
    fn default() -> Self {
        Self::pcm_16k_mono()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveAudioWavFile {
    pub path: PathBuf,
    pub format: LiveAudioFormat,
    pub pcm_bytes: usize,
    pub wav_bytes: usize,
}

impl LiveAudioWavFile {
    pub fn cleanup(self) -> std::io::Result<()> {
        if self.path.exists() {
            std::fs::remove_file(self.path)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveAudioError {
    MissingStamp,
    NotSpeaking,
    StampMismatch { expected: String, actual: String },
    EmptyAudio,
    LengthOverflow,
    Io(String),
}

impl fmt::Display for LiveAudioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStamp => formatter.write_str("live audio stamp is required"),
            Self::NotSpeaking => formatter.write_str("live audio session is not speaking"),
            Self::StampMismatch { expected, actual } => {
                write!(
                    formatter,
                    "live audio stamp mismatch: expected {expected}, got {actual}"
                )
            }
            Self::EmptyAudio => formatter.write_str("live audio buffer is empty"),
            Self::LengthOverflow => formatter.write_str("live audio buffer is too large"),
            Self::Io(message) => write!(formatter, "live audio io error: {message}"),
        }
    }
}

impl std::error::Error for LiveAudioError {}

impl From<std::io::Error> for LiveAudioError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveAudioBuffer {
    format: LiveAudioFormat,
    current_stamp: Option<String>,
    pcm_frames: Vec<u8>,
}

impl LiveAudioBuffer {
    pub fn new() -> Self {
        Self::with_format(LiveAudioFormat::default())
    }

    pub fn with_format(format: LiveAudioFormat) -> Self {
        Self {
            format,
            current_stamp: None,
            pcm_frames: Vec::new(),
        }
    }

    pub fn format(&self) -> LiveAudioFormat {
        self.format
    }

    pub fn is_speaking(&self) -> bool {
        self.current_stamp.is_some()
    }

    pub fn current_stamp(&self) -> Option<&str> {
        self.current_stamp.as_deref()
    }

    pub fn pcm_len(&self) -> usize {
        self.pcm_frames.len()
    }

    pub fn start_speaking(&mut self, stamp: impl Into<String>) -> Result<(), LiveAudioError> {
        let stamp = normalize_stamp(stamp)?;
        self.current_stamp = Some(stamp);
        self.pcm_frames.clear();
        Ok(())
    }

    pub fn push_frame(
        &mut self,
        stamp: impl AsRef<str>,
        frame: impl AsRef<[u8]>,
    ) -> Result<usize, LiveAudioError> {
        self.ensure_stamp(stamp.as_ref())?;
        self.pcm_frames.extend_from_slice(frame.as_ref());
        Ok(self.pcm_frames.len())
    }

    pub fn finish_wav_bytes(&mut self, stamp: impl AsRef<str>) -> Result<Vec<u8>, LiveAudioError> {
        let bytes = self.wav_bytes_for_stamp(stamp.as_ref())?;
        self.reset();
        Ok(bytes)
    }

    pub fn finish_wav_to_path(
        &mut self,
        stamp: impl AsRef<str>,
        path: impl Into<PathBuf>,
    ) -> Result<LiveAudioWavFile, LiveAudioError> {
        let bytes = self.wav_bytes_for_stamp(stamp.as_ref())?;
        let path = path.into();
        ensure_parent_dir(&path)?;
        std::fs::write(&path, &bytes)?;
        let file = LiveAudioWavFile {
            path,
            format: self.format,
            pcm_bytes: self.pcm_frames.len(),
            wav_bytes: bytes.len(),
        };
        self.reset();
        Ok(file)
    }

    pub fn reset(&mut self) {
        self.current_stamp = None;
        self.pcm_frames.clear();
    }

    fn ensure_stamp(&self, stamp: &str) -> Result<(), LiveAudioError> {
        let stamp = normalize_stamp(stamp)?;
        let Some(current_stamp) = &self.current_stamp else {
            return Err(LiveAudioError::NotSpeaking);
        };
        if current_stamp != &stamp {
            return Err(LiveAudioError::StampMismatch {
                expected: current_stamp.clone(),
                actual: stamp,
            });
        }
        Ok(())
    }

    fn wav_bytes_for_stamp(&self, stamp: &str) -> Result<Vec<u8>, LiveAudioError> {
        self.ensure_stamp(stamp)?;
        if self.pcm_frames.is_empty() {
            return Err(LiveAudioError::EmptyAudio);
        }
        build_wav_bytes(self.format, &self.pcm_frames)
    }
}

impl Default for LiveAudioBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn build_wav_bytes(format: LiveAudioFormat, pcm_frames: &[u8]) -> Result<Vec<u8>, LiveAudioError> {
    let data_len = u32::try_from(pcm_frames.len()).map_err(|_| LiveAudioError::LengthOverflow)?;
    let chunk_size = 36u32
        .checked_add(data_len)
        .ok_or(LiveAudioError::LengthOverflow)?;
    let mut wav = Vec::with_capacity(44 + pcm_frames.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&chunk_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&format.channels.to_le_bytes());
    wav.extend_from_slice(&format.sample_rate.to_le_bytes());
    wav.extend_from_slice(&format.byte_rate().to_le_bytes());
    wav.extend_from_slice(&format.block_align().to_le_bytes());
    wav.extend_from_slice(&format.bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm_frames);
    Ok(wav)
}

fn ensure_parent_dir(path: &Path) -> Result<(), LiveAudioError> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)?;
    Ok(())
}

fn normalize_stamp(stamp: impl Into<String>) -> Result<String, LiveAudioError> {
    let stamp = stamp.into();
    let trimmed = stamp.trim();
    if trimmed.is_empty() {
        return Err(LiveAudioError::MissingStamp);
    }
    Ok(trimmed.to_string())
}
