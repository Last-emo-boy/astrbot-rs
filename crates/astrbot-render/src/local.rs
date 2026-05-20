use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, Result};
use astrbot_storage::{TempArtifactRoot, safe_artifact_segment};
use async_trait::async_trait;
use image::codecs::{jpeg::JpegEncoder, png::PngEncoder};
use image::{ColorType, ImageBuffer, ImageEncoder, Rgb, Rgba};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::font::{FontCatalog, FontRequest, FontSelection, TextLayoutLine, TextMeasurer};
use crate::markdown::{InlineSpan, MarkdownBlock, MarkdownDocument};
use crate::t2i::{
    RenderArtifact, RenderFormat, RenderMode, RenderStrategy, T2iRenderRequest, T2iRenderResult,
    T2iRenderer, render_template_string,
};
use crate::template::TemplateCatalog;

static NEXT_RENDER_ARTIFACT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRasterOptions {
    pub width: u32,
    pub font_size: u16,
    pub footer_text: String,
}

impl Default for LocalRasterOptions {
    fn default() -> Self {
        Self {
            width: 800,
            font_size: 26,
            footer_text: "Powered by AstrBot v0.1.0".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRasterPlan {
    pub document: MarkdownDocument,
    pub lines: Vec<TextLayoutLine>,
    pub font: FontSelection,
    pub width: u32,
    pub footer_text: String,
}

impl LocalRasterPlan {
    pub fn from_markdown(
        text: &str,
        font_catalog: &FontCatalog,
        measurer: &TextMeasurer,
        options: &LocalRasterOptions,
    ) -> Self {
        let document = MarkdownDocument::parse(text);
        let font = font_catalog.resolve(FontRequest::new(options.font_size));
        let max_width = options.width.saturating_sub(20).max(1);
        let mut lines = Vec::new();
        for text in document_text_fragments(&document) {
            lines.extend(measurer.wrap_text(&text, &font, max_width));
        }

        Self {
            document,
            lines,
            font,
            width: options.width,
            footer_text: options.footer_text.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalRenderArtifactWriter {
    output_dir: PathBuf,
}

impl LocalRenderArtifactWriter {
    pub fn from_output_dir(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    pub fn from_temp_root(root: TempArtifactRoot) -> Self {
        Self {
            output_dir: default_t2i_output_dir_from_root(root),
        }
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    pub fn write(
        &self,
        prefix: impl AsRef<str>,
        format: RenderFormat,
        bytes: impl AsRef<[u8]>,
    ) -> Result<PathBuf> {
        fs::create_dir_all(&self.output_dir).map_err(|err| {
            AstrbotError::Pipeline(format!(
                "create T2I render output directory {}: {err}",
                self.output_dir.display()
            ))
        })?;

        let path = self.next_path(prefix.as_ref(), format);
        fs::write(&path, bytes).map_err(|err| {
            AstrbotError::Pipeline(format!(
                "write T2I render artifact {}: {err}",
                path.display()
            ))
        })?;
        Ok(path)
    }

    fn next_path(&self, prefix: &str, format: RenderFormat) -> PathBuf {
        let prefix = safe_artifact_segment(prefix);
        self.output_dir.join(format!(
            "{}_{}.{}",
            prefix,
            next_render_artifact_id(),
            format.extension()
        ))
    }
}

#[derive(Clone, Debug)]
pub struct TemplateRenderer {
    catalog: TemplateCatalog,
    artifacts: LocalRenderArtifactWriter,
}

impl TemplateRenderer {
    pub fn new(catalog: TemplateCatalog, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            catalog,
            artifacts: LocalRenderArtifactWriter::from_output_dir(output_dir),
        }
    }

    pub fn with_temp_root(catalog: TemplateCatalog, root: TempArtifactRoot) -> Self {
        Self {
            catalog,
            artifacts: LocalRenderArtifactWriter::from_temp_root(root),
        }
    }

    pub fn catalog(&self) -> &TemplateCatalog {
        &self.catalog
    }
}

#[async_trait]
impl T2iRenderer for TemplateRenderer {
    async fn render(&self, request: T2iRenderRequest) -> Result<T2iRenderResult> {
        reject_non_local(&request, "template renderer")?;

        let template = self.catalog.get_template(&request.options.template_name)?;
        let rendered = render_template_string(&template, &request.template_data);
        let path = self.artifacts.write(
            request.options.template_name.as_str(),
            request.options.format,
            rendered,
        )?;

        Ok(T2iRenderResult {
            artifact: RenderArtifact::file(path, request.options.format),
            template_name: request.options.template_name,
            strategy_used: RenderStrategy::LocalOnly,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalRenderEngine {
    HeadlessChromium,
    ServerSideSkia,
    BuiltinRaster,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderEngineDecision {
    pub selected: LocalRenderEngine,
    pub considered: Vec<LocalRenderEngine>,
    pub rationale: String,
}

impl RenderEngineDecision {
    pub fn builtin_raster() -> Self {
        Self {
            selected: LocalRenderEngine::BuiltinRaster,
            considered: vec![
                LocalRenderEngine::HeadlessChromium,
                LocalRenderEngine::ServerSideSkia,
                LocalRenderEngine::BuiltinRaster,
            ],
            rationale: "Use the built-in deterministic raster artifact path until a managed Chromium or Skia backend is configured.".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalTemplateRasterRenderer {
    catalog: TemplateCatalog,
    artifacts: LocalRenderArtifactWriter,
    decision: RenderEngineDecision,
}

impl LocalTemplateRasterRenderer {
    pub fn new(catalog: TemplateCatalog, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            catalog,
            artifacts: LocalRenderArtifactWriter::from_output_dir(output_dir),
            decision: RenderEngineDecision::builtin_raster(),
        }
    }

    pub fn with_temp_root(catalog: TemplateCatalog, root: TempArtifactRoot) -> Self {
        Self {
            catalog,
            artifacts: LocalRenderArtifactWriter::from_temp_root(root),
            decision: RenderEngineDecision::builtin_raster(),
        }
    }

    pub fn decision(&self) -> &RenderEngineDecision {
        &self.decision
    }
}

#[async_trait]
impl T2iRenderer for LocalTemplateRasterRenderer {
    async fn render(&self, request: T2iRenderRequest) -> Result<T2iRenderResult> {
        reject_non_local(&request, "local template raster renderer")?;

        let template = self.catalog.get_template(&request.options.template_name)?;
        let rendered = render_template_string(&template, &request.template_data);
        let bytes = encode_deterministic_raster(
            request.options.format,
            &rendered,
            request.options.quality,
        )?;
        let path = self.artifacts.write(
            request.options.template_name.as_str(),
            request.options.format,
            bytes,
        )?;

        Ok(T2iRenderResult {
            artifact: RenderArtifact::file(path, request.options.format),
            template_name: request.options.template_name,
            strategy_used: RenderStrategy::LocalOnly,
        })
    }
}

#[derive(Clone, Debug)]
pub struct LocalMarkdownRenderer {
    artifacts: LocalRenderArtifactWriter,
    options: LocalRasterOptions,
    fonts: FontCatalog,
    measurer: TextMeasurer,
}

impl LocalMarkdownRenderer {
    pub fn new(root: TempArtifactRoot) -> Self {
        Self {
            artifacts: LocalRenderArtifactWriter::from_temp_root(root),
            options: LocalRasterOptions::default(),
            fonts: FontCatalog::default(),
            measurer: TextMeasurer::default(),
        }
    }

    pub fn with_options(mut self, options: LocalRasterOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_font_catalog(mut self, fonts: FontCatalog) -> Self {
        self.fonts = fonts;
        self
    }

    pub fn plan(&self, text: &str) -> LocalRasterPlan {
        LocalRasterPlan::from_markdown(text, &self.fonts, &self.measurer, &self.options)
    }
}

#[async_trait]
impl T2iRenderer for LocalMarkdownRenderer {
    async fn render(&self, request: T2iRenderRequest) -> Result<T2iRenderResult> {
        reject_non_local(&request, "local markdown renderer")?;

        let plan = self.plan(&request.text);
        let bytes = serde_json::to_vec(&plan).map_err(|err| {
            AstrbotError::Pipeline(format!("serialize local T2I raster plan: {err}"))
        })?;
        let path = self.artifacts.write(
            request.options.template_name.as_str(),
            request.options.format,
            bytes,
        )?;

        Ok(T2iRenderResult {
            artifact: RenderArtifact::file(path, request.options.format),
            template_name: request.options.template_name,
            strategy_used: RenderStrategy::LocalOnly,
        })
    }
}

pub fn default_t2i_output_dir() -> PathBuf {
    default_t2i_output_dir_from_root(TempArtifactRoot::default())
}

fn default_t2i_output_dir_from_root(root: TempArtifactRoot) -> PathBuf {
    root.bucket("render").join("t2i")
}

fn reject_non_local(request: &T2iRenderRequest, label: &str) -> Result<()> {
    if matches!(request.options.mode, RenderMode::Url) {
        return Err(AstrbotError::Pipeline(format!(
            "{label} only supports file artifacts"
        )));
    }
    if matches!(request.options.strategy, RenderStrategy::NetworkOnly) {
        return Err(AstrbotError::Pipeline(format!(
            "{label} cannot satisfy network-only T2I requests"
        )));
    }
    Ok(())
}

fn document_text_fragments(document: &MarkdownDocument) -> Vec<String> {
    document
        .blocks
        .iter()
        .flat_map(|block| match block {
            MarkdownBlock::Paragraph(spans)
            | MarkdownBlock::Quote(spans)
            | MarkdownBlock::ListItem(spans) => spans_text(spans),
            MarkdownBlock::Heading { text, .. } => vec![text.clone()],
            MarkdownBlock::CodeBlock { code, .. } => code.lines().map(str::to_string).collect(),
            MarkdownBlock::Image { alt, url } => vec![format!("{alt} {url}")],
            MarkdownBlock::Blank => Vec::new(),
        })
        .collect()
}

fn spans_text(spans: &[InlineSpan]) -> Vec<String> {
    spans
        .iter()
        .map(|span| match span {
            InlineSpan::Text(text)
            | InlineSpan::Bold(text)
            | InlineSpan::Italic(text)
            | InlineSpan::Strike(text)
            | InlineSpan::Code(text)
            | InlineSpan::Underline(text) => text.clone(),
        })
        .collect()
}

fn next_render_artifact_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_RENDER_ARTIFACT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}_{sequence}")
}

fn encode_deterministic_raster(
    format: RenderFormat,
    payload: &str,
    quality: u8,
) -> Result<Vec<u8>> {
    let text = strip_html_tags(payload);
    let lines = wrap_raster_text(&text, 74);
    let width = 960;
    let height = (180 + lines.len() as u32 * 34).clamp(360, 1_600);
    let mut image = ImageBuffer::from_pixel(width, height, Rgba([248, 250, 252, 255]));

    fill_rect(&mut image, 0, 0, width, 96, Rgba([20, 82, 118, 255]));
    fill_rect(
        &mut image,
        32,
        128,
        width - 64,
        height - 176,
        Rgba([255, 255, 255, 255]),
    );
    stroke_rect(
        &mut image,
        32,
        128,
        width - 64,
        height - 176,
        Rgba([203, 213, 225, 255]),
    );
    draw_text_line(
        &mut image,
        "AstrBot T2I",
        48,
        34,
        3,
        Rgba([255, 255, 255, 255]),
    );

    let mut y = 156;
    for line in lines.iter().take(((height - 220) / 34) as usize) {
        draw_text_line(&mut image, line, 64, y, 2, Rgba([15, 23, 42, 255]));
        y += 34;
    }
    draw_text_line(
        &mut image,
        "Powered by AstrBot",
        48,
        height.saturating_sub(42),
        2,
        Rgba([71, 85, 105, 255]),
    );

    encode_image(format, quality, &image)
}

fn encode_image(
    format: RenderFormat,
    quality: u8,
    image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    match format {
        RenderFormat::Png => PngEncoder::new(&mut bytes)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ColorType::Rgba8.into(),
            )
            .map_err(raster_error)?,
        RenderFormat::Jpeg => {
            let rgb = image
                .pixels()
                .flat_map(|pixel| {
                    let [r, g, b, _] = pixel.0;
                    Rgb([r, g, b]).0
                })
                .collect::<Vec<_>>();
            JpegEncoder::new_with_quality(&mut bytes, quality.clamp(1, 95))
                .encode(&rgb, image.width(), image.height(), ColorType::Rgb8.into())
                .map_err(raster_error)?;
        }
    }
    Ok(bytes)
}

fn strip_html_tags(input: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in input.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn wrap_raster_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len =
            current.chars().count() + usize::from(!current.is_empty()) + word.chars().count();
        if !current.is_empty() && next_len > max_chars {
            lines.push(current);
            current = word.to_string();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(" ".to_string());
    }
    lines
}

fn draw_text_line(
    image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    text: &str,
    x: u32,
    y: u32,
    scale: u32,
    color: Rgba<u8>,
) {
    let mut cursor = x;
    for character in text.chars() {
        draw_glyph(image, character, cursor, y, scale, color);
        cursor = cursor.saturating_add(7 * scale);
        if cursor + 6 * scale >= image.width() {
            break;
        }
    }
}

fn draw_glyph(
    image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    character: char,
    x: u32,
    y: u32,
    scale: u32,
    color: Rgba<u8>,
) {
    if character.is_whitespace() {
        return;
    }
    let seed = character as u32;
    for row in 0..7 {
        for col in 0..5 {
            let bit = seed
                .rotate_left((row * 5 + col) as u32 % 31)
                .wrapping_add(row as u32 * 17)
                .wrapping_add(col as u32 * 31)
                & 1;
            if bit == 1 || row == 0 || row == 6 {
                fill_rect(image, x + col * scale, y + row * scale, scale, scale, color);
            }
        }
    }
}

fn fill_rect(
    image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: Rgba<u8>,
) {
    let max_x = x.saturating_add(width).min(image.width());
    let max_y = y.saturating_add(height).min(image.height());
    for py in y.min(image.height())..max_y {
        for px in x.min(image.width())..max_x {
            image.put_pixel(px, py, color);
        }
    }
}

fn stroke_rect(
    image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: Rgba<u8>,
) {
    fill_rect(image, x, y, width, 1, color);
    fill_rect(
        image,
        x,
        y.saturating_add(height).saturating_sub(1),
        width,
        1,
        color,
    );
    fill_rect(image, x, y, 1, height, color);
    fill_rect(
        image,
        x.saturating_add(width).saturating_sub(1),
        y,
        1,
        height,
        color,
    );
}

fn raster_error(err: image::ImageError) -> AstrbotError {
    AstrbotError::Pipeline(format!("encode local T2I raster image: {err}"))
}

#[allow(dead_code)]
fn _template_data_type_anchor(_: &BTreeMap<String, Value>) {}

#[cfg(test)]
mod tests {
    use std::fs;

    use astrbot_storage::TempArtifactRoot;
    use image::GenericImageView;
    use serde_json::Value;

    use crate::{
        CONVERSATION_RECAP_TEMPLATE_NAME, ERROR_PROMPT_TEMPLATE_NAME, InlineSpan,
        KNOWLEDGE_CARD_TEMPLATE_NAME, RenderArtifactKind, RenderFormat, RenderMode, RenderOptions,
        RenderStrategy, T2iRenderRequest, T2iRenderer, TemplateCatalog, TemplateName,
    };

    use super::{
        LocalMarkdownRenderer, LocalRenderEngine, LocalTemplateRasterRenderer, TemplateRenderer,
        default_t2i_output_dir_from_root,
    };

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
        assert_eq!(result.artifact.kind, RenderArtifactKind::File);
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

    #[tokio::test]
    async fn markdown_renderer_uses_temp_artifact_boundary_and_plan_modules() {
        let root =
            std::env::temp_dir().join(format!("astrbot_render_markdown_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let renderer = LocalMarkdownRenderer::new(TempArtifactRoot::new(&root));
        let request = T2iRenderRequest::from_text("# Title\nhello **bold**");

        let result = renderer.render(request).await.unwrap();

        assert!(result.artifact.value.contains("render"));
        assert!(result.artifact.value.contains("t2i"));
        let payload: Value =
            serde_json::from_slice(&fs::read(result.artifact.value).unwrap()).unwrap();
        assert_eq!(payload["document"]["blocks"][0]["Heading"]["level"], 1);
        assert_eq!(
            payload["document"]["blocks"][1]["Paragraph"][1],
            serde_json::json!({"Bold": "bold"})
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn local_template_raster_renderer_writes_png_and_jpeg_artifacts_for_builtin_templates() {
        let root = std::env::temp_dir().join(format!(
            "astrbot_render_template_raster_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let renderer = LocalTemplateRasterRenderer::new(TemplateCatalog::without_user_dir(), &root);
        assert_eq!(
            renderer.decision().selected,
            LocalRenderEngine::BuiltinRaster
        );

        for (name, format, magic) in [
            (
                CONVERSATION_RECAP_TEMPLATE_NAME,
                RenderFormat::Png,
                b"\x89PNG".as_slice(),
            ),
            (
                KNOWLEDGE_CARD_TEMPLATE_NAME,
                RenderFormat::Jpeg,
                b"\xFF\xD8".as_slice(),
            ),
            (
                ERROR_PROMPT_TEMPLATE_NAME,
                RenderFormat::Png,
                b"\x89PNG".as_slice(),
            ),
        ] {
            let request = T2iRenderRequest::from_text("hello").with_options(RenderOptions {
                strategy: RenderStrategy::LocalOnly,
                mode: RenderMode::File,
                format,
                template_name: TemplateName::new(name).unwrap(),
                ..RenderOptions::default()
            });
            let result = renderer.render(request).await.unwrap();
            let bytes = fs::read(&result.artifact.value).unwrap();
            let decoded = image::load_from_memory(&bytes).expect("raster should decode");

            assert_eq!(result.artifact.kind, RenderArtifactKind::File);
            assert!(bytes.starts_with(magic));
            assert_eq!(decoded.dimensions().0, 960);
            assert!(decoded.dimensions().1 >= 360);
            assert!(result.artifact.value.ends_with(format.extension()));
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn local_plan_keeps_inline_styles_before_raster_output() {
        let renderer = LocalMarkdownRenderer::new(TempArtifactRoot::default());
        let plan = renderer.plan("hello `code`");

        let crate::MarkdownBlock::Paragraph(spans) = &plan.document.blocks[0] else {
            panic!("paragraph expected");
        };
        assert!(spans.contains(&InlineSpan::Code("code".to_string())));
        assert!(!plan.lines.is_empty());
    }

    #[test]
    fn default_output_dir_uses_storage_temp_artifact_root() {
        let dir =
            default_t2i_output_dir_from_root(TempArtifactRoot::from_astrbot_root("workspace"));

        assert_eq!(
            dir,
            std::path::PathBuf::from("workspace/data/temp/render/t2i")
        );
    }
}
