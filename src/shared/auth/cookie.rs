use crate::shared::auth::constants;

/// 7 天（秒），用于"记住我"场景
pub const REMEMBER_ME_MAX_AGE_SECONDS: u64 = 604800;
/// 15 分钟（秒），用于未勾选"记住我"的短期会话
pub const SHORT_MAX_AGE_SECONDS: u64 = 900;
/// 1 天（秒），注册用户 refresh cookie 默认存活时长
pub const REGISTER_DEFAULT_REFRESH_MAX_AGE_SECONDS: u64 = 86400;

/// 构建 refresh_token 的 HttpOnly Secure SameSite=Strict cookie
pub fn build_refresh_cookie(token: &str, max_age_seconds: u64, cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}={token}; Path=/api/v1/auth/refresh; Max-Age={max_age_seconds}; HttpOnly; SameSite=Strict{secure}",
        name = constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN,
    )
}

/// 清除 refresh_token cookie
pub fn build_clear_refresh_cookie(cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}=; Path=/api/v1/auth/refresh; Max-Age=0; HttpOnly; SameSite=Strict{secure}",
        name = constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN,
    )
}

/// 构建 session cookie（access_token），Path=/
pub fn build_session_cookie(access_token: &str, max_age_seconds: u64, cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}={access_token}; Path=/; Max-Age={max_age_seconds}; HttpOnly; SameSite=Strict{secure}",
        name = constants::SESSION_COOKIE_NAME_FOR_JWT_ACCESS_TOKEN,
    )
}

/// 清除 session cookie（access_token）
pub fn build_clear_session_cookie(cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict{secure}",
        name = constants::SESSION_COOKIE_NAME_FOR_JWT_ACCESS_TOKEN,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_session_cookie_has_http_only() {
        let cookie = build_session_cookie("test_token", 3600, true);
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Path=/"));
    }

    #[test]
    fn build_clear_session_cookie_max_age_zero() {
        let cookie = build_clear_session_cookie(true);
        assert!(cookie.contains("Max-Age=0"));
    }

    #[test]
    fn build_refresh_cookie_non_secure() {
        let cookie = build_refresh_cookie("test", 3600, false);
        assert!(!cookie.contains("Secure"));
    }
}
