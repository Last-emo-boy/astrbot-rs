use astrbot_core::Result;
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaItem {
    pub media_type: String,
    pub file_name: String,
    pub content: Vec<u8>,
    pub mime_type: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseResult {
    pub text: String,
    pub media: Vec<MediaItem>,
}

#[async_trait]
pub trait DocumentParser: Send + Sync {
    async fn parse(&self, file_content: Vec<u8>, file_name: &str) -> Result<ParseResult>;
}

#[derive(Clone, Debug, Default)]
pub struct PlainTextParser;

#[async_trait]
impl DocumentParser for PlainTextParser {
    async fn parse(&self, file_content: Vec<u8>, _file_name: &str) -> Result<ParseResult> {
        let text = String::from_utf8(file_content)
            .map_err(|error| crate::types::kb_error(format!("invalid utf-8 text: {error}")))?;
        Ok(ParseResult {
            text,
            media: Vec::new(),
        })
    }
}
