use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct TempAudioFile {
    path: PathBuf,
}

impl TempAudioFile {
    pub fn new(label: &str, extension: &str, contents: impl AsRef<[u8]>) -> Self {
        let path = unique_temp_path(label, extension);
        fs::write(&path, contents).expect("audio fixture should be written");
        Self { path }
    }

    pub fn wav(label: &str, contents: impl AsRef<[u8]>) -> Self {
        Self::new(label, "wav", contents)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn path_string(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

impl Drop for TempAudioFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug)]
pub struct TempOutputDir {
    path: PathBuf,
}

impl TempOutputDir {
    pub fn new(label: &str) -> Self {
        Self {
            path: unique_temp_dir(label),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempOutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
pub struct GeneratedAudioFile {
    path: PathBuf,
}

impl GeneratedAudioFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn read(&self) -> Vec<u8> {
        fs::read(&self.path).expect("audio file should be written")
    }
}

impl Drop for GeneratedAudioFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-rs-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ))
}

fn unique_temp_path(label: &str, extension: &str) -> PathBuf {
    let extension = extension.trim_start_matches('.');
    unique_temp_dir(label).with_extension(extension)
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
