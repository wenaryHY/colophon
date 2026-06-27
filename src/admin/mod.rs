use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use std::{path::PathBuf, sync::Arc};

use crate::state::AppState;

/// Serves /admin and /admin/* paths from the dist directory.
/// index.html lives inside dist/ and is reused as the admin shell entry.
pub async fn admin_static(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
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
        Ok(d) => {
            let mut resp = ([(header::CONTENT_TYPE, mime)], d).into_response();
            // N-3: SVG sandbox — 与 M-4 (theme/handler/public.rs) 保持一致
            apply_svg_sandbox_csp_if_svg(&mut resp);
            resp
        }
        Err(_) => (
            [(header::CONTENT_TYPE, "text/plain")],
            b"404 Not Found".to_vec(),
        )
            .into_response(),
    }
}

/// N-3: 对 SVG 响应添加 Content-Security-Policy: sandbox header
/// 防止浏览器执行 SVG 内嵌的 JavaScript（与 theme/handler/public.rs M-4 一致）
fn apply_svg_sandbox_csp_if_svg(response: &mut Response) {
    let is_svg = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("image/svg+xml"))
        .unwrap_or(false);

    if is_svg {
        response.headers_mut().insert(
            "content-security-policy",
            header::HeaderValue::from_static("sandbox"),
        );
    }
}
