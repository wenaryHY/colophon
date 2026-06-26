use axum::http::HeaderMap;
use serde::Deserialize;

use crate::shared::error::{AppError, AppResult};

#[derive(Debug, Deserialize)]
pub struct TurnstileResponse {
    pub success: bool,
    #[serde(rename = "error-codes")]
    #[allow(dead_code)]
    pub error_codes: Option<Vec<String>>,
}

pub async fn verify_turnstile(token: &str, secret: &str) -> bool {
    let client = reqwest::Client::new();
    let res = client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&[("secret", secret), ("response", token)])
        .send()
        .await;

    match res {
        Ok(r) => r
            .json::<TurnstileResponse>()
            .await
            .map(|r| r.success)
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// 从请求头提取 Turnstile token 并验证
/// - 未配置 secret 时直接放行（向下兼容）
/// - token 缺失或验证失败返回 AppError
pub async fn verify_turnstile_from_request(
    headers: &HeaderMap,
    secret: &str,
    site_key: &str,
    event_name: &str,
) -> AppResult<()> {
    // 未配置 Turnstile 时直接跳过
    if secret.is_empty() {
        return Ok(());
    }

    let token = headers
        .get("x-turnstile-token")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("请完成人机验证".into()))?;

    if token.is_empty() {
        return Err(AppError::BadRequest("Turnstile 配置不完整".into()));
    }

    if !verify_turnstile(token, secret).await {
        tracing::warn!(
            event = event_name,
            turnstile_site_key = %site_key,
            "turnstile verification failed"
        );
        return Err(AppError::BadRequest("人机验证失败，请重试".into()));
    }
    Ok(())
}
