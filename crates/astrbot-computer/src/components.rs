use astrbot_tool::{ToolCatalog, ToolDescriptor, ToolSource};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::BooterKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerComponent {
    Shell,
    Python,
    FileSystem,
    Browser,
    SkillLifecycle,
}

impl ComputerComponent {
    pub fn defaults_for(kind: BooterKind) -> Vec<Self> {
        match kind {
            BooterKind::Local => vec![Self::Shell, Self::Python],
            BooterKind::Shipyard | BooterKind::Boxlite | BooterKind::Remote => {
                vec![Self::Shell, Self::Python, Self::FileSystem]
            }
            BooterKind::ShipyardNeo => Self::sandbox_defaults(),
        }
    }

    pub fn sandbox_defaults() -> Vec<Self> {
        vec![
            Self::Shell,
            Self::Python,
            Self::FileSystem,
            Self::Browser,
            Self::SkillLifecycle,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellComponentSpec {
    pub supports_background: bool,
}

impl Default for ShellComponentSpec {
    fn default() -> Self {
        Self {
            supports_background: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonComponentSpec {
    pub supports_images: bool,
    pub kernel_id: Option<String>,
}

impl Default for PythonComponentSpec {
    fn default() -> Self {
        Self {
            supports_images: true,
            kernel_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSystemComponentSpec {
    pub supports_upload: bool,
    pub supports_download: bool,
}

impl Default for FileSystemComponentSpec {
    fn default() -> Self {
        Self {
            supports_upload: true,
            supports_download: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserComponentSpec {
    pub supports_batch: bool,
    pub supports_skill_run: bool,
}

impl Default for BrowserComponentSpec {
    fn default() -> Self {
        Self {
            supports_batch: true,
            supports_skill_run: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputerToolDeclaration {
    pub descriptor: ToolDescriptor,
    pub component: ComputerComponent,
    pub admin_only: bool,
}

impl ComputerToolDeclaration {
    pub fn new(
        component: ComputerComponent,
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            descriptor: ToolDescriptor::new(name)
                .with_description(description)
                .with_parameters(parameters)
                .with_source(ToolSource::Internal),
            component,
            admin_only: true,
        }
    }

    pub fn public(mut self) -> Self {
        self.admin_only = false;
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct ComponentToolCatalog {
    declarations: Vec<ComputerToolDeclaration>,
}

impl ComponentToolCatalog {
    pub fn new(declarations: Vec<ComputerToolDeclaration>) -> Self {
        Self { declarations }
    }

    pub fn for_components(components: &[ComputerComponent]) -> Self {
        Self {
            declarations: component_tool_declarations(components),
        }
    }

    pub fn declarations(&self) -> &[ComputerToolDeclaration] {
        &self.declarations
    }

    pub fn extend_tool_catalog(&self, catalog: &mut ToolCatalog) {
        for declaration in &self.declarations {
            catalog.add_tool(declaration.descriptor.clone());
        }
    }
}

pub fn component_tool_declarations(
    components: &[ComputerComponent],
) -> Vec<ComputerToolDeclaration> {
    let mut declarations = Vec::new();
    let has = |component| components.contains(&component);

    if has(ComputerComponent::Shell) {
        declarations.push(shell_tool());
    }
    if has(ComputerComponent::Python) {
        declarations.push(python_tool());
    }
    if has(ComputerComponent::FileSystem) {
        declarations.push(upload_tool());
        declarations.push(download_tool());
    }
    if has(ComputerComponent::Browser) {
        declarations.push(browser_exec_tool());
        declarations.push(browser_batch_tool());
        declarations.push(browser_skill_tool());
    }
    if has(ComputerComponent::SkillLifecycle) {
        declarations.extend(skill_lifecycle_tools());
    }

    declarations.sort_by(|left, right| left.descriptor.name.cmp(&right.descriptor.name));
    declarations
}

fn shell_tool() -> ComputerToolDeclaration {
    ComputerToolDeclaration::new(
        ComputerComponent::Shell,
        "astrbot_execute_shell",
        "Execute a command in the configured computer-use shell.",
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "background": { "type": "boolean", "default": false },
                "env": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "default": {}
                }
            },
            "required": ["command"]
        }),
    )
}

fn python_tool() -> ComputerToolDeclaration {
    ComputerToolDeclaration::new(
        ComputerComponent::Python,
        "astrbot_execute_ipython",
        "Run code in the configured computer-use Python environment.",
        json!({
            "type": "object",
            "properties": {
                "code": { "type": "string" },
                "silent": { "type": "boolean", "default": false }
            },
            "required": ["code"]
        }),
    )
}

fn upload_tool() -> ComputerToolDeclaration {
    ComputerToolDeclaration::new(
        ComputerComponent::FileSystem,
        "astrbot_upload_file",
        "Upload a local file to the sandbox filesystem.",
        json!({
            "type": "object",
            "properties": {
                "local_path": { "type": "string" }
            },
            "required": ["local_path"]
        }),
    )
}

fn download_tool() -> ComputerToolDeclaration {
    ComputerToolDeclaration::new(
        ComputerComponent::FileSystem,
        "astrbot_download_file",
        "Download a file from the sandbox filesystem.",
        json!({
            "type": "object",
            "properties": {
                "remote_path": { "type": "string" },
                "also_send_to_user": { "type": "boolean", "default": true }
            },
            "required": ["remote_path"]
        }),
    )
}

fn browser_exec_tool() -> ComputerToolDeclaration {
    ComputerToolDeclaration::new(
        ComputerComponent::Browser,
        "astrbot_execute_browser",
        "Execute one browser automation command in the sandbox.",
        json!({
            "type": "object",
            "properties": {
                "cmd": { "type": "string" },
                "timeout": { "type": "integer", "default": 30 },
                "description": { "type": "string" },
                "tags": { "type": "string" },
                "learn": { "type": "boolean", "default": false },
                "include_trace": { "type": "boolean", "default": false }
            },
            "required": ["cmd"]
        }),
    )
}

fn browser_batch_tool() -> ComputerToolDeclaration {
    ComputerToolDeclaration::new(
        ComputerComponent::Browser,
        "astrbot_execute_browser_batch",
        "Execute a browser command batch in the sandbox.",
        json!({
            "type": "object",
            "properties": {
                "commands": { "type": "array", "items": { "type": "string" } },
                "timeout": { "type": "integer", "default": 60 },
                "stop_on_error": { "type": "boolean", "default": true },
                "description": { "type": "string" },
                "tags": { "type": "string" },
                "learn": { "type": "boolean", "default": false },
                "include_trace": { "type": "boolean", "default": false }
            },
            "required": ["commands"]
        }),
    )
}

fn browser_skill_tool() -> ComputerToolDeclaration {
    ComputerToolDeclaration::new(
        ComputerComponent::Browser,
        "astrbot_run_browser_skill",
        "Run a released browser skill in the sandbox by skill key.",
        json!({
            "type": "object",
            "properties": {
                "skill_key": { "type": "string" },
                "timeout": { "type": "integer", "default": 60 },
                "stop_on_error": { "type": "boolean", "default": true },
                "include_trace": { "type": "boolean", "default": false },
                "description": { "type": "string" },
                "tags": { "type": "string" }
            },
            "required": ["skill_key"]
        }),
    )
}

fn skill_lifecycle_tools() -> Vec<ComputerToolDeclaration> {
    [
        (
            "astrbot_get_execution_history",
            "Get execution history from the current sandbox.",
        ),
        (
            "astrbot_annotate_execution",
            "Annotate one execution history record.",
        ),
        (
            "astrbot_create_skill_payload",
            "Create an immutable Neo skill payload.",
        ),
        (
            "astrbot_get_skill_payload",
            "Get one Neo skill payload by reference.",
        ),
        (
            "astrbot_create_skill_candidate",
            "Create a Neo skill candidate from execution evidence.",
        ),
        (
            "astrbot_list_skill_candidates",
            "List Neo skill candidates.",
        ),
        (
            "astrbot_evaluate_skill_candidate",
            "Evaluate a Neo skill candidate.",
        ),
        (
            "astrbot_promote_skill_candidate",
            "Promote a Neo skill candidate to a release.",
        ),
        ("astrbot_list_skill_releases", "List Neo skill releases."),
        (
            "astrbot_rollback_skill_release",
            "Rollback one Neo skill release.",
        ),
        (
            "astrbot_sync_skill_release",
            "Sync a stable Neo skill release to local skill files.",
        ),
    ]
    .into_iter()
    .map(|(name, description)| {
        ComputerToolDeclaration::new(
            ComputerComponent::SkillLifecycle,
            name,
            description,
            json!({
                "type": "object",
                "properties": {}
            }),
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use astrbot_tool::{ToolActivationPolicy, ToolCatalog};

    use super::{ComponentToolCatalog, ComputerComponent, component_tool_declarations};

    #[test]
    fn component_declarations_expose_tools_by_capability() {
        let declarations =
            component_tool_declarations(&[ComputerComponent::Shell, ComputerComponent::Browser]);
        let names = declarations
            .iter()
            .map(|declaration| declaration.descriptor.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"astrbot_execute_shell"));
        assert!(names.contains(&"astrbot_execute_browser"));
        assert!(names.contains(&"astrbot_execute_browser_batch"));
        assert!(!names.contains(&"astrbot_execute_ipython"));
    }

    #[test]
    fn component_tool_catalog_enters_generic_tool_catalog() {
        let bridge = ComponentToolCatalog::for_components(&[ComputerComponent::FileSystem]);
        let mut catalog = ToolCatalog::new();

        bridge.extend_tool_catalog(&mut catalog);

        let tools = catalog.active_tools(&ToolActivationPolicy::new());
        assert_eq!(tools.len(), 2);
        assert!(
            tools
                .iter()
                .all(|tool| tool.source == astrbot_tool::ToolSource::Internal)
        );
    }
}
