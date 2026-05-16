use astrbot_core::AstrbotError;
use axum::{Json, http::StatusCode};

use crate::ErrorResponse;

pub(crate) fn map_submit_error(error: AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match error {
        AstrbotError::EmptyMessage => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

pub(crate) fn map_storage_error(error: AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
