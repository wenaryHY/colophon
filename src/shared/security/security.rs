use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, request::Parts},
    middleware::Next,
    response::Response,
};

use crate::{shared::error::AppError, state::AppState};

/// M-1: 可信代理 IP 提取器 — 仅当连接来源 IP 是可信代理时，才信任 X-Forwarded-For
#[derive(Clone, Debug)]
pub struct ForwardedIpExtractor {
    /// 可信代理 IP 列表
    pub trusted_proxies: Vec<IpAddr>,
}

impl axum_governor::KeyExtractor for ForwardedIpExtractor {
    type Key = IpAddr;

    fn extract(&self, parts: &Parts) -> Result<axum_governor::KeyOutcome<Self::Key>, axum_governor::ExtractionError> {
        // 获取真实 TCP 对端 IP
        let peer_ip = parts
            .extensions
            .get::<ConnectInfo<std::net::SocketAddr>>()
            .map(|addr| addr.0.ip());

        // M-1: 仅当连接来源是可信代理时，才信任代理头
        if let Some(ip) = peer_ip {
            if self.trusted_proxies.contains(&ip) {
                // 来源是可信代理，尝试从代理头提取真实客户端 IP
                if let Some(client_ip) = forwarded_ip(&parts.headers)
                    .or_else(|| {
                        parts.headers
                            .get("x-real-ip")
                            .and_then(|v| v.to_str().ok())
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                            .map(ToOwned::to_owned)
                    })
                    .and_then(|s| s.parse::<IpAddr>().ok())
                {
                    return Ok(axum_governor::KeyOutcome {
                        key: client_ip,
                        quota_override: None,
                    });
                }
            }
            // 来源不是可信代理，或代理头缺失，使用真实 TCP 对端 IP
            return Ok(axum_governor::KeyOutcome {
                key: ip,
                quota_override: None,
            });
        }

        // 无 ConnectInfo，拒绝限流
        Err(axum_governor::ExtractionError::Other(Box::new(
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "无法提取客户端 IP：ConnectInfo 缺失",
            ),
        )))
    }
}

const LOGIN_WINDOW: Duration = Duration::from_secs(60);
const MAX_LOGIN_ATTEMPTS: u32 = 8;
/// 登录限流器的最大条目数，防止恶意 IP 扫描导致内存无限增长。
const MAX_LOGIN_RATE_LIMIT_ENTRIES: usize = 10000;

pub const SECURITY_PROFILE_HEADER: &str = "x-colophon-security-profile";
pub const SECURITY_PROFILE_THEME_HTML: &str = "theme-html";
pub const SECURITY_PROFILE_CUSTOM_HTML: &str = "custom-html";
pub const SECURITY_PROFILE_PREVIEW: &str = "preview";

// ⚠️ 技术债务 (P2-1)：CSP 包含 'unsafe-inline' 降低 XSS 防护
// 原因：主题系统允许用户上传自定义主题，大量依赖内联脚本
// 迁移路径：未来需实施 CSP nonce 机制，逐步移除 unsafe-inline
// 当前缓解措施：严格的其他 CSP 指令 + SSRF 防护 + 主题沙箱
const THEME_HTML_CSP: &str = "default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'self'; form-action 'self'; upgrade-insecure-requests; img-src 'self' data: blob: https:; media-src 'self' data: blob: https:; font-src 'self' data: https://fonts.gstatic.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; script-src 'self' 'unsafe-inline' https://static.cloudflareinsights.com; connect-src 'self' ws: wss:; frame-src 'none'";
const CUSTOM_HTML_CSP: &str = "default-src 'self' data: blob:; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; upgrade-insecure-requests; img-src 'self' data: blob:; media-src 'self' data: blob:; font-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; connect-src 'none'; frame-src 'none'; worker-src 'self' blob:";
/// 预览页面 CSP — 比主题页面更严格，禁止内联脚本
pub const PREVIEW_CSP: &str = "default-src 'self'; script-src 'none'; object-src 'none'; base-uri 'self'; upgrade-insecure-requests; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: https:; media-src 'none'; frame-src 'none';";

#[derive(Debug, Default)]
pub struct LoginRateLimiter {
    attempts: HashMap<String, AttemptWindow>,
}

#[derive(Debug)]
struct AttemptWindow {
    count: u32,
    expires_at: Instant,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow(&mut self, key: String, now: Instant) -> bool {
        // 淘汰过期条目
        self.attempts.retain(|_, window| window.expires_at > now);

        // M-3: 容量上限保护：超过上限时拒绝新 IP（防止 OOM 攻击）
        if self.attempts.len() >= MAX_LOGIN_RATE_LIMIT_ENTRIES
            && !self.attempts.contains_key(&key)
        {
            tracing::warn!(
                "login rate limiter at capacity ({MAX_LOGIN_RATE_LIMIT_ENTRIES} entries), rejecting new key"
            );
            return false;
        }

        self.attempts
            .entry(key)
            .or_insert_with(|| AttemptWindow::new(now))
            .record(now)
    }
}

