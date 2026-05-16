use astrbot_core::Result;
use async_trait::async_trait;

use crate::types::kb_error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkingOptions {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
}

impl Default for ChunkingOptions {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 50,
        }
    }
}

impl ChunkingOptions {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Result<Self> {
        if chunk_size == 0 {
            return Err(kb_error("chunk_size must be greater than 0"));
        }
        if chunk_overlap >= chunk_size {
            return Err(kb_error("chunk_overlap must be less than chunk_size"));
        }
        Ok(Self {
            chunk_size,
            chunk_overlap,
        })
    }
}

#[async_trait]
pub trait DocumentChunker: Send + Sync {
    async fn chunk(&self, text: &str, options: ChunkingOptions) -> Result<Vec<String>>;
}

#[derive(Clone, Debug)]
pub struct RecursiveCharacterChunker {
    separators: Vec<String>,
}

impl Default for RecursiveCharacterChunker {
    fn default() -> Self {
        Self {
            separators: vec![
                "\n\n".to_string(),
                "\n".to_string(),
                "。".to_string(),
                "，".to_string(),
                ". ".to_string(),
                ", ".to_string(),
                " ".to_string(),
                String::new(),
            ],
        }
    }
}

impl RecursiveCharacterChunker {
    pub fn with_separators<I, S>(separators: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            separators: separators.into_iter().map(Into::into).collect(),
        }
    }

    fn chunk_recursive(&self, text: &str, options: &ChunkingOptions) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }
        if text.chars().count() <= options.chunk_size {
            return vec![text.to_string()];
        }

        for separator in &self.separators {
            if separator.is_empty() {
                return split_by_character(text, options);
            }
            if !text.contains(separator) {
                continue;
            }

            let mut splits = text
                .split(separator)
                .filter(|part| !part.is_empty())
                .map(|part| format!("{part}{separator}"))
                .collect::<Vec<_>>();
            if text.ends_with(separator) {
                if let Some(last) = splits.last_mut() {
                    *last = last.trim_end_matches(separator).to_string() + separator;
                }
            } else if let Some(last) = splits.last_mut() {
                *last = last.trim_end_matches(separator).to_string();
            }
            if splits.len() <= 1 {
                continue;
            }

            let mut chunks = Vec::new();
            let mut current = String::new();
            for split in splits {
                if split.chars().count() > options.chunk_size {
                    if !current.is_empty() {
                        chunks.extend(self.chunk_recursive(&current, options));
                        current.clear();
                    }
                    chunks.extend(self.chunk_recursive(&split, options));
                    continue;
                }

                if current.chars().count() + split.chars().count() > options.chunk_size {
                    chunks.push(current.clone());
                    current = overlap_suffix(&current, options.chunk_overlap);
                    current.push_str(&split);
                } else {
                    current.push_str(&split);
                }
            }
            if !current.is_empty() {
                chunks.push(current);
            }
            return chunks;
        }

        vec![text.to_string()]
    }
}

#[async_trait]
impl DocumentChunker for RecursiveCharacterChunker {
    async fn chunk(&self, text: &str, options: ChunkingOptions) -> Result<Vec<String>> {
        ChunkingOptions::new(options.chunk_size, options.chunk_overlap)?;
        Ok(self.chunk_recursive(text, &options))
    }
}

fn split_by_character(text: &str, options: &ChunkingOptions) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let step = options.chunk_size - options.chunk_overlap;
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + options.chunk_size).min(chars.len());
        chunks.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        start += step;
    }
    chunks
}

fn overlap_suffix(text: &str, overlap: usize) -> String {
    if overlap == 0 {
        return String::new();
    }
    let chars = text.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(overlap);
    chars[start..].iter().collect()
}
