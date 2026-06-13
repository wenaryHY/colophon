use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::IntoResponse,
};

use crate::{
    modules::{
        post::{post_types::ContentType, repository as post_repository},
        setting::repository as setting_repository,
    },
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

/// 生成 hreflang 多语言标记（符合 Google SEO 最佳实践）
///
/// 注意：当前未实施 URL 前缀路由（/en/posts/xxx 不存在），
/// 所以三个 hreflang 都指向同一 URL。这符合 Google 文档的
/// "通过 cookie/header 切换语言"模式。
/// 未来如果实施 URL 前缀，再修改为 /zh/xxx 和 /en/xxx。
fn generate_hreflang_links(url: &str) -> String {
    format!(
        r#"    <xhtml:link rel="alternate" hreflang="zh" href="{url}"/>
    <xhtml:link rel="alternate" hreflang="en" href="{url}"/>
    <xhtml:link rel="alternate" hreflang="x-default" href="{url}"/>"#
    )
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

    // 首页
    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let home_url = format!("{site_url}/");
    xml.push_str(&format!(
        r#"  <url>
    <loc>{home_url}</loc>
    <lastmod>{now}</lastmod>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
{hreflang}
  </url>
"#,
        hreflang = generate_hreflang_links(&home_url)
    ));

    // 文章和页面
    for post in posts {
        let path_prefix = if post.content_type == ContentType::Page {
            "pages"
        } else {
            "posts"
        };
        let post_url = format!("{site_url}/{path_prefix}/{}", post.slug);
        let lastmod = &post.updated_at[..10];
        xml.push_str(&format!(
            r#"  <url>
    <loc>{post_url}</loc>
    <lastmod>{lastmod}</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.8</priority>
{hreflang}
  </url>
"#,
            hreflang = generate_hreflang_links(&post_url)
        ));
    }

    xml.push_str("</urlset>");
    Ok(xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_localhost_uses_http() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "localhost:3000".parse().unwrap());
        assert_eq!(
            infer_site_url_from_host_header(&headers),
            "http://localhost:3000"
        );
    }

    #[test]
    fn infer_loopback_uses_http() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(
            infer_site_url_from_host_header(&headers),
            "http://127.0.0.1:8080"
        );
    }

    #[test]
    fn infer_public_domain_uses_https() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "example.com".parse().unwrap());
        assert_eq!(
            infer_site_url_from_host_header(&headers),
            "https://example.com"
        );
    }

    #[test]
    fn infer_missing_host_defaults_to_localhost() {
        let headers = HeaderMap::new();
        assert_eq!(
            infer_site_url_from_host_header(&headers),
            "http://localhost"
        );
    }

    #[test]
    fn hreflang_includes_zh_en_and_x_default() {
        let url = "https://example.com/posts/hello-world";
        let xml = generate_hreflang_links(url);
        assert!(xml.contains(r#"hreflang="zh""#));
        assert!(xml.contains(r#"hreflang="en""#));
        assert!(xml.contains(r#"hreflang="x-default""#));
    }

    #[test]
    fn hreflang_all_variants_point_to_same_url_when_no_url_prefix_routing() {
        let url = "https://example.com/posts/hello-world";
        let xml = generate_hreflang_links(url);
        // 当前无 URL 前缀路由，三个变体都指向同一 URL
        let occurrences = xml.matches(url).count();
        assert_eq!(
            occurrences, 3,
            "expected url to appear 3 times in hreflang block"
        );
    }

    #[test]
    fn hreflang_uses_xhtml_namespace_prefix() {
        let url = "https://example.com/";
        let xml = generate_hreflang_links(url);
        // 必须使用 xhtml: 前缀以匹配 <urlset> 中声明的命名空间
        assert!(xml.contains("<xhtml:link"));
        assert!(xml.contains(r#"rel="alternate""#));
    }
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
