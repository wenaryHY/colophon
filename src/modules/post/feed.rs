use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
};

use crate::{
    modules::{
        post::{post_types::ContentType, repository as post_repository},
        seo::sitemap::infer_site_url_from_host_header,
        setting::repository as setting_repository,
    },
    state::AppState,
};

/// GET /rss.xml — 生成 Atom 1.0 feed。
/// 查询最近 20 篇公开文章，返回 Atom 格式 XML。
pub async fn render_atom_feed(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match generate_atom_feed_xml(&state, &headers).await {
        Ok(xml) => (
            [
                (header::CONTENT_TYPE, "application/atom+xml; charset=utf-8"),
                (header::CACHE_CONTROL, "max-age=3600, s-maxage=3600"),
            ],
            xml,
        )
            .into_response(),
        Err(err_msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            err_msg,
        )
            .into_response(),
    }
}

/// 生成 Atom feed XML 字符串（与 handler 分离，方便测试）。
async fn generate_atom_feed_xml(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, String> {
    let posts = post_repository::list_recent_public_posts(&state.pool, 20)
        .await
        .map_err(|e| format!("Failed to fetch posts: {e}"))?;

    let site_url = {
        let cached = state.site_url.read().await.clone();
        let trimmed = cached.trim_end_matches('/');
        if !trimmed.is_empty() {
            trimmed.to_string()
        } else {
            infer_site_url_from_host_header(headers)
        }
    };

    let site_title = setting_repository::get_string(&state.pool, "site_title", "Colophon")
        .await
        .unwrap_or_else(|_| "Colophon".to_string());

    let mut entries = String::new();
    for post in &posts {
        let path_prefix = if post.content_type == ContentType::Page {
            "pages"
        } else {
            "posts"
        };
        let post_url = format!("{}/{}/{}", site_url, path_prefix, post.slug);
        let updated = post
            .published_at
            .as_deref()
            .unwrap_or(&post.updated_at);
        let published = post.published_at.as_deref().unwrap_or(&post.created_at);
        let content = post.excerpt.as_deref().unwrap_or("");
        let author = escape_xml(&post.author_display_name);

        entries.push_str(&format!(
            r#"  <entry>
    <title>{title}</title>
    <link href="{url}" />
    <id>{url}</id>
    <published>{published}</published>
    <updated>{updated}</updated>
    <author><name>{author}</name></author>
    <content type="html">{content}</content>
  </entry>
"#,
            title = escape_xml(&post.title),
            url = escape_xml(&post_url),
            published = escape_xml(published),
            updated = escape_xml(updated),
            content = escape_xml(content),
        ));
    }

    let feed_updated = posts
        .first()
        .and_then(|p| p.published_at.as_deref().or(Some(p.updated_at.as_str())))
        .unwrap_or("");

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>{site_title}</title>
  <link href="{site_url}" />
  <link href="{site_url}/rss.xml" rel="self" />
  <id>{site_url}/</id>
  <updated>{feed_updated}</updated>
{entries}
</feed>"#
    );

    Ok(xml)
}

/// GET /feed → /rss.xml 301 永久重定向。
pub async fn redirect_feed_to_rss() -> Redirect {
    Redirect::permanent("/rss.xml")
}

/// XML 特殊字符转义（&, <, >, ", '）。
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
