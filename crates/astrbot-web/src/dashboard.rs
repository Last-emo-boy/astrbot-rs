use std::fs;

use astrbot_runtime::DashboardAssetSelection;
use axum::http::Uri;
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::get,
};

#[derive(Clone, Debug)]
struct DashboardStaticState {
    assets: DashboardAssetSelection,
}

pub fn dashboard_static_router(assets: DashboardAssetSelection) -> Router {
    Router::new()
        .route("/", get(index))
        .fallback(get(asset))
        .with_state(DashboardStaticState { assets })
}

async fn index(State(state): State<DashboardStaticState>) -> Response {
    serve_asset(&state, "/")
}

async fn asset(uri: Uri, State(state): State<DashboardStaticState>) -> Response {
    serve_asset(&state, uri.path())
}

fn serve_asset(state: &DashboardStaticState, request_path: &str) -> Response {
    if !state.assets.webui_enabled {
        return (StatusCode::OK, "WebUI is disabled.").into_response();
    }

    let Some(path) = state.assets.asset_path(request_path) else {
        return (StatusCode::FORBIDDEN, "invalid dashboard asset path").into_response();
    };

    match fs::read(&path) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, content_type_for(&path))
            .body(Body::from(bytes))
            .unwrap_or_else(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("dashboard asset response: {error}"),
                )
                    .into_response()
            }),
        Err(error) if request_path.starts_with("/api/") => (
            StatusCode::NOT_FOUND,
            format!("dashboard API route not found: {error}"),
        )
            .into_response(),
        Err(error) => (
            StatusCode::NOT_FOUND,
            format!("dashboard asset not found: {error}"),
        )
            .into_response(),
    }
}

fn content_type_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("gif") => "image/gif",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use astrbot_runtime::{DashboardAssetSelection, DashboardAssetSource};
    use axum::{
        body::Body,
        http::{Request, StatusCode, header::CONTENT_TYPE},
    };
    use tower::ServiceExt;

    use super::dashboard_static_router;

    #[tokio::test]
    async fn dashboard_static_router_serves_brand_assets_with_content_types() {
        let root =
            std::env::temp_dir().join(format!("astrbot-dashboard-static-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("assets/images")).expect("asset directory should create");
        fs::write(root.join("index.html"), "<main>AstrBot</main>").expect("index should write");
        fs::write(root.join("styles.css"), "body { color: #111827; }").expect("css should write");
        fs::write(root.join("app.js"), "console.log('astrbot')").expect("js should write");
        fs::write(
            root.join("assets/images/astrbot_logo_mini.webp"),
            [0x52, 0x49, 0x46, 0x46],
        )
        .expect("webp should write");
        fs::write(root.join("assets/images/icon.svg"), "<svg></svg>").expect("svg should write");

        let router = dashboard_static_router(DashboardAssetSelection {
            source: DashboardAssetSource::Explicit,
            root_dir: root.clone(),
            webui_enabled: true,
        });

        for (path, expected) in [
            ("/about", "text/html; charset=utf-8"),
            ("/styles.css", "text/css; charset=utf-8"),
            ("/app.js", "text/javascript; charset=utf-8"),
            ("/assets/images/astrbot_logo_mini.webp", "image/webp"),
            ("/assets/images/icon.svg", "image/svg+xml"),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("router should respond");
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(
                response
                    .headers()
                    .get(CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok()),
                Some(expected),
                "{path}"
            );
        }

        let _ = fs::remove_dir_all(root);
    }
}
