use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, Result};
use astrbot_media::{
    MediaDownloadPolicy, MediaDownloadRequest, MediaDownloadService, ReqwestMediaDownloadService,
};
use async_trait::async_trait;

static AUDIO_CONVERSION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioFormat {
    Silk,
    Amr,
}

impl AudioFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Silk => "silk",
            Self::Amr => "amr",
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioTranscodeTarget {
    Wav,
    Amr,
    Silk,
}

impl AudioTranscodeTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Amr => "amr",
            Self::Silk => "silk",
        }
    }

    fn extension(self) -> &'static str {
        self.as_str()
    }
}

impl fmt::Display for AudioTranscodeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct AudioConversionRequest {
    pub provider_label: String,
    pub audio_url: String,
    pub format: AudioFormat,
    pub target_format: AudioTranscodeTarget,
    pub audio: Vec<u8>,
}

#[async_trait]
pub trait AudioMediaConverter: Send + Sync {
    async fn convert(&self, request: AudioConversionRequest) -> Result<Vec<u8>>;
}

#[derive(Clone, Debug)]
pub struct AudioConversionCommandRequest {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub target_format: AudioTranscodeTarget,
}

#[async_trait]
pub trait AudioConversionCommand: Send + Sync {
    async fn run(&self, request: AudioConversionCommandRequest) -> Result<()>;
}

#[derive(Clone, Debug, Default)]
pub struct UnsupportedAudioMediaConverter;

#[async_trait]
impl AudioMediaConverter for UnsupportedAudioMediaConverter {
    async fn convert(&self, request: AudioConversionRequest) -> Result<Vec<u8>> {
        Err(AstrbotError::Provider(format!(
            "{} audio requires {} conversion, but no audio media converter is configured for the media conversion boundary",
            request.provider_label, request.format
        )))
    }
}

#[derive(Clone, Debug)]
pub struct FfmpegCommandRunner {
    binary: PathBuf,
}

impl Default for FfmpegCommandRunner {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("ffmpeg"),
        }
    }
}

impl FfmpegCommandRunner {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

#[async_trait]
impl AudioConversionCommand for FfmpegCommandRunner {
    async fn run(&self, request: AudioConversionCommandRequest) -> Result<()> {
        if request.target_format == AudioTranscodeTarget::Silk {
            return Err(AstrbotError::Provider(
                "silk audio output requires a configured silk encoder backend".to_string(),
            ));
        }

        let mut command = Command::new(&self.binary);
        command
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y")
            .arg("-i")
            .arg(&request.input_path);

        match request.target_format {
            AudioTranscodeTarget::Wav => {
                command
                    .arg("-acodec")
                    .arg("pcm_s16le")
                    .arg("-ar")
                    .arg("16000")
                    .arg("-ac")
                    .arg("1");
            }
            AudioTranscodeTarget::Amr => {
                command
                    .arg("-ar")
                    .arg("8000")
                    .arg("-ac")
                    .arg("1")
                    .arg("-ab")
                    .arg("12.2k");
            }
            AudioTranscodeTarget::Silk => unreachable!("silk target handled before command build"),
        }
        command.arg(&request.output_path);

        let output = command.output().map_err(|err| {
            AstrbotError::Provider(format!(
                "failed to execute ffmpeg audio converter {}: {err}",
                self.binary.display()
            ))
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(AstrbotError::Provider(format!(
                "ffmpeg audio conversion to {} failed: {}",
                request.target_format,
                if stderr.is_empty() {
                    output.status.to_string()
                } else {
                    stderr
                }
            )));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct FfmpegAudioMediaConverter {
    temp_dir: PathBuf,
    runner: Arc<dyn AudioConversionCommand>,
    silk_runner: Option<Arc<dyn AudioConversionCommand>>,
}

impl fmt::Debug for FfmpegAudioMediaConverter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FfmpegAudioMediaConverter")
            .field("temp_dir", &self.temp_dir)
            .finish_non_exhaustive()
    }
}

impl FfmpegAudioMediaConverter {
    pub fn new(temp_dir: impl Into<PathBuf>) -> Self {
        Self::with_runner(temp_dir, Arc::new(FfmpegCommandRunner::default()))
    }

    pub fn with_binary(temp_dir: impl Into<PathBuf>, binary: impl Into<PathBuf>) -> Self {
        Self::with_runner(temp_dir, Arc::new(FfmpegCommandRunner::new(binary)))
    }

    pub fn with_runner(
        temp_dir: impl Into<PathBuf>,
        runner: Arc<dyn AudioConversionCommand>,
    ) -> Self {
        Self {
            temp_dir: temp_dir.into(),
            runner,
            silk_runner: None,
        }
    }

    pub fn with_silk_runner(mut self, runner: Arc<dyn AudioConversionCommand>) -> Self {
        self.silk_runner = Some(runner);
        self
    }

    fn runner_for(
        &self,
        target_format: AudioTranscodeTarget,
    ) -> Result<Arc<dyn AudioConversionCommand>> {
        if target_format == AudioTranscodeTarget::Silk {
            return self.silk_runner.clone().ok_or_else(|| {
                AstrbotError::Provider(
                    "silk audio output requires a configured silk encoder backend".to_string(),
                )
            });
        }
        Ok(self.runner.clone())
    }
}

#[async_trait]
impl AudioMediaConverter for FfmpegAudioMediaConverter {
    async fn convert(&self, request: AudioConversionRequest) -> Result<Vec<u8>> {
        if request.audio.is_empty() {
            return Err(AstrbotError::Provider(format!(
                "{} audio media converter received empty audio",
                request.provider_label
            )));
        }

        fs::create_dir_all(&self.temp_dir).map_err(|err| {
            AstrbotError::Provider(format!(
                "failed to create audio conversion temp dir {}: {err}",
                self.temp_dir.display()
            ))
        })?;

        let base = unique_audio_conversion_name();
        let input_path = self
            .temp_dir
            .join(format!("{base}-input.{}", request.format.as_str()));
        let output_path = self.temp_dir.join(format!(
            "{base}-output.{}",
            request.target_format.extension()
        ));
        let runner = self.runner_for(request.target_format)?;

        let result = async {
            fs::write(&input_path, &request.audio).map_err(|err| {
                AstrbotError::Provider(format!(
                    "failed to write audio conversion input {}: {err}",
                    input_path.display()
                ))
            })?;
            runner
                .run(AudioConversionCommandRequest {
                    input_path: input_path.clone(),
                    output_path: output_path.clone(),
                    target_format: request.target_format,
                })
                .await?;
            let converted = fs::read(&output_path).map_err(|err| {
                AstrbotError::Provider(format!(
                    "failed to read converted audio {}: {err}",
                    output_path.display()
                ))
            })?;
            if converted.is_empty() {
                return Err(AstrbotError::Provider(format!(
                    "{} audio media converter returned empty {} audio",
                    request.provider_label, request.target_format
                )));
            }
            Ok(converted)
        }
        .await;

        let _ = fs::remove_file(&input_path);
        let _ = fs::remove_file(&output_path);
        result
    }
}

#[derive(Clone)]
pub struct AudioInputLoader {
    downloader: Arc<dyn MediaDownloadService>,
    download_policy: MediaDownloadPolicy,
    converter: Arc<dyn AudioMediaConverter>,
}

impl fmt::Debug for AudioInputLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioInputLoader").finish_non_exhaustive()
    }
}

impl AudioInputLoader {
    pub fn new(timeout: Duration) -> Result<Self> {
        let download_policy = MediaDownloadPolicy {
            timeout,
            max_bytes: None,
        };
        let downloader = Arc::new(ReqwestMediaDownloadService::new(download_policy.clone())?);

        Ok(Self {
            downloader,
            download_policy,
            converter: Arc::new(UnsupportedAudioMediaConverter),
        })
    }

