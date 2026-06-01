use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::shared::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: i64,
}

/// 签发 JWT，不依赖 AppState — 解耦架构：infra 层不应了解 state 层
pub fn issue_token(
    secret: &str,
    token_lifetime_seconds: i64,
    user_id: String,
    username: String,
    role: String,
) -> Result<String, AppError> {
    let exp = chrono::Utc::now().timestamp() + token_lifetime_seconds;
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
