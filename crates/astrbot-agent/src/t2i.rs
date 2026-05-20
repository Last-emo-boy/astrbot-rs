use std::fs;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use astrbot_render::{
    RenderArtifactKind, RenderFormat, RenderMode, RenderOptions, RenderStrategy, T2iRenderRequest,
    T2iRenderer, TemplateName,
};
use astrbot_tool::T2I_RENDER_TOOL;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::tool_loop::{
    AgentToolExecutionRequest, AgentToolExecutionResult, AgentToolExecutor, AgentToolOutput,
};

pub struct T2iToolExecutor {
    renderer: Arc<dyn T2iRenderer>,
}

impl T2iToolExecutor {
    pub fn new(renderer: Arc<dyn T2iRenderer>) -> Self {
        Self { renderer }
    }
}

#[async_trait]
impl AgentToolExecutor for T2iToolExecutor {
    async fn execute(
        &self,
        request: AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        if request.descriptor.name != T2I_RENDER_TOOL {
            return Err(AstrbotError::Pipeline(format!(
                "T2I executor cannot handle tool {}",
                request.descriptor.name
            )));
        }

        let prompt = required_string(&request, "prompt")?;
        let template_name = optional_string(&request, "template")
            .map(TemplateName::new)
            .transpose()?
            .unwrap_or_default();
        let format = render_format(&request);
        let render_request = T2iRenderRequest::from_text(prompt).with_options(RenderOptions {
            strategy: RenderStrategy::LocalOnly,
            mode: RenderMode::File,
            format,
            template_name,
            ..RenderOptions::default()
        });
        let render_result = self.renderer.render(render_request).await?;

        let mut result = AgentToolExecutionResult::completed(format!(
            "T2I rendered {} artifact: {}",
            format.extension(),
            render_result.artifact.value
        ));
        if render_result.artifact.kind == RenderArtifactKind::File {
            let bytes = fs::read(&render_result.artifact.value).map_err(|err| {
                AstrbotError::Pipeline(format!(
                    "read T2I rendered artifact {}: {err}",
                    render_result.artifact.value
                ))
            })?;
            result = result.with_output(AgentToolOutput::image(
                STANDARD.encode(bytes),
                mime_type(format),
            ));
        }
        Ok(result)
    }
}

fn required_string(request: &AgentToolExecutionRequest, name: &str) -> Result<String> {
    optional_string(request, name)
        .ok_or_else(|| AstrbotError::Pipeline(format!("{name} must be a non-empty string")))
}

fn optional_string(request: &AgentToolExecutionRequest, name: &str) -> Option<String> {
    request
        .argument(name)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn render_format(request: &AgentToolExecutionRequest) -> RenderFormat {
    match optional_string(request, "format").as_deref() {
        Some("png") => RenderFormat::Png,
        _ => RenderFormat::Jpeg,
    }
}

fn mime_type(format: RenderFormat) -> &'static str {
    match format {
        RenderFormat::Png => "image/png",
        RenderFormat::Jpeg => "image/jpeg",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use astrbot_render::{
        CONVERSATION_RECAP_TEMPLATE_NAME, LocalTemplateRasterRenderer, TemplateCatalog,
    };
    use astrbot_tool::{T2I_RENDER_TOOL, ToolDescriptor};
    use serde_json::Value;

    use super::T2iToolExecutor;
    use crate::{AgentToolExecutor, AgentToolOutput};

    #[tokio::test]
    async fn t2i_tool_executor_returns_text_and_image_output() {
        let root = std::env::temp_dir().join(format!("astrbot_agent_t2i_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let executor = T2iToolExecutor::new(Arc::new(LocalTemplateRasterRenderer::new(
            TemplateCatalog::without_user_dir(),
            &root,
        )));
        let result = executor
            .execute(crate::AgentToolExecutionRequest::new(
                ToolDescriptor::new(T2I_RENDER_TOOL),
                "call-1",
                "session-1",
                args(serde_json::json!({
                    "prompt": "release checklist",
                    "template": CONVERSATION_RECAP_TEMPLATE_NAME,
                    "format": "png"
                })),
                "{}",
            ))
            .await
            .expect("T2I tool should render");

        assert!(result.into_text().contains("T2I rendered png artifact"));
        let image = executor
            .execute(crate::AgentToolExecutionRequest::new(
                ToolDescriptor::new(T2I_RENDER_TOOL),
                "call-2",
                "session-1",
                args(serde_json::json!({
                    "prompt": "release checklist",
                    "format": "png"
                })),
                "{}",
            ))
            .await
            .expect("T2I tool should render image");
        assert!(image.outputs.iter().any(|output| {
            matches!(
                output,
                AgentToolOutput::Image { mime_type, .. } if mime_type == "image/png"
            )
        }));

        let _ = std::fs::remove_dir_all(&root);
    }

    fn args(value: Value) -> BTreeMap<String, Value> {
        value
            .as_object()
            .expect("arguments object")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}
