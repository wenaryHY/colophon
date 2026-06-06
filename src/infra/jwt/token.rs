use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{shared::error::AppError, shared::role::Role};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: Role,
    pub exp: i64,
}

/// 签发 JWT，不依赖 AppState — 解耦架构：infra 层不应了解 state 层
pub fn issue_token(
    secret: &str,
    token_lifetime_seconds_in_seconds: u64,
    user_id: String,
    username: String,
    role: Role,
) -> Result<String, AppError> {
    // 此处是唯一一处 u64 → i64 转换：JWT exp 字段要求 i64
    let exp = chrono::Utc::now().timestamp() + token_lifetime_seconds_in_seconds as i64;
    let claims = Claims {
        sub: user_id,
        username,
        role,
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|err| AppError::Anyhow(anyhow::anyhow!("failed to issue token: {err}")))
}

/// 验证 JWT，不依赖 AppState
pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}

/// 生成 64 字符 hex 随机 refresh token
pub fn generate_refresh_token() -> String {
    let random_bytes: [u8; 32] = rand::random();
    hex::encode(random_bytes)
}

/// 对 token 做 SHA-256 哈希后 hex 编码，用于安全存储
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::role::Role;

    const TEST_SECRET: &str = "test-jwt-secret-for-unit-tests";

    #[test]
    fn issue_and_decode_roundtrip() {
        let token = issue_token(
            TEST_SECRET,
            3600,
            "user-1".into(),
            "alice".into(),
            Role::Admin,
        )
        .unwrap();
        let claims = decode_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.role, Role::Admin);
    }

    #[test]
    fn decode_rejects_wrong_secret() {
        let token = issue_token(
            TEST_SECRET,
            3600,
            "user-1".into(),
            "alice".into(),
            Role::Admin,
        )
        .unwrap();
        assert!(decode_token(&token, "wrong-secret").is_err());
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode_token("not.a.jwt", TEST_SECRET).is_err());
    }

    #[test]
    fn generate_refresh_token_is_64_hex_chars() {
        let token = generate_refresh_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_refresh_token_is_unique() {
        let t1 = generate_refresh_token();
        let t2 = generate_refresh_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn hash_token_is_deterministic() {
        let h1 = hash_token("my_token");
        let h2 = hash_token("my_token");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_token_produces_64_hex_chars() {
        let h = hash_token("some_token");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_token_different_inputs_differ() {
        let h1 = hash_token("token_a");
        let h2 = hash_token("token_b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn issued_token_has_correct_expiry_range() {
        let lifetime = 7200u64;
        let before = chrono::Utc::now().timestamp();
        let token = issue_token(
            TEST_SECRET,
            lifetime,
            "u1".into(),
            "bob".into(),
            Role::Member,
        )
        .unwrap();
        let after = chrono::Utc::now().timestamp();
        let claims = decode_token(&token, TEST_SECRET).unwrap();
        assert!(claims.exp >= before + lifetime as i64);
        assert!(claims.exp <= after + lifetime as i64);
    }
}
