#![allow(dead_code)]
use std::collections::HashMap;

use serde::Serialize;

use crate::modules::post::post_types::ContentType;

#[derive(Debug, Clone, Serialize)]
pub struct SeoMeta {
    pub title: String,
    pub description: String,
    pub keywords: String,
    pub canonical_url: String,
    pub og_title: String,
    pub og_description: String,
    pub og_type: String,
    pub og_url: String,
    pub og_image: String,
    pub twitter_card: String,
    pub twitter_title: String,
    pub twitter_description: String,
    pub twitter_image: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonLdNode {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@type")]
    pub kind: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn strip_html_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;

    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }

    output
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 从 HTML 内容中提取纯文本摘要（generate_excerpt 的语义别名）
pub fn extract_description(html_content: &str, fallback: &str) -> String {
    let excerpt = generate_excerpt(html_content, 160);
    if excerpt.is_empty() {
        fallback.to_string()
    } else {
        excerpt
    }
}

pub fn generate_excerpt(input: &str, max_chars: usize) -> String {
    let cleaned = collapse_whitespace(&strip_html_tags(input));
    let mut excerpt = String::new();

    for ch in cleaned.chars() {
        if excerpt.chars().count() >= max_chars {
            break;
        }
        excerpt.push(ch);
    }

    if cleaned.chars().count() > max_chars {
        excerpt.push_str("…");
    }

    excerpt
}

pub fn build_home_meta(
    site_title: &str,
    site_description: &str,
    site_url: &str,
    site_keywords: &str,
    og_image: &str,
) -> SeoMeta {
    let description = if site_description.trim().is_empty() {
        format!("{} - Colophon 博客站点", site_title)
    } else {
        site_description.trim().to_string()
    };

    SeoMeta {
        title: site_title.to_string(),
        description: description.clone(),
        keywords: site_keywords.to_string(),
        canonical_url: site_url.to_string(),
        og_title: site_title.to_string(),
        og_description: description.clone(),
        og_type: "website".to_string(),
        og_url: site_url.to_string(),
        og_image: og_image.to_string(),
        twitter_card: if og_image.is_empty() {
            "summary"
        } else {
            "summary_large_image"
        }
        .to_string(),
        twitter_title: site_title.to_string(),
        twitter_description: description,
        twitter_image: og_image.to_string(),
    }
}

pub fn build_post_meta(
    site_title: &str,
    site_url: &str,
    post_title: &str,
    post_slug: &str,
    post_excerpt: Option<&str>,
    post_html: &str,
    site_keywords: &str,
    og_image: &str,
) -> SeoMeta {
    build_post_meta_with_content_type(
        site_title,
        site_url,
        post_title,
        post_slug,
        post_excerpt,
        post_html,
        site_keywords,
        og_image,
        ContentType::Post,
    )
}

pub fn build_post_meta_with_content_type(
    site_title: &str,
    site_url: &str,
    post_title: &str,
    post_slug: &str,
    post_excerpt: Option<&str>,
    post_html: &str,
    site_keywords: &str,
    og_image: &str,
    content_type: ContentType,
) -> SeoMeta {
    let description = post_excerpt
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| generate_excerpt(post_html, 160));
    let path_prefix = if content_type.is_page() {
        "pages"
    } else {
        "posts"
    };
    let canonical_url = format!(
        "{}/{}/{}",
        site_url.trim_end_matches('/'),
        path_prefix,
        post_slug
    );
    let title = format!("{} - {}", post_title, site_title);

    SeoMeta {
        title,
        description: description.clone(),
        keywords: site_keywords.to_string(),
        canonical_url: canonical_url.clone(),
        og_title: post_title.to_string(),
        og_description: description.clone(),
        og_type: "article".to_string(),
        og_url: canonical_url,
        og_image: og_image.to_string(),
        twitter_card: if og_image.is_empty() {
            "summary"
        } else {
            "summary_large_image"
        }
        .to_string(),
        twitter_title: post_title.to_string(),
        twitter_description: description,
        twitter_image: og_image.to_string(),
    }
}

pub fn build_home_json_ld(site_title: &str, site_description: &str, site_url: &str) -> JsonLdNode {
    let mut extra = HashMap::new();
    extra.insert("name".to_string(), serde_json::json!(site_title));
    extra.insert(
        "description".to_string(),
        serde_json::json!(site_description),
    );
    extra.insert("url".to_string(), serde_json::json!(site_url));

    JsonLdNode {
        context: "https://schema.org".to_string(),
        kind: "WebSite".to_string(),
        extra,
    }
}

pub fn build_post_json_ld(
    site_title: &str,
    site_url: &str,
    post_title: &str,
    post_slug: &str,
    post_excerpt: &str,
    author_name: &str,
    published_at: Option<&str>,
    updated_at: &str,
) -> JsonLdNode {
    build_post_json_ld_with_content_type(
        site_title,
        site_url,
        post_title,
        post_slug,
        post_excerpt,
        author_name,
        published_at,
        updated_at,
        ContentType::Post,
    )
}

pub fn build_post_json_ld_with_content_type(
    site_title: &str,
    site_url: &str,
    post_title: &str,
    post_slug: &str,
    post_excerpt: &str,
    author_name: &str,
    published_at: Option<&str>,
    updated_at: &str,
    content_type: ContentType,
) -> JsonLdNode {
    let mut extra = HashMap::new();
    extra.insert("headline".to_string(), serde_json::json!(post_title));
    extra.insert("description".to_string(), serde_json::json!(post_excerpt));
    let path_prefix = if content_type.is_page() {
        "pages"
    } else {
        "posts"
    };
    extra.insert(
        "mainEntityOfPage".to_string(),
        serde_json::json!(format!(
            "{}/{}/{}",
            site_url.trim_end_matches('/'),
            path_prefix,
            post_slug
        )),
    );
    extra.insert(
        "author".to_string(),
        serde_json::json!({
            "@type": "Person",
            "name": author_name,
        }),
    );
    extra.insert(
        "publisher".to_string(),
        serde_json::json!({
            "@type": "Organization",
            "name": site_title,
        }),
    );
    extra.insert(
        "datePublished".to_string(),
        serde_json::json!(published_at.unwrap_or(updated_at)),
    );
    extra.insert("dateModified".to_string(), serde_json::json!(updated_at));

    JsonLdNode {
        context: "https://schema.org".to_string(),
        kind: "BlogPosting".to_string(),
        extra,
    }
}