impl AttemptWindow {
    fn new(now: Instant) -> Self {
        Self {
            count: 0,
            expires_at: now + LOGIN_WINDOW,
        }
    }

    fn record(&mut self, now: Instant) -> bool {
        if self.expires_at <= now {
            self.count = 0;
            self.expires_at = now + LOGIN_WINDOW;
        }
        self.count += 1;
        self.count <= MAX_LOGIN_ATTEMPTS
    }
}

fn forwarded_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")?
        .to_str()
        .ok()?
        .split(',')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn client_key(headers: &HeaderMap) -> String {
    forwarded_ip(headers)
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn login_rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let key = client_key(request.headers());
    let allowed = {
        let mut limiter = state.login_rate_limiter.lock().await;
        limiter.allow(key.clone(), Instant::now())
    };

    if !allowed {
        tracing::warn!(
            module = "security",
            event = "login_rate_limited",
            client_key = %key,
            "login request blocked by rate limiter"
        );
        return Err(AppError::TooManyRequests(
            "too many login attempts, please retry in a minute".into(),
        ));
    }

    Ok(next.run(request).await)
}

fn csp_for_profile(profile: &str) -> Option<&'static str> {
    match profile {
        SECURITY_PROFILE_THEME_HTML => Some(THEME_HTML_CSP),
        SECURITY_PROFILE_CUSTOM_HTML => Some(CUSTOM_HTML_CSP),
        SECURITY_PROFILE_PREVIEW => Some(PREVIEW_CSP),
        _ => None,
    }
}

pub fn mark_response_security_profile(response: &mut Response, profile: &'static str) {
    response
        .headers_mut()
        .insert(SECURITY_PROFILE_HEADER, HeaderValue::from_static(profile));
}

pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("X-Frame-Options", HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        "Cross-Origin-Resource-Policy",
        HeaderValue::from_static("same-origin"),
    );

    if let Some(profile) = headers
        .remove(SECURITY_PROFILE_HEADER)
        .and_then(|value| value.to_str().ok().map(ToOwned::to_owned))
    {
        if let Some(csp) = csp_for_profile(&profile) {
            headers.insert("Content-Security-Policy", HeaderValue::from_static(csp));
        }
    }

    response
}

