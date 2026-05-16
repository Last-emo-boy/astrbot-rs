use astrbot_runtime::{RuntimeConfigSchema, RuntimeConfigUpdatePreview};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug, Serialize)]
pub struct ManagementConfigSchemaResponse {
    pub schema: RuntimeConfigSchema,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ManagementConfigMutationRequest {
    pub config: Value,
}

pub type ManagementConfigMutationResponse = RuntimeConfigUpdatePreview;

pub async fn schema(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementConfigSchemaResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;

    Ok(Json(ManagementConfigSchemaResponse {
        schema: service.schema(),
    }))
}

pub async fn preview_update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigMutationRequest>,
) -> Result<Json<ManagementConfigMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let preview = service
        .preview_update_value(request.config)
        .map_err(map_config_error)?;

    Ok(Json(preview))
}

pub async fn apply_update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigMutationRequest>,
) -> Result<Json<ManagementConfigMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let preview = service
        .save_update_value(request.config)
        .map_err(map_config_error)?;

    Ok(Json(preview))
}

fn config_service_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "management config service is not configured".to_string(),
        }),
    )
}

fn map_config_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
