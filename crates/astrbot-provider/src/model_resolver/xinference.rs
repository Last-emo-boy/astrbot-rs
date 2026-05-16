use std::sync::{Arc, Mutex};

use astrbot_core::{AstrbotError, Result};

use crate::http::{extract_error_message, join_api_path};
use crate::protocol::xinference::{
    XinferenceLaunchModelRequest, parse_launch_model_uid, parse_running_model_uid,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum XinferenceModelType {
    Audio,
    Rerank,
}

impl XinferenceModelType {
    fn as_protocol_type(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Rerank => "rerank",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct XinferenceModelResolver {
    client: reqwest::Client,
    models_url: String,
    requested_model: String,
    model_type: XinferenceModelType,
    launch_model_if_not_running: bool,
    model_uid: Arc<Mutex<Option<String>>>,
}

impl XinferenceModelResolver {
    pub(crate) fn new(
        client: reqwest::Client,
        api_base: impl AsRef<str>,
        requested_model: impl Into<String>,
        model_type: XinferenceModelType,
        launch_model_if_not_running: bool,
    ) -> Self {
        Self {
            client,
            models_url: join_api_path(api_base.as_ref(), "v1/models"),
            requested_model: requested_model.into(),
            model_type,
            launch_model_if_not_running,
            model_uid: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) async fn resolve_model_uid(&self) -> Result<String> {
        if let Some(model_uid) = self.cached_model_uid()? {
            return Ok(model_uid);
        }

        if let Some(model_uid) = self.find_running_model_uid().await? {
            return self.cache_model_uid(model_uid);
        }

        if self.launch_model_if_not_running {
            let model_uid = self.launch_model().await?;
            return self.cache_model_uid(model_uid);
        }

        Err(AstrbotError::Provider(format!(
            "Xinference {} model {} is not running and auto-launch is disabled",
            self.model_type.as_protocol_type(),
            self.requested_model
        )))
    }

    fn cached_model_uid(&self) -> Result<Option<String>> {
        self.model_uid
            .lock()
            .map(|model_uid| model_uid.clone())
            .map_err(|_| AstrbotError::Provider("Xinference model UID cache poisoned".to_string()))
    }

    fn cache_model_uid(&self, model_uid: String) -> Result<String> {
        let mut cached = self.model_uid.lock().map_err(|_| {
            AstrbotError::Provider("Xinference model UID cache poisoned".to_string())
        })?;
        *cached = Some(model_uid.clone());
        Ok(model_uid)
    }

    async fn find_running_model_uid(&self) -> Result<Option<String>> {
        let response = self
            .client
            .get(&self.models_url)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Xinference list models request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Xinference list models").await?;
        parse_running_model_uid(&body, &self.requested_model)
    }

    async fn launch_model(&self) -> Result<String> {
        let response = self
            .client
            .post(&self.models_url)
            .json(&XinferenceLaunchModelRequest {
                model_name: self.requested_model.clone(),
                model_type: self.model_type.as_protocol_type(),
            })
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Xinference launch model request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Xinference launch model").await?;
        parse_launch_model_uid(&body)
    }
}

async fn response_body_or_error(response: reqwest::Response, label: &str) -> Result<String> {
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        AstrbotError::Provider(format!("failed to read provider response: {err}"))
    })?;

    if !status.is_success() {
        return Err(AstrbotError::Provider(format!(
            "{label} returned {status}: {}",
            extract_error_message(&body)
        )));
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::{XinferenceModelResolver, XinferenceModelType};

    #[test]
    fn model_type_maps_to_xinference_launch_payload_type() {
        assert_eq!(XinferenceModelType::Audio.as_protocol_type(), "audio");
        assert_eq!(XinferenceModelType::Rerank.as_protocol_type(), "rerank");
    }

    #[tokio::test]
    async fn resolver_reports_not_running_when_auto_launch_is_disabled() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept test request");
            use std::io::{Read, Write};
            let mut buffer = [0; 1024];
            let _ = stream.read(&mut buffer);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 11\r\n\r\n{\"data\":[]}",
                )
                .expect("write test response");
        });
        let base_url = format!("http://{addr}");
        let resolver = XinferenceModelResolver::new(
            reqwest::Client::new(),
            base_url,
            "missing-model",
            XinferenceModelType::Rerank,
            false,
        );

        let err = resolver
            .resolve_model_uid()
            .await
            .expect_err("missing model should fail");

        assert!(err.to_string().contains("auto-launch is disabled"));
        handle.join().expect("test server should finish");
    }
}
