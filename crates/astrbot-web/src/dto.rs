use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../../dashboard-next/src/api/dto/"))]
pub struct SubmitTextRequest {
    pub sender_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub image_urls: Vec<String>,
    #[serde(default)]
    pub message_parts: Vec<WebChatMessagePart>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../../dashboard-next/src/api/dto/"))]
pub enum WebChatMessagePart {
    Plain {
        text: String,
    },
    Image {
        #[serde(alias = "image_url")]
        url: String,
    },
    Reply {
        #[serde(deserialize_with = "deserialize_stringish")]
        message_id: String,
        #[serde(default)]
        selected_text: String,
    },
    Record {
        #[serde(alias = "record_url")]
        url: String,
    },
    Video {
        #[serde(alias = "video_url")]
        url: String,
    },
    File {
        #[serde(default, alias = "filename")]
        name: String,
        #[serde(alias = "file_url")]
        url: String,
    },
}

fn deserialize_stringish<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringishVisitor;

    impl<'de> de::Visitor<'de> for StringishVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string or integer")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }
    }

    deserializer.deserialize_any(StringishVisitor)
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmitTextResponse {
    pub event_id: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebChatMessageResponse {
    pub text: String,
    #[serde(default)]
    pub image_urls: Vec<String>,
    #[serde(default)]
    pub message_parts: Vec<WebChatMessagePart>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebChatMessagesResponse {
    pub conversation_id: String,
    pub messages: Vec<WebChatMessageResponse>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../../dashboard-next/src/api/dto/"))]
pub struct ErrorResponse {
    pub error: String,
}
