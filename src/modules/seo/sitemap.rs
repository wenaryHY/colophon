use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::IntoResponse,
};

use crate::{
    modules::{post::repository as post_repository, setting::repository as setting_repository},
    state::AppState,
};

/// 从 Host header 推断 site_url，用于数据库 site_url 为空时的兜底
pub fn infer_site_url_from_host_header(headers: &HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let scheme = if host.starts_with("localhost") || host.starts_with("127.") {
        "http"
    } else {
        "https"
    };
    format!("{}://{}", scheme, host)
}

/// 生成 sitemap.xml 字符串（供 handler 和测试调用）
pub async fn generate_sitemap_xml(state: &AppState) -> Result<String, String> {
    let site_url = setting_repository::get_string(&state.pool, "site_url", "")
        .await
        .map_err(|e| e.to_string())?;
    // 如果 site_url 为空，在 Handler 层传入 fallback，这里仍保留测试兼容
    build_sitemap_xml_inner(site_url.trim_end_matches('/'), state).await
}

/// 生成 sitemap.xml 字符串，支持 fallback site_url 兜底
pub async fn generate_sitemap_xml_with_fallback(
    state: &AppState,
    fallback_site_url: &str,
) -> Result<String, String> {
    let site_url = setting_repository::get_string(&state.pool, "site_url", fallback_site_url)
        .await
        .map_err(|e| e.to_string())?;
    build_sitemap_xml_inner(site_url.trim_end_matches('/'), state).await
}

async fn build_sitemap_xml_inner(site_url: &str, state: &AppState) -> Result<String, String> {
    let posts = post_repository::list_for_sitemap(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
        xmlns:xhtml="http://www.w3.org/1999/xhtml">
"#,
    );

    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
    xml.push_str(&format!(
        r#"  <url>
    <loc>{site_url}/</loc>
    <lastmod>{now}</lastmod>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
  </url>
"#,
    ));

    for post in posts {
        let path_prefix = if post.content_type == "page" { "pages" } else { "posts" };
        let post_url = format!("{site_url}/{path_prefix}/{}", post.slug);
        let lastmod = &post.updated_at[..10];
        xml.push_str(&format!(
            r#"  <url>
    <loc>{post_url}</loc>
    <lastmod>{lastmod}</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.8</priority>
  </url>
"#,
        ));
    }

    xml.push_str("</urlset>");
    Ok(xml)
}

/// Handler for GET /sitemap.xml
pub async fn serve_sitemap(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let fallback_site_url = infer_site_url_from_host_header(&headers);
    match generate_sitemap_xml_with_fallback(&state, &fallback_site_url).await {
        Ok(xml) => (
            [
                (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
                (header::CACHE_CONTROL, "max-age=3600, s-maxage=3600"),
            ],
            xml,
        ),
        Err(_) => (
            [
                (header::CONTENT_TYPE, "text/plain"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            String::from("500 Internal Server Error"),
        ),
    }
}
