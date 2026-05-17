use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadProgressSnapshot {
    pub url: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub elapsed_millis: u128,
}

impl DownloadProgressSnapshot {
    pub fn percent(&self) -> Option<f32> {
        let total = self.total_bytes?;
        if total == 0 {
            return None;
        }
        Some(self.downloaded_bytes as f32 / total as f32)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadProgressEvent {
    Started(DownloadProgressSnapshot),
    Advanced(DownloadProgressSnapshot),
    Finished(DownloadProgressSnapshot),
}

#[async_trait]
pub trait DownloadProgressSink: Send + Sync {
    async fn record(&self, event: DownloadProgressEvent);
}

#[derive(Clone, Debug, Default)]
pub struct NoopDownloadProgressSink;

#[async_trait]
impl DownloadProgressSink for NoopDownloadProgressSink {
    async fn record(&self, _event: DownloadProgressEvent) {}
}

pub(crate) struct ProgressTracker {
    url: String,
    total_bytes: Option<u64>,
    downloaded_bytes: u64,
    started_at: Instant,
}

impl ProgressTracker {
    pub(crate) fn new(url: impl Into<String>, total_bytes: Option<u64>) -> Self {
        Self {
            url: url.into(),
            total_bytes,
            downloaded_bytes: 0,
            started_at: Instant::now(),
        }
    }

    pub(crate) fn advance(&mut self, bytes: u64) -> DownloadProgressSnapshot {
        self.downloaded_bytes += bytes;
        self.snapshot()
    }

    pub(crate) fn snapshot(&self) -> DownloadProgressSnapshot {
        DownloadProgressSnapshot {
            url: self.url.clone(),
            downloaded_bytes: self.downloaded_bytes,
            total_bytes: self.total_bytes,
            elapsed_millis: self.started_at.elapsed().as_millis(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DownloadProgressSnapshot;

    #[test]
    fn progress_snapshot_reports_fraction_when_total_known() {
        let snapshot = DownloadProgressSnapshot {
            url: "https://example.test/file".to_string(),
            downloaded_bytes: 25,
            total_bytes: Some(100),
            elapsed_millis: 10,
        };

        assert_eq!(snapshot.percent(), Some(0.25));
    }
}
