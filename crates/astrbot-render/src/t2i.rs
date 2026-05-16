use std::collections::BTreeMap;
use std::path::PathBuf;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::template::{TemplateCatalog, TemplateName};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderStrategy {
    #[default]
    NetworkPreferred,
    NetworkOnly,
    LocalOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderFormat {
    Png,
    #[default]
    Jpeg,
}

impl RenderFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderMode {
    #[default]
    File,
    Url,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOptions {
    pub strategy: RenderStrategy,
    pub mode: RenderMode,
    pub format: RenderFormat,
    pub full_page: bool,
    pub quality: u8,
    pub template_name: TemplateName,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            strategy: RenderStrategy::NetworkPreferred,
            mode: RenderMode::File,
            format: RenderFormat::Jpeg,
            full_page: true,
            quality: 40,
            template_name: TemplateName::default(),
        }
    }
}

impl RenderOptions {
    pub fn local_file(template_name: TemplateName) -> Self {
        Self {
            strategy: RenderStrategy::LocalOnly,
            mode: RenderMode::File,
            template_name,
            ..Self::default()
        }
    }

    pub fn network_url(template_name: TemplateName) -> Self {
        Self {
            strategy: RenderStrategy::NetworkOnly,
            mode: RenderMode::Url,
            template_name,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct T2iRenderRequest {
    pub text: String,
    pub template_data: BTreeMap<String, Value>,
    pub options: RenderOptions,
}

impl T2iRenderRequest {
    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let mut template_data = BTreeMap::new();
        template_data.insert(
            "text".to_string(),
            Value::String(escape_template_text(&text)),
        );
        template_data.insert("version".to_string(), Value::String("v0.1.0".to_string()));

        Self {
            text,
            template_data,
            options: RenderOptions::default(),
        }
    }

    pub fn with_options(mut self, options: RenderOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_template_data(mut self, key: impl Into<String>, value: Value) -> Self {
        self.template_data.insert(key.into(), value);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderArtifactKind {
    File,
    Url,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderArtifact {
    pub kind: RenderArtifactKind,
    pub value: String,
    pub format: RenderFormat,
}

impl RenderArtifact {
    pub fn file(path: impl Into<PathBuf>, format: RenderFormat) -> Self {
        Self {
            kind: RenderArtifactKind::File,
            value: path.into().to_string_lossy().to_string(),
            format,
        }
    }

    pub fn url(url: impl Into<String>, format: RenderFormat) -> Self {
        Self {
            kind: RenderArtifactKind::Url,
            value: url.into(),
            format,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct T2iRenderResult {
    pub artifact: RenderArtifact,
    pub template_name: TemplateName,
    pub strategy_used: RenderStrategy,
}

#[async_trait]
pub trait T2iRenderer: Send + Sync {
    async fn render(&self, request: T2iRenderRequest) -> Result<T2iRenderResult>;
}

#[derive(Clone, Debug)]
pub struct TemplateRenderer {
    catalog: TemplateCatalog,
    output_dir: PathBuf,
}

impl TemplateRenderer {
    pub fn new(catalog: TemplateCatalog, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            catalog,
            output_dir: output_dir.into(),
        }
    }

    pub fn catalog(&self) -> &TemplateCatalog {
        &self.catalog
    }

    fn artifact_path(&self, request: &T2iRenderRequest) -> PathBuf {
        self.output_dir.join(format!(
            "{}.{}",
            request.options.template_name.as_str(),
            request.options.format.extension()
        ))
    }
}

#[async_trait]
impl T2iRenderer for TemplateRenderer {
    async fn render(&self, request: T2iRenderRequest) -> Result<T2iRenderResult> {
        if matches!(request.options.mode, RenderMode::Url) {
            return Err(AstrbotError::Pipeline(
                "template renderer only supports file artifacts".to_string(),
            ));
        }
        if matches!(request.options.strategy, RenderStrategy::NetworkOnly) {
            return Err(AstrbotError::Pipeline(
                "template renderer cannot satisfy network-only T2I requests".to_string(),
            ));
        }

        let template = self.catalog.get_template(&request.options.template_name)?;
        let rendered = render_template_string(&template, &request.template_data);
        std::fs::create_dir_all(&self.output_dir).map_err(|err| {
            AstrbotError::Pipeline(format!(
                "create T2I render output directory {}: {err}",
                self.output_dir.display()
            ))
        })?;
        let path = self.artifact_path(&request);
        std::fs::write(&path, rendered).map_err(|err| {
            AstrbotError::Pipeline(format!(
                "write T2I render artifact {}: {err}",
                path.display()
            ))
        })?;

        Ok(T2iRenderResult {
            artifact: RenderArtifact::file(path, request.options.format),
            template_name: request.options.template_name,
            strategy_used: RenderStrategy::LocalOnly,
        })
    }
}

fn render_template_string(template: &str, data: &BTreeMap<String, Value>) -> String {
    let mut rendered = template.to_string();
    for (key, value) in data {
        let placeholder = format!("{{{{ {key} }}}}");
        let compact_placeholder = format!("{{{{{key}}}}}");
        let value = value
            .as_str()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| value.to_string());
        rendered = rendered.replace(&placeholder, &value);
        rendered = rendered.replace(&compact_placeholder, &value);
    }
    rendered
}

fn escape_template_text(text: &str) -> String {
    text.replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::Value;

    use crate::{
        RenderMode, RenderOptions, RenderStrategy, T2iRenderRequest, T2iRenderer, TemplateCatalog,
        TemplateName, TemplateRenderer,
    };

    #[test]
    fn default_request_escapes_backticks_and_uses_base_template() {
        let request = T2iRenderRequest::from_text("`code`");

        assert_eq!(request.options.template_name.as_str(), "base");
        assert_eq!(
            request.template_data.get("text"),
            Some(&Value::String("\\`code\\`".to_string()))
        );
    }

    #[tokio::test]
    async fn template_renderer_writes_transport_neutral_artifact() {
        let root = std::env::temp_dir().join(format!("astrbot_render_{}", std::process::id()));
        let template_dir = root.join("templates");
        let output_dir = root.join("output");
        let _ = fs::remove_dir_all(&root);

        let catalog = TemplateCatalog::new(&template_dir);
        let template = TemplateName::new("plain").unwrap();
        catalog
            .put_user_template(&template, "hello {{ text }} {{ version }}")
            .unwrap();

        let renderer = TemplateRenderer::new(catalog, &output_dir);
        let request = T2iRenderRequest::from_text("world").with_options(RenderOptions {
            strategy: RenderStrategy::LocalOnly,
            mode: RenderMode::File,
            template_name: template.clone(),
            ..RenderOptions::default()
        });

        let result = renderer.render(request).await.unwrap();

        assert_eq!(result.template_name, template);
        assert_eq!(result.strategy_used, RenderStrategy::LocalOnly);
        assert_eq!(
            fs::read_to_string(result.artifact.value).unwrap(),
            "hello world v0.1.0"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn local_renderer_rejects_network_only_requests() {
        let root = std::env::temp_dir().join(format!(
            "astrbot_render_network_only_{}",
            std::process::id()
        ));
        let renderer = TemplateRenderer::new(TemplateCatalog::without_user_dir(), &root);
        let request = T2iRenderRequest::from_text("hello").with_options(RenderOptions {
            strategy: RenderStrategy::NetworkOnly,
            ..RenderOptions::default()
        });

        assert!(renderer.render(request).await.is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
