//! Context compression — manage conversation history length for LLM requests
//!
//! Provides configurable strategies to keep context within model limits:
//! - Truncate: drop oldest messages beyond a threshold
//! - Summarize: replace early messages with a summary (placeholder)

use crate::errors::{AstrBotError, Result};
use crate::provider::ChatMessage;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Compression strategy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionStrategy {
    /// Keep only the most recent N messages
    Truncate,
    /// Replace early messages with a summary (placeholder — not yet implemented)
    Summarize,
}

impl Default for CompressionStrategy {
    fn default() -> Self {
        Self::Truncate
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCompressionConfig {
    /// Max messages to send to LLM (excluding system prompt)
    pub max_messages: usize,
    /// Strategy when history exceeds max_messages
    pub strategy: CompressionStrategy,
    /// Max tokens estimate (soft limit — skeleton, not enforced yet)
    pub max_tokens_estimate: Option<usize>,
}

impl ContextCompressionConfig {
    pub fn new(max_messages: usize) -> Self {
        Self {
            max_messages,
            strategy: CompressionStrategy::Truncate,
            max_tokens_estimate: None,
        }
    }

    pub fn with_strategy(mut self, strategy: CompressionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_max_tokens_estimate(mut self, max: usize) -> Self {
        self.max_tokens_estimate = Some(max);
        self
    }
}

impl Default for ContextCompressionConfig {
    fn default() -> Self {
        Self::new(20)
    }
}

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

/// Compressor that applies the configured strategy to a message list.
pub struct ContextCompressor {
    config: ContextCompressionConfig,
}

impl ContextCompressor {
    pub fn new(config: ContextCompressionConfig) -> Self {
        Self { config }
    }

    pub fn default_truncate() -> Self {
        Self::new(ContextCompressionConfig::default())
    }

    /// Compress messages in-place.
    ///
    /// System prompt (first message with role="system") is preserved.
    /// The rest is compressed according to the strategy.
    pub fn compress(&self, messages: &mut Vec<ChatMessage>) -> Result<()> {
        if messages.is_empty() {
            return Ok(());
        }

        // Find system prompt index (if any) — don't count it against the limit
        let system_idx = messages
            .iter()
            .position(|m| m.role == "system")
            .map(|_| 0usize); // system prompt is always at index 0 in our convention

        let non_system_count = if system_idx.is_some() {
            messages.len().saturating_sub(1)
        } else {
            messages.len()
        };

        if non_system_count <= self.config.max_messages {
            return Ok(());
        }

        match self.config.strategy {
            CompressionStrategy::Truncate => self.truncate(messages, system_idx.is_some()),
            CompressionStrategy::Summarize => {
                // Placeholder: fall back to truncate until summarizer is wired
                self.truncate(messages, system_idx.is_some())
            }
        }
    }

    /// Truncate to keep only the most recent messages.
    fn truncate(&self, messages: &mut Vec<ChatMessage>, has_system: bool) -> Result<()> {
        let keep = self.config.max_messages;

        if has_system {
            // Keep system prompt at index 0, then last `keep` messages
            if messages.len() > keep + 1 {
                let split_point = messages.len() - keep;
                // Remove messages [1..split_point], keep [0] + [split_point..]
                let mut retained = vec![messages[0].clone()];
                retained.extend_from_slice(&messages[split_point..]);
                *messages = retained;
            }
        } else {
            // No system prompt — just keep last `keep`
            if messages.len() > keep {
                let split_point = messages.len() - keep;
                messages.drain(0..split_point);
            }
        }

        Ok(())
    }

    /// Placeholder for summarization strategy.
    /// In the future this will:
    /// 1. Identify messages to summarize (oldest N beyond threshold)
    /// 2. Call an LLM or internal summarizer to produce a condensed version
    /// 3. Replace the summarized messages with a single system/note message
    pub fn summarize_placeholder(&self, _messages: &mut Vec<ChatMessage>) -> Result<()> {
        // TODO: wire summarizer (requires LLM call — complex, deferred)
        Err(AstrBotError::NotImplemented(
            "Summarize strategy not yet implemented — use Truncate".to_string(),
        ))
    }

    pub fn config(&self) -> &ContextCompressionConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(count: usize) -> Vec<ChatMessage> {
        (0..count)
            .map(|i| ChatMessage::user(format!("msg{}", i)))
            .collect()
    }

    #[test]
    fn test_compress_noop_when_under_limit() {
        let comp = ContextCompressor::default_truncate();
        let mut msgs = make_messages(5);
        comp.compress(&mut msgs).unwrap();
        assert_eq!(msgs.len(), 5);
    }

    #[test]
    fn test_truncate_without_system() {
        let comp = ContextCompressor::new(ContextCompressionConfig::new(3));
        let mut msgs = make_messages(10);
        comp.compress(&mut msgs).unwrap();
        assert_eq!(msgs.len(), 3);
        // Kept the last 3
        assert_eq!(msgs[0].content, "msg7");
        assert_eq!(msgs[1].content, "msg8");
        assert_eq!(msgs[2].content, "msg9");
    }

    #[test]
    fn test_truncate_with_system_prompt() {
        let comp = ContextCompressor::new(ContextCompressionConfig::new(3));
        let mut msgs = vec![ChatMessage::system("You are helpful")];
        msgs.extend(make_messages(10));
        comp.compress(&mut msgs).unwrap();
        assert_eq!(msgs.len(), 4); // system + 3 recent
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[0].content, "You are helpful");
        assert_eq!(msgs[1].content, "msg7");
        assert_eq!(msgs[2].content, "msg8");
        assert_eq!(msgs[3].content, "msg9");
    }

    #[test]
    fn test_truncate_exactly_at_limit() {
        let comp = ContextCompressor::new(ContextCompressionConfig::new(5));
        let mut msgs = vec![ChatMessage::system("sys")];
        msgs.extend(make_messages(5));
        comp.compress(&mut msgs).unwrap();
        assert_eq!(msgs.len(), 6); // no truncation needed
    }

    #[test]
    fn test_empty_messages() {
        let comp = ContextCompressor::default_truncate();
        let mut msgs: Vec<ChatMessage> = vec![];
        comp.compress(&mut msgs).unwrap();
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_system_only() {
        let comp = ContextCompressor::new(ContextCompressionConfig::new(2));
        let mut msgs = vec![ChatMessage::system("sys")];
        comp.compress(&mut msgs).unwrap();
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_config_serialization() {
        let config = ContextCompressionConfig::new(10)
            .with_strategy(CompressionStrategy::Summarize)
            .with_max_tokens_estimate(4096);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("10"));
        assert!(json.contains("Summarize"));

        let decoded: ContextCompressionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.max_messages, 10);
        assert_eq!(decoded.strategy, CompressionStrategy::Summarize);
        assert_eq!(decoded.max_tokens_estimate, Some(4096));
    }
}
