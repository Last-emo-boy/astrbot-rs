use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result as CoreResult};
use astrbot_render::{TemplateCatalog, TemplateName, TemplateSource};
use astrbot_runtime::RuntimeConfigService;
use axum::{
    Json,
    extract::{Path as AxumPath, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementT2iTemplateState {
    catalog: TemplateCatalog,
    active_template: Arc<RwLock<TemplateName>>,
    active_template_path: PathBuf,
}

impl ManagementT2iTemplateState {
    pub fn new(template_dir: impl Into<PathBuf>, active_template_path: impl Into<PathBuf>) -> Self {
        let catalog = TemplateCatalog::new(template_dir);
        let active_template_path = active_template_path.into();
        let active_template = read_active_template(&active_template_path, &catalog);
        Self {
            catalog,
            active_template: Arc::new(RwLock::new(active_template)),
            active_template_path,
        }
    }

    pub fn from_config_service(service: RuntimeConfigService) -> CoreResult<Self> {
        let config = service.read_config()?;
        let layout = config.paths.resolve();
        Ok(Self::new(
            layout.t2i_template_dir,
            active_template_path(service.config_path()),
        ))
    }

    pub fn templates(&self) -> CoreResult<Vec<ManagementT2iTemplateDescriptor>> {
        let active = self.active_template()?;
        self.catalog
            .list_templates()?
            .into_iter()
            .map(|entry| {
                let name = entry.name.as_str().to_string();
                Ok(ManagementT2iTemplateDescriptor {
                    active: active.as_str() == name,
                    is_default: entry.is_default,
                    name,
                    source: match entry.source {
                        TemplateSource::Builtin => "builtin".to_string(),
                        TemplateSource::User => "user".to_string(),
                    },
                })
            })
            .collect()
    }

    pub fn active_template(&self) -> CoreResult<TemplateName> {
        self.active_template
            .read()
            .map_err(|error| AstrbotError::Pipeline(error.to_string()))
            .map(|template| template.clone())
    }

    pub fn get_template(&self, name: &str) -> CoreResult<String> {
        let name = TemplateName::new(name.to_string())?;
        self.catalog.get_template(&name)
    }

    pub fn create_template(&self, name: String, content: String) -> CoreResult<String> {
        let name = normalized_template_name(name)?;
        if content.is_empty() {
            return Err(AstrbotError::Pipeline(
                "Name and content are required.".to_string(),
            ));
        }
        let template = TemplateName::new(name.clone())?;
        if self.catalog.get_template(&template).is_ok() {
            return Err(AstrbotError::Pipeline(
                "Template with this name already exists.".to_string(),
            ));
        }
        self.catalog.put_user_template(&template, &content)?;
        Ok(name)
    }

    pub fn update_template(&self, name: String, content: String) -> CoreResult<String> {
        let name = normalized_template_name(name)?;
        let template = TemplateName::new(name.clone())?;
        self.catalog.get_template(&template)?;
        self.catalog.put_user_template(&template, &content)?;
        Ok(name)
    }

    pub fn delete_template(&self, name: String) -> CoreResult<bool> {
        let name = normalized_template_name(name)?;
        if name == astrbot_render::DEFAULT_TEMPLATE_NAME {
            return Err(AstrbotError::Pipeline(
                "Default template cannot be deleted.".to_string(),
            ));
        }
        let template = TemplateName::new(name.clone())?;
        self.catalog.delete_user_template(&template)?;
        if self.active_template()?.as_str() == name {
            self.set_active_template(astrbot_render::DEFAULT_TEMPLATE_NAME.to_string())?;
        }
        Ok(true)
    }

    pub fn set_active_template(&self, name: String) -> CoreResult<String> {
        let name = normalized_template_name(name)?;
        let template = TemplateName::new(name.clone())?;
        self.catalog.get_template(&template)?;
        {
            let mut active = self
                .active_template
                .write()
                .map_err(|error| AstrbotError::Pipeline(error.to_string()))?;
            *active = template;
        }
        self.persist_active_template(&name)?;
        Ok(name)
    }

    pub fn reset_default_template(&self) -> CoreResult<()> {
        let base = TemplateName::base();
        match self.catalog.delete_user_template(&base) {
            Ok(()) => {}
            Err(error) if error.to_string().contains("missing T2I user template") => {}
            Err(error) => return Err(error),
        }
        self.set_active_template(astrbot_render::DEFAULT_TEMPLATE_NAME.to_string())?;
        Ok(())
    }

    fn persist_active_template(&self, name: &str) -> CoreResult<()> {
        if let Some(parent) = self.active_template_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AstrbotError::Pipeline(format!(
                    "create T2I active template state dir {}: {error}",
                    parent.display()
                ))
            })?;
        }
        fs::write(&self.active_template_path, name).map_err(|error| {
            AstrbotError::Pipeline(format!(
                "write T2I active template state {}: {error}",
                self.active_template_path.display()
            ))
        })
    }
}

