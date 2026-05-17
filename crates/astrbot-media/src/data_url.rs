use astrbot_core::{AstrbotError, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::mime::is_supported_image_mime_type;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataUrl {
    mime_type: String,
    data: String,
}

impl DataUrl {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        let Some(rest) = value.strip_prefix("data:") else {
            return Err(AstrbotError::Provider(
                "media input must be a data URL".to_string(),
            ));
        };
        let Some((metadata, data)) = rest.split_once(',') else {
            return Err(AstrbotError::Provider("invalid media data URL".to_string()));
        };
        let mut metadata_parts = metadata.split(';');
        let mime_type = metadata_parts
            .next()
            .filter(|mime_type| !mime_type.trim().is_empty())
            .ok_or_else(|| {
                AstrbotError::Provider("missing media data URL MIME type".to_string())
            })?;
        if !metadata_parts.any(|part| part.eq_ignore_ascii_case("base64")) {
            return Err(AstrbotError::Provider(
                "media data URL must be base64 encoded".to_string(),
            ));
        }
        STANDARD.decode(data).map_err(|err| {
            AstrbotError::Provider(format!("invalid base64 media data URL payload: {err}"))
        })?;

        Ok(Self {
            mime_type: mime_type.to_ascii_lowercase(),
            data: data.to_string(),
        })
    }

    pub fn parse_image(value: &str) -> Result<Self> {
        let parsed = Self::parse(value)?;
        if !is_supported_image_mime_type(parsed.mime_type()) {
            return Err(AstrbotError::Provider(format!(
                "unsupported image data URL MIME type: {}",
                parsed.mime_type()
            )));
        }
        Ok(parsed)
    }

    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    pub fn base64_data(&self) -> &str {
        &self.data
    }

    pub fn decode_bytes(&self) -> Result<Vec<u8>> {
        STANDARD.decode(&self.data).map_err(|err| {
            AstrbotError::Provider(format!("invalid base64 media data URL payload: {err}"))
        })
    }

    pub fn to_data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime_type, self.data)
    }

    pub fn from_base64(mime_type: impl Into<String>, data: impl Into<String>) -> Result<Self> {
        let mime_type = mime_type.into().trim().to_ascii_lowercase();
        let data = data.into();
        STANDARD.decode(&data).map_err(|err| {
            AstrbotError::Provider(format!("invalid base64 media payload: {err}"))
        })?;
        Ok(Self { mime_type, data })
    }
}

pub fn encode_data_url(mime_type: &str, bytes: &[u8]) -> String {
    format!("data:{mime_type};base64,{}", STANDARD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::{DataUrl, encode_data_url};

    #[test]
    fn parses_image_data_url_once_for_all_provider_serializers() {
        let parsed =
            DataUrl::parse_image("data:image/png;base64,iVBORw0KGgo=").expect("valid image");

        assert_eq!(parsed.mime_type(), "image/png");
        assert_eq!(parsed.base64_data(), "iVBORw0KGgo=");
    }

    #[test]
    fn rejects_remote_urls_before_provider_protocol_serialization() {
        let error = DataUrl::parse_image("https://example.test/image.png").expect_err("remote URL");

        assert!(error.to_string().contains("data URL"));
    }

    #[test]
    fn encodes_bytes_as_data_url() {
        assert_eq!(
            encode_data_url("image/png", b"png"),
            "data:image/png;base64,cG5n"
        );
    }
}