    pub fn with_converter(mut self, converter: Arc<dyn AudioMediaConverter>) -> Self {
        self.converter = converter;
        self
    }

    pub fn with_downloader(mut self, downloader: Arc<dyn MediaDownloadService>) -> Self {
        self.downloader = downloader;
        self
    }

    pub async fn load(&self, audio_url: &str, provider_label: &str) -> Result<Vec<u8>> {
        if audio_url.trim().is_empty() {
            return Err(AstrbotError::Provider(
                "speech-to-text request must contain an audio URL".to_string(),
            ));
        }

        let audio = if is_http_url(audio_url) {
            self.download(audio_url, provider_label).await?
        } else {
            let audio = fs::read(audio_url).map_err(|err| {
                AstrbotError::Provider(format!("failed to read audio file {audio_url}: {err}"))
            })?;
            if audio.is_empty() {
                return Err(AstrbotError::Provider(format!(
                    "audio file {audio_url} was empty"
                )));
            }
            audio
        };

        let Some(format) = detect_audio_conversion_requirement(audio_url, &audio) else {
            return Ok(audio);
        };

        let converted = self
            .converter
            .convert(AudioConversionRequest {
                provider_label: provider_label.to_string(),
                audio_url: audio_url.to_string(),
                format,
                target_format: AudioTranscodeTarget::Wav,
                audio,
            })
            .await?;
        if converted.is_empty() {
            return Err(AstrbotError::Provider(format!(
                "{provider_label} audio media converter returned empty audio"
            )));
        }

        Ok(converted)
    }

    async fn download(&self, audio_url: &str, provider_label: &str) -> Result<Vec<u8>> {
        self.downloader
            .download(
                MediaDownloadRequest::new(audio_url.to_string())
                    .with_policy(self.download_policy.clone()),
            )
            .await
            .map(|media| media.bytes)
            .map_err(|err| {
                AstrbotError::Provider(format!("{provider_label} audio download failed: {err}"))
            })
    }
}

pub fn detect_audio_conversion_requirement(audio_url: &str, audio: &[u8]) -> Option<AudioFormat> {
    let audio_url = audio_url.to_ascii_lowercase();
    if audio.starts_with(b"SILK")
        || audio_url.ends_with(".silk")
        || audio_url.contains("multimedia.nt.qq.com.cn")
    {
        return Some(AudioFormat::Silk);
    }
    if audio.starts_with(b"#!AMR") || audio_url.ends_with(".amr") {
        return Some(AudioFormat::Amr);
    }

    None
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn unique_audio_conversion_name() -> String {
    let counter = AUDIO_CONVERSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("astrbot-audio-{}-{counter}-{nanos}", std::process::id())
}
