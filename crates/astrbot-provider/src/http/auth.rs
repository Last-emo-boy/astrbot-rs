use std::collections::HashMap;

use astrbot_core::{AstrbotError, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

pub(crate) fn json_bearer_headers(
    api_key: Option<&str>,
    custom_headers: &HashMap<String, String>,
    invalid_api_key_message: &str,
) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    insert_bearer_header(&mut headers, api_key, invalid_api_key_message)?;
    insert_custom_headers(&mut headers, custom_headers)?;
    Ok(headers)
}

pub(crate) fn bearer_headers(
    api_key: Option<&str>,
    custom_headers: &HashMap<String, String>,
    invalid_api_key_message: &str,
) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    insert_bearer_header(&mut headers, api_key, invalid_api_key_message)?;
    insert_custom_headers(&mut headers, custom_headers)?;
    Ok(headers)
}

pub(crate) fn json_api_key_headers(
    header_name: HeaderName,
    api_key: Option<&str>,
    custom_headers: &HashMap<String, String>,
    invalid_api_key_message: &str,
) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(api_key) = non_empty(api_key) {
        let value = HeaderValue::from_str(api_key)
            .map_err(|_| AstrbotError::Provider(invalid_api_key_message.to_string()))?;
        headers.insert(header_name, value);
    }

    insert_custom_headers(&mut headers, custom_headers)?;
    Ok(headers)
}

pub(crate) fn insert_custom_headers(
    headers: &mut HeaderMap,
    custom_headers: &HashMap<String, String>,
) -> Result<()> {
    for (key, value) in custom_headers {
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|_| {
            AstrbotError::Provider(format!("invalid custom provider header name: {key}"))
        })?;
        let value = HeaderValue::from_str(value).map_err(|_| {
            AstrbotError::Provider(format!("invalid custom provider header value for: {key}"))
        })?;
        headers.insert(name, value);
    }
    Ok(())
}

fn insert_bearer_header(
    headers: &mut HeaderMap,
    api_key: Option<&str>,
    invalid_api_key_message: &str,
) -> Result<()> {
    let Some(api_key) = non_empty(api_key) else {
        return Ok(());
    };

    let bearer = format!("Bearer {api_key}");
    let value = HeaderValue::from_str(&bearer)
        .map_err(|_| AstrbotError::Provider(invalid_api_key_message.to_string()))?;
    headers.insert(AUTHORIZATION, value);
    Ok(())
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

    use super::json_bearer_headers;

    #[test]
    fn json_bearer_headers_adds_content_type_bearer_and_custom_headers() {
        let mut custom = HashMap::new();
        custom.insert("x-extra".to_string(), "value".to_string());

        let headers =
            json_bearer_headers(Some("secret"), &custom, "bad key").expect("headers should build");

        assert_eq!(
            headers
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        assert_eq!(
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer secret")
        );
        assert_eq!(
            headers.get("x-extra").and_then(|value| value.to_str().ok()),
            Some("value")
        );
    }
}