impl std::fmt::Debug for ManagementT2iTemplateState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementT2iTemplateState")
            .field("active_template_path", &self.active_template_path)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementT2iTemplateDescriptor {
    pub name: String,
    pub source: String,
    pub is_default: bool,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementT2iTemplateCreateRequest {
    pub name: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementT2iTemplateUpdateRequest {
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementT2iTemplateSetActiveRequest {
    pub name: String,
}

pub async fn list_templates(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let templates = t2i_state(&state)?.templates().map_err(map_t2i_error)?;
    Ok(legacy_ok(templates, ""))
}

pub async fn active_template(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let active = t2i_state(&state)?
        .active_template()
        .map_err(map_t2i_error)?;
    Ok(legacy_ok(json!({ "active_template": active.as_str() }), ""))
}

pub async fn get_template(
    State(state): State<ManagementApiState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let content = t2i_state(&state)?
        .get_template(&name)
        .map_err(map_t2i_error)?;
    Ok(legacy_ok(json!({ "name": name, "content": content }), ""))
}

pub async fn create_template(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementT2iTemplateCreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<ErrorResponse>)> {
    let name = t2i_state(&state)?
        .create_template(request.name, request.content)
        .map_err(map_t2i_error)?;
    Ok((
        StatusCode::CREATED,
        legacy_ok_value(json!({ "name": name }), "Template created successfully."),
    ))
}

pub async fn update_template(
    State(state): State<ManagementApiState>,
    AxumPath(name): AxumPath<String>,
    Json(request): Json<ManagementT2iTemplateUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let name = t2i_state(&state)?
        .update_template(name, request.content)
        .map_err(map_t2i_error)?;
    let message = format!("模板 '{name}' 已更新。");
    Ok(legacy_ok(json!({ "name": name }), message))
}

pub async fn delete_template(
    State(state): State<ManagementApiState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    t2i_state(&state)?
        .delete_template(name)
        .map_err(map_t2i_error)?;
    Ok(legacy_ok(json!({}), "Template deleted successfully."))
}

pub async fn set_active_template(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementT2iTemplateSetActiveRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let name = t2i_state(&state)?
        .set_active_template(request.name)
        .map_err(map_t2i_error)?;
    Ok(legacy_ok(
        json!({ "active_template": name }),
        format!("模板 '{name}' 已成功应用。"),
    ))
}

pub async fn reset_default_template(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    t2i_state(&state)?
        .reset_default_template()
        .map_err(map_t2i_error)?;
    Ok(legacy_ok(
        json!({ "active_template": astrbot_render::DEFAULT_TEMPLATE_NAME }),
        "Default template has been reset and activated.",
    ))
}

fn t2i_state(
    state: &ManagementApiState,
) -> Result<&ManagementT2iTemplateState, (StatusCode, Json<ErrorResponse>)> {
    state.t2i_templates().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "T2I template management is not configured".to_string(),
            }),
        )
    })
}

fn legacy_ok(data: impl Serialize, message: impl Into<String>) -> Json<Value> {
    legacy_ok_value(json!(data), message)
}

fn legacy_ok_value(data: Value, message: impl Into<String>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": message.into(),
        "data": data,
    }))
}

fn map_t2i_error(error: AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    let message = error.to_string();
    let status = if message.contains("already exists") {
        StatusCode::CONFLICT
    } else if message.contains("missing T2I template") || message.contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(ErrorResponse { error: message }))
}

fn normalized_template_name(name: String) -> CoreResult<String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AstrbotError::Pipeline(
            "Name and content are required.".to_string(),
        ));
    }
    Ok(name)
}

fn read_active_template(path: &Path, catalog: &TemplateCatalog) -> TemplateName {
    let active = fs::read_to_string(path)
        .ok()
        .and_then(|value| TemplateName::new(value.trim().to_string()).ok())
        .unwrap_or_else(TemplateName::base);
    if catalog.get_template(&active).is_ok() {
        active
    } else {
        TemplateName::base()
    }
}

fn active_template_path(config_path: &Path) -> PathBuf {
    let parent = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = config_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("runtime-config");
    parent.join(format!("{stem}.t2i_active_template"))
}
