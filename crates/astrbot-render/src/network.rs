use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::t2i::{
    RenderArtifact, RenderFormat, RenderMode, RenderStrategy, T2iRenderRequest, T2iRenderResult,
    T2iRenderer,
};
use crate::template::TemplateCatalog;

pub const DEFAULT_T2I_ENDPOINT: &str = "https://t2i.soulter.top/text2img";
pub const OFFICIAL_T2I_ENDPOINTS_URL: &str = "https://api.soulter.top/astrbot/t2i-endpoints";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct T2iEndpoint {
    url: String,
}

impl T2iEndpoint {
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url = clean_endpoint_url(&url.into())?;
        Ok(Self { url })
    }

    pub fn default_endpoint() -> Self {
        Self {
            url: DEFAULT_T2I_ENDPOINT.to_string(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.url
    }

    pub fn generate_url(&self) -> String {
        format!("{}/generate", self.url)
    }

    pub fn artifact_url(&self, artifact_id: &str) -> String {
        format!("{}/{}", self.url, artifact_id.trim_start_matches('/'))
    }
}

impl Default for T2iEndpoint {
    fn default() -> Self {
        Self::default_endpoint()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialT2iEndpointDescriptor {
    pub url: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkT2iEndpointCatalog {
    base_endpoint: T2iEndpoint,
    endpoints: Vec<T2iEndpoint>,
}

impl NetworkT2iEndpointCatalog {
    pub fn new(base_endpoint: impl Into<String>) -> Result<Self> {
        let base_endpoint = T2iEndpoint::new(base_endpoint)?;
        Ok(Self {
            endpoints: vec![base_endpoint.clone()],
            base_endpoint,
        })
    }

    pub fn default_official() -> Self {
        Self {
            base_endpoint: T2iEndpoint::default_endpoint(),
            endpoints: vec![T2iEndpoint::default_endpoint()],
        }
    }

    pub fn with_official_endpoints<I>(mut self, endpoints: I) -> Self
    where
        I: IntoIterator<Item = OfficialT2iEndpointDescriptor>,
    {
        let discovered = endpoints
            .into_iter()
            .filter(|endpoint| endpoint.active)
            .filter_map(|endpoint| T2iEndpoint::new(endpoint.url).ok())
            .collect::<Vec<_>>();
        if !discovered.is_empty() {
            self.endpoints = discovered;
        }
        self
    }

    pub fn endpoints(&self) -> &[T2iEndpoint] {
        if self.endpoints.is_empty() {
            std::slice::from_ref(&self.base_endpoint)
        } else {
            &self.endpoints
        }
    }
}

impl Default for NetworkT2iEndpointCatalog {
    fn default() -> Self {
        Self::default_official()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkT2iRequestPayload {
    pub generate_url: String,
    pub template: String,
    pub template_data: serde_json::Map<String, Value>,
    pub return_url: bool,
    pub options: NetworkT2iRenderOptions,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkT2iRenderOptions {
    pub full_page: bool,
    pub format: RenderFormat,
    pub quality: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkT2iRenderOutput {
    ArtifactId(String),
    FilePath(String),
}

#[async_trait]
pub trait NetworkT2iClient: Send + Sync {
    async fn render(
        &self,
        endpoint: &T2iEndpoint,
        payload: NetworkT2iRequestPayload,
    ) -> Result<NetworkT2iRenderOutput>;
}

#[derive(Clone)]
pub struct NetworkT2iRenderer {
    catalog: NetworkT2iEndpointCatalog,
    templates: TemplateCatalog,
    client: Arc<dyn NetworkT2iClient>,
}

impl NetworkT2iRenderer {
    pub fn new(
        catalog: NetworkT2iEndpointCatalog,
        templates: TemplateCatalog,
        client: Arc<dyn NetworkT2iClient>,
    ) -> Self {
        Self {
            catalog,
            templates,
            client,
        }
    }
}

#[async_trait]
impl T2iRenderer for NetworkT2iRenderer {
    async fn render(&self, request: T2iRenderRequest) -> Result<T2iRenderResult> {
        if matches!(request.options.strategy, RenderStrategy::LocalOnly) {
            return Err(AstrbotError::Pipeline(
                "network T2I renderer cannot satisfy local-only requests".to_string(),
            ));
        }

        let template = self
            .templates
            .get_template(&request.options.template_name)?;
        let payload_options = NetworkT2iRenderOptions {
            full_page: request.options.full_page,
            format: request.options.format,
            quality: request.options.quality,
        };
        let template_data = request
            .template_data
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<serde_json::Map<_, _>>();

        let mut last_error = None;
        for endpoint in self.catalog.endpoints() {
            let payload = NetworkT2iRequestPayload {
                generate_url: endpoint.generate_url(),
                template: template.clone(),
                template_data: template_data.clone(),
                return_url: matches!(request.options.mode, RenderMode::Url),
                options: payload_options.clone(),
            };
            match self.client.render(endpoint, payload).await {
                Ok(output) => {
                    return network_output_to_result(endpoint, &request, output);
                }
                Err(error) => {
                    last_error = Some(error.to_string());
                }
            }
        }

        Err(AstrbotError::Pipeline(format!(
            "all T2I endpoints failed: {}",
            last_error.unwrap_or_else(|| "no endpoint attempted".to_string())
        )))
    }
}

fn network_output_to_result(
    endpoint: &T2iEndpoint,
    request: &T2iRenderRequest,
    output: NetworkT2iRenderOutput,
) -> Result<T2iRenderResult> {
    let artifact = match (request.options.mode, output) {
        (RenderMode::Url, NetworkT2iRenderOutput::ArtifactId(id)) => {
            RenderArtifact::url(endpoint.artifact_url(&id), request.options.format)
        }
        (RenderMode::File, NetworkT2iRenderOutput::FilePath(path)) => {
            RenderArtifact::file(path, request.options.format)
        }
        (RenderMode::Url, NetworkT2iRenderOutput::FilePath(path)) => {
            return Err(AstrbotError::Pipeline(format!(
                "network T2I client returned file path for URL mode: {path}"
            )));
        }
        (RenderMode::File, NetworkT2iRenderOutput::ArtifactId(id)) => {
            return Err(AstrbotError::Pipeline(format!(
                "network T2I client returned artifact id for file mode: {id}"
            )));
        }
    };

    Ok(T2iRenderResult {
        artifact,
        template_name: request.options.template_name.clone(),
        strategy_used: RenderStrategy::NetworkOnly,
    })
}

fn clean_endpoint_url(url: &str) -> Result<String> {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(AstrbotError::Pipeline(
            "T2I endpoint URL cannot be empty".to_string(),
        ));
    }
    if trimmed.ends_with("/text2img") || trimmed.ends_with("text2img") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed}/text2img"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use astrbot_core::{AstrbotError, Result};
    use async_trait::async_trait;

    use crate::{
        NetworkT2iClient, NetworkT2iEndpointCatalog, NetworkT2iRenderOutput, NetworkT2iRenderer,
        OfficialT2iEndpointDescriptor, RenderArtifactKind, RenderMode, RenderOptions,
        RenderStrategy, T2iEndpoint, T2iRenderRequest, T2iRenderer, TemplateCatalog, TemplateName,
    };

    use super::NetworkT2iRequestPayload;

    #[test]
    fn endpoint_catalog_normalizes_base_and_filters_inactive_official_endpoints() {
        let catalog = NetworkT2iEndpointCatalog::new("https://example.test").unwrap();
        assert_eq!(
            catalog.endpoints()[0].as_str(),
            "https://example.test/text2img"
        );

        let catalog = catalog.with_official_endpoints([
            OfficialT2iEndpointDescriptor {
                url: "https://inactive.test".to_string(),
                active: false,
            },
            OfficialT2iEndpointDescriptor {
                url: "https://active.test/text2img/".to_string(),
                active: true,
            },
        ]);

        assert_eq!(
            catalog.endpoints()[0].as_str(),
            "https://active.test/text2img"
        );
    }

    #[tokio::test]
    async fn network_renderer_falls_back_across_endpoints_and_returns_url() {
        let catalog = NetworkT2iEndpointCatalog::new("https://first.test")
            .unwrap()
            .with_official_endpoints([
                OfficialT2iEndpointDescriptor {
                    url: "https://first.test".to_string(),
                    active: true,
                },
                OfficialT2iEndpointDescriptor {
                    url: "https://second.test".to_string(),
                    active: true,
                },
            ]);
        let templates = TemplateCatalog::without_user_dir();
        let client = Arc::new(FallbackClient::default());
        let renderer = NetworkT2iRenderer::new(catalog, templates, client.clone());
        let request = T2iRenderRequest::from_text("hello").with_options(RenderOptions {
            strategy: RenderStrategy::NetworkOnly,
            mode: RenderMode::Url,
            template_name: TemplateName::base(),
            ..RenderOptions::default()
        });

        let result = renderer.render(request).await.unwrap();

        assert_eq!(result.artifact.kind, RenderArtifactKind::Url);
        assert_eq!(
            result.artifact.value,
            "https://second.test/text2img/image-id"
        );
        let attempts = client.attempts.lock().unwrap();
        assert_eq!(
            attempts.as_slice(),
            [
                "https://first.test/text2img",
                "https://second.test/text2img"
            ]
        );
    }

    #[derive(Default)]
    struct FallbackClient {
        attempts: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl NetworkT2iClient for FallbackClient {
        async fn render(
            &self,
            endpoint: &T2iEndpoint,
            payload: NetworkT2iRequestPayload,
        ) -> Result<NetworkT2iRenderOutput> {
            assert_eq!(payload.generate_url, endpoint.generate_url());
            self.attempts
                .lock()
                .unwrap()
                .push(endpoint.as_str().to_string());
            if endpoint.as_str().contains("first") {
                Err(AstrbotError::Pipeline("first endpoint failed".to_string()))
            } else {
                Ok(NetworkT2iRenderOutput::ArtifactId("image-id".to_string()))
            }
        }
    }
}