/// 记录被限流拦截的请求信息（HTTP 429 响应）
pub async fn log_rate_limited(
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();
    let headers = request.headers().clone();

    let response = next.run(request).await;

    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        let ip = forwarded_ip(&headers)
            .or_else(|| {
                headers
                    .get("x-real-ip")
                    .and_then(|v| v.to_str().ok())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "unknown".into());

        tracing::warn!(
            event = "rate_limited",
            client_ip = %ip,
            method = %method,
            path = %path,
            "请求被限流拦截 (HTTP 429)"
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_governor::KeyExtractor;
    use std::time::{Duration, Instant};

    #[test]
    fn rate_limiter_allows_within_limit() {
        let mut limiter = LoginRateLimiter::new();
        let now = Instant::now();
        for _ in 0..MAX_LOGIN_ATTEMPTS {
            assert!(limiter.allow("ip1".into(), now));
        }
    }

    #[test]
    fn rate_limiter_blocks_after_limit() {
        let mut limiter = LoginRateLimiter::new();
        let now = Instant::now();
        for _ in 0..MAX_LOGIN_ATTEMPTS {
            limiter.allow("ip1".into(), now);
        }
        assert!(!limiter.allow("ip1".into(), now));
    }

    #[test]
    fn rate_limiter_resets_after_window() {
        let mut limiter = LoginRateLimiter::new();
        let now = Instant::now();
        for _ in 0..MAX_LOGIN_ATTEMPTS {
            limiter.allow("ip1".into(), now);
        }
        assert!(!limiter.allow("ip1".into(), now));
        let after_window = now + LOGIN_WINDOW + Duration::from_secs(1);
        assert!(limiter.allow("ip1".into(), after_window));
    }

    #[test]
    fn rate_limiter_tracks_keys_independently() {
        let mut limiter = LoginRateLimiter::new();
        let now = Instant::now();
        for _ in 0..MAX_LOGIN_ATTEMPTS {
            limiter.allow("ip1".into(), now);
        }
        assert!(!limiter.allow("ip1".into(), now));
        assert!(limiter.allow("ip2".into(), now));
    }

    #[test]
    fn forwarded_ip_extracts_first_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());
        assert_eq!(forwarded_ip(&headers), Some("1.2.3.4".to_string()));
    }

    #[test]
    fn forwarded_ip_returns_none_when_missing() {
        let headers = HeaderMap::new();
        assert_eq!(forwarded_ip(&headers), None);
    }

    #[test]
    fn forwarded_ip_ignores_empty_value() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "".parse().unwrap());
        assert_eq!(forwarded_ip(&headers), None);
    }

    #[test]
    fn client_key_prefers_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        headers.insert("x-real-ip", "5.6.7.8".parse().unwrap());
        assert_eq!(client_key(&headers), "1.2.3.4");
    }

    #[test]
    fn client_key_falls_back_to_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "5.6.7.8".parse().unwrap());
        assert_eq!(client_key(&headers), "5.6.7.8");
    }

    #[test]
    fn client_key_returns_unknown_when_no_headers() {
        let headers = HeaderMap::new();
        assert_eq!(client_key(&headers), "unknown");
    }

    #[test]
    fn csp_for_profile_returns_correct_policies() {
        assert!(csp_for_profile(SECURITY_PROFILE_THEME_HTML).is_some());
        assert!(csp_for_profile(SECURITY_PROFILE_CUSTOM_HTML).is_some());
        assert!(csp_for_profile(SECURITY_PROFILE_PREVIEW).is_some());
        assert!(csp_for_profile("nonexistent").is_none());
    }

    #[test]
    fn csp_theme_html_contains_self() {
        let csp = csp_for_profile(SECURITY_PROFILE_THEME_HTML).unwrap();
        assert!(csp.contains("'self'"));
    }

    /// M-1: 验证 ForwardedIpExtractor 拒绝非可信代理的 X-Forwarded-For
    /// 当前漏洞：客户端可伪造 X-Forwarded-For 绕过限流
    /// 期望修复后：仅当来源 IP 是可信代理时才信任代理头
    #[test]
    fn security_fix_m1_forwarded_ip_extractor_uses_peer_ip_for_untrusted_proxy() {
        use axum::extract::ConnectInfo;
        use std::net::SocketAddr;

        let extractor = ForwardedIpExtractor {
            trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
        };

        // 模拟非可信代理（8.8.8.8）发来的请求，带有伪造的 X-Forwarded-For
        let mut request = axum::http::Request::new(());
        request
            .headers_mut()
            .insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new("8.8.8.8".parse().unwrap(), 12345)));

        let (parts, _) = request.into_parts();
        let result = extractor.extract(&parts).unwrap();
        // 期望：使用真实对端 IP（8.8.8.8），而非伪造的 1.2.3.4
        assert_eq!(result.key, "8.8.8.8".parse::<IpAddr>().unwrap());
    }

    /// M-1: 验证可信代理的 X-Forwarded-For 被正确解析
    #[test]
    fn security_fix_m1_forwarded_ip_extractor_trusts_authorized_proxy() {
        use axum::extract::ConnectInfo;
        use std::net::SocketAddr;

        let extractor = ForwardedIpExtractor {
            trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
        };

        // 模拟可信代理（127.0.0.1）发来的请求
        let mut request = axum::http::Request::new(());
        request
            .headers_mut()
            .insert("x-forwarded-for", "10.0.0.1".parse().unwrap());
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new("127.0.0.1".parse().unwrap(), 8080)));

        let (parts, _) = request.into_parts();
        let result = extractor.extract(&parts).unwrap();
        // 期望：使用 X-Forwarded-For 中的客户端 IP
        assert_eq!(result.key, "10.0.0.1".parse::<IpAddr>().unwrap());
    }

    /// M-3: 限流器达到容量上限时应拒绝新 IP，而非降级放行
    /// 当前漏洞：超过 MAX_LOGIN_RATE_LIMIT_ENTRIES 时，新 IP 直接放行
    /// 期望修复后：返回 false（拒绝）
    #[test]
    fn security_fix_m3_rate_limiter_rejects_when_at_capacity() {
        let mut limiter = LoginRateLimiter::new();
        let now = Instant::now();

        // 填满限流器到上限
        for i in 0..MAX_LOGIN_RATE_LIMIT_ENTRIES {
            let key = format!("ip_{}", i);
            limiter.allow(key, now);
        }

        // 新 IP 应被拒绝，而非降级放行
        let result = limiter.allow("new_ip_attacker".into(), now);
        assert!(!result, "rate limiter should reject new IP when at capacity");
    }
}
