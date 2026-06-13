use axum::{
    extract::{Request, State},
    http::{header, HeaderMap},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::state::AppState;

/// 默认语言（与数据库 users.language 默认值保持一致）
pub const DEFAULT_LANG: &str = "zh";
/// 支持的语言代码（与数据库 CHECK 约束保持一致）
const SUPPORTED_LANGS: &[&str] = &["zh", "en"];

/// 从请求中提取语言偏好，优先级：cookie > Accept-Language > 默认 zh
///
/// 中间件层只做一件事：把语言写入 request extensions，供后续 handler 读取。
/// 不修改请求其他部分。
pub async fn inject_language(
    State(_state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let lang = resolve_language_from_headers(req.headers());
    req.extensions_mut().insert(CurrentLanguage(lang));
    next.run(req).await
}

/// 公共辅助：直接从 HeaderMap 解析语言
///
/// 现有 handler 大多已经提取 `HeaderMap`，可直接调用本函数获取当前语言，
/// 避免改动 handler 签名。
///
/// 优先级：cookie > Accept-Language > 默认 zh
pub fn resolve_language_from_headers(headers: &HeaderMap) -> String {
    let raw = extract_lang_from_cookie(headers)
        .or_else(|| extract_lang_from_accept_language(headers))
        .unwrap_or_else(|| DEFAULT_LANG.to_string());
    normalize_lang(&raw).to_string()
}

/// 从 request extensions 读取当前语言（由中间件注入）。
/// 如果中间件未生效或未注入，回退到默认 zh。
pub fn current_lang_from_extensions(req: &Request) -> String {
    req.extensions()
        .get::<CurrentLanguage>()
        .map(|l| l.0.clone())
        .unwrap_or_else(|| DEFAULT_LANG.to_string())
}

/// 把任意原始语言字符串归一化为支持的语言代码（zh / en）。
/// - "en" / "en-US" / "en-GB" → "en"
/// - 其他一律落回 zh（包括 "zh"、"zh-CN"、未知值）
fn normalize_lang(raw: &str) -> &'static str {
    let lower = raw.trim().to_ascii_lowercase();
    if lower == "en" || lower.starts_with("en-") {
        "en"
    } else if lower == "zh" || lower.starts_with("zh-") {
        "zh"
    } else {
        // 未知语言：默认 zh，但保险起见再校验一次是否在白名单
        if SUPPORTED_LANGS.contains(&lower.as_str()) {
            // SAFETY: SUPPORTED_LANGS 元素是 'static
            SUPPORTED_LANGS
                .iter()
                .find(|s| **s == lower)
                .copied()
                .unwrap_or(DEFAULT_LANG)
        } else {
            DEFAULT_LANG
        }
    }
}

/// 从 Cookie 头中提取 lang 值
fn extract_lang_from_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in cookie_header.split(';') {
        let pair = pair.trim();
        if let Some(value) = pair.strip_prefix("lang=") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// 从 Accept-Language 头提取首个语言代码
///
/// Accept-Language 格式：zh-CN,zh;q=0.9,en;q=0.8
/// 取第一项的主语言（第一个 `-` 之前部分）。
fn extract_lang_from_accept_language(headers: &HeaderMap) -> Option<String> {
    let accept_lang = headers.get(header::ACCEPT_LANGUAGE)?.to_str().ok()?;
    let first_lang = accept_lang.split(',').next()?.trim();
    let lang_code = first_lang.split(';').next()?.trim();
    if lang_code.is_empty() {
        return None;
    }
    Some(lang_code.to_string())
}

/// 构造 Set-Cookie 值，用于 API 更新语言偏好后写回浏览器。
///
/// - `Max-Age=31536000`：365 天
/// - `Path=/`：全站可用
/// - `SameSite=Lax`：避免跨站 CSRF
/// - 不带 `Secure`：开发期 HTTP 也可写入；生产环境由反向代理或后续配置叠加
pub fn build_lang_cookie(lang: &str) -> String {
    // 防御：再做一次归一化，杜绝把非法值塞进 Set-Cookie
    let safe = normalize_lang(lang);
    format!("lang={}; Max-Age=31536000; Path=/; SameSite=Lax", safe)
}

/// 扩展类型：存储当前请求的语言（中间件注入到 request extensions）
#[derive(Clone, Debug)]
pub struct CurrentLanguage(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers_with(name: axum::http::HeaderName, value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(name, HeaderValue::from_str(value).unwrap());
        h
    }

    #[test]
    fn cookie_takes_priority_over_accept_language() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("lang=en; other=x"));
        h.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.9"),
        );
        assert_eq!(resolve_language_from_headers(&h), "en");
    }

    #[test]
    fn accept_language_used_when_no_cookie() {
        let h = headers_with(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9");
        assert_eq!(resolve_language_from_headers(&h), "en");
    }

    #[test]
    fn defaults_to_zh_when_nothing_present() {
        let h = HeaderMap::new();
        assert_eq!(resolve_language_from_headers(&h), "zh");
    }

    #[test]
    fn unknown_language_falls_back_to_zh() {
        let h = headers_with(header::ACCEPT_LANGUAGE, "fr-FR,fr;q=0.9");
        assert_eq!(resolve_language_from_headers(&h), "zh");
    }

    #[test]
    fn zh_variants_normalize_to_zh() {
        let h = headers_with(header::ACCEPT_LANGUAGE, "zh-Hant-TW");
        assert_eq!(resolve_language_from_headers(&h), "zh");
    }

    #[test]
    fn empty_cookie_value_does_not_override() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("lang=; other=x"));
        h.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("en"));
        assert_eq!(resolve_language_from_headers(&h), "en");
    }

    #[test]
    fn build_lang_cookie_normalizes_invalid_input() {
        // 非法语言不应塞进 Set-Cookie
        let cookie = build_lang_cookie("ja-JP");
        assert!(cookie.starts_with("lang=zh;"));
        let cookie = build_lang_cookie("EN-us");
        assert!(cookie.starts_with("lang=en;"));
    }
}
