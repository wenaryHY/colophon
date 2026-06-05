use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
    /// Cloudflare Turnstile token（前端 widget 回调产生的 token，为空则跳过验证）
    #[serde(default)]
    pub turnstile_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub login: String,
    pub password: String,
    pub remember_me: Option<bool>,
    /// Cloudflare Turnstile token（前端 widget 回调产生的 token，为空则跳过验证）
    #[serde(default)]
    pub turnstile_token: Option<String>,
}

/// 登录/注册响应中返回的用户摘要信息
#[derive(Debug, Serialize)]
pub struct AuthUserInfo {
    pub id: String,
    pub username: String,
    pub role: String,
}

/// 登录/注册响应体：用户信息 + access_token
#[derive(Debug, Serialize)]
pub struct LoginResponseData {
    pub user: AuthUserInfo,
    pub access_token: String,
}
