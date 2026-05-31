use std::sync::Arc;

use axum::{extract::State, http::header, response::IntoResponse};

use crate::{
    modules::{post::repository as post_repository, setting::repository as setting_repository},
    state::AppState,
};

/// 生成 sitemap.xml 字符串（供 handler 和测试调用）
pub async fn generate_sitemap_xml(state: &AppState) -> Result<String, String> {
    let site_url = setting_repository::get_string(&state.pool, "site_url", "")
        .await
        .map_err(|e| e.to_string())?;
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
pub async fn serve_sitemap(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match generate_sitemap_xml(&state).await {
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
