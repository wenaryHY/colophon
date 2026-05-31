use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};

use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};

use crate::{shared::error::AppError, state::AppState};

const LOGIN_WINDOW: Duration = Duration::from_secs(60);
const MAX_LOGIN_ATTEMPTS: u32 = 8;

pub const SECURITY_PROFILE_HEADER: &str = "x-inkforge-security-profile";
pub const SECURITY_PROFILE_THEME_HTML: &str = "theme-html";
pub const SECURITY_PROFILE_CUSTOM_HTML: &str = "custom-html";
pub const SECURITY_PROFILE_PREVIEW: &str = "preview";

const THEME_HTML_CSP: &str = "default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'self'; form-action 'self'; img-src 'self' data: blob: https:; media-src 'self' data: blob: https:; font-src 'self' data: https://fonts.gstatic.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; script-src 'self' 'unsafe-inline'; connect-src 'self' ws: wss:; frame-src 'none'";
const CUSTOM_HTML_CSP: &str = "default-src 'self' data: blob:; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; img-src 'self' data: blob:; media-src 'self' data: blob:; font-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; connect-src 'none'; frame-src 'none'; worker-src 'self' blob:";
/// 预览页面 CSP — 比主题页面更严格，禁止内联脚本
pub const PREVIEW_CSP: &str = "default-src 'self'; script-src 'none'; object-src 'none'; base-uri 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: https:; media-src 'none'; frame-src 'none';";

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

    fn allow(&mut self, key: String, now: Instant) -> bool {
        self.attempts.retain(|_, window| window.expires_at > now);
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
    headers.insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
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
