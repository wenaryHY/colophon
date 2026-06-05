use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
};
use std::{path::PathBuf, sync::Arc};

use crate::state::AppState;

/// Serves /admin and /admin/* paths from the dist directory.
/// index.html lives inside dist/ and is reused as the admin shell entry.
pub async fn admin_static(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if path.contains("..") || path.contains('\\') || path.starts_with('/') {
        return (
            [(header::CONTENT_TYPE, "text/plain")],
            b"403 Forbidden".to_vec(),
        )
            .into_response();
    }

    let ext = path.rsplit('.').next().unwrap_or("");
    let mime = match ext {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" => "application/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "ico" => "image/x-icon",
        "map" => "application/json",
        _ => "application/octet-stream",
    };

    let full_path: PathBuf = state.admin_dist_dir.join(&path);

    match tokio::fs::read(&full_path).await {
        Ok(d) => ([(header::CONTENT_TYPE, mime)], d).into_response(),
        Err(_) => (
            [(header::CONTENT_TYPE, "text/plain")],
            b"404 Not Found".to_vec(),
        )
            .into_response(),
    }
}
