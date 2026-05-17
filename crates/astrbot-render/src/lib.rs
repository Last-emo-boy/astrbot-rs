pub mod font;
pub mod local;
pub mod markdown;
pub mod network;
pub mod t2i;
pub mod template;

pub use font::{
    FontCatalog, FontFamily, FontRequest, FontSelection, FontStyle, TextLayoutLine, TextMeasurer,
};
pub use local::{
    LocalMarkdownRenderer, LocalRasterOptions, LocalRasterPlan, LocalRenderArtifactWriter,
    TemplateRenderer, default_t2i_output_dir,
};
pub use markdown::{InlineSpan, MarkdownBlock, MarkdownDocument};
pub use network::{
    DEFAULT_T2I_ENDPOINT, NetworkT2iClient, NetworkT2iEndpointCatalog, NetworkT2iRenderOutput,
    NetworkT2iRenderer, OfficialT2iEndpointDescriptor, T2iEndpoint,
};
pub use t2i::{
    RenderArtifact, RenderArtifactKind, RenderFormat, RenderMode, RenderOptions, RenderStrategy,
    T2iRenderRequest, T2iRenderResult, T2iRenderer,
};
pub use template::{
    DEFAULT_TEMPLATE_NAME, TemplateCatalog, TemplateDescriptor, TemplateName, TemplateSource,
};
