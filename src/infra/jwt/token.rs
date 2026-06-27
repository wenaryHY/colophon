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
    pub token_version: i32,
    /// N-1: JWT issuer — 必须与 decode_token 中 set_issuer 的值一致
    pub iss: Option<String>,
}

/// 签发 JWT，不依赖 AppState — 解耦架构：infra 层不应了解 state 层
pub fn issue_token(
    secret: &str,
    token_lifetime_seconds_in_seconds: u64,
    user_id: String,
    username: String,
    role: Role,
    token_version: i32,
) -> Result<String, AppError> {
    // 此处是唯一一处 u64 → i64 转换：JWT exp 字段要求 i64
    let exp = chrono::Utc::now().timestamp() + token_lifetime_seconds_in_seconds as i64;
    let claims = Claims {
        sub: user_id,
        username,
        role,
        exp,
        token_version,
        iss: Some("colophon".into()),
    };

    // 显式指定 HS256 算法，与 decode_token 保持一致
    let mut header = Header::new(jsonwebtoken::Algorithm::HS256);
    header.typ = Some("JWT".to_string());
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|err| AppError::Anyhow(anyhow::anyhow!("failed to issue token: {err}")))
}

/// 验证 JWT，不依赖 AppState
/// 显式指定 HS256 算法，防止算法混淆攻击（alg:none / RS256 公钥混淆）
pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_issuer(&["colophon"]);
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
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
            1,
        )
        .unwrap();
        let claims = decode_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.role, Role::Admin);
        assert_eq!(claims.token_version, 1);
    }

    #[test]
    fn decode_rejects_wrong_secret() {
        let token = issue_token(
            TEST_SECRET,
            3600,
            "user-1".into(),
            "alice".into(),
            Role::Admin,
            1,
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
            1,
        )
        .unwrap();
        let after = chrono::Utc::now().timestamp();
        let claims = decode_token(&token, TEST_SECRET).unwrap();
        assert!(claims.exp >= before + lifetime as i64);
        assert!(claims.exp <= after + lifetime as i64);
    }

    /// H-2: JWT 算法混淆攻击测试 — 攻击者用 HS384 替代 HS256 签名
    /// 当前漏洞：Validation::default() 允许 HS256/HS384/HS512
    /// 期望修复后：仅允许 HS256
    #[test]
    fn security_fix_h2_rejects_hs384_algorithm_confusion() {
        use jsonwebtoken::{Header, encode};
        // 构造一个使用 HS384 的恶意 token
        let mut header = Header::default();
        header.alg = jsonwebtoken::Algorithm::HS384;
        let claims = Claims {
            sub: "attacker".into(),
            username: "evil".into(),
            role: Role::Admin,
            exp: chrono::Utc::now().timestamp() + 3600,
            token_version: 1,
            iss: None,
        };
        let malicious_token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();
        // 期望：decode_token 应拒绝非 HS256 算法
        let result = decode_token(&malicious_token, TEST_SECRET);
        assert!(result.is_err(), "HS384 token should be rejected");
    }

    /// H-2: JWT 算法混淆攻击测试 — HS512
    #[test]
    fn security_fix_h2_rejects_hs512_algorithm_confusion() {
        use jsonwebtoken::{Header, encode};
        let mut header = Header::default();
        header.alg = jsonwebtoken::Algorithm::HS512;
        let claims = Claims {
            sub: "attacker".into(),
            username: "evil".into(),
            role: Role::Admin,
            exp: chrono::Utc::now().timestamp() + 3600,
            token_version: 1,
            iss: None,
        };
        let malicious_token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();
        let result = decode_token(&malicious_token, TEST_SECRET);
        assert!(result.is_err(), "HS512 token should be rejected");
    }

    /// N-1: JWT issuer 校验失效 — issue_token 不设置 iss 字段
    /// 期望：decode_token 拒绝 iss="evil" 的 token
    #[test]
    fn security_fix_n1_decode_rejects_wrong_issuer() {
        use jsonwebtoken::{Header, encode};
        // 构造 iss = "evil" 的恶意 token
        let header = Header::new(jsonwebtoken::Algorithm::HS256);
        #[derive(serde::Serialize)]
        struct EvilClaims {
            sub: String,
            username: String,
            role: Role,
            exp: i64,
            token_version: i32,
            iss: String,
        }
        let claims = EvilClaims {
            sub: "attacker".into(),
            username: "evil".into(),
            role: Role::Admin,
            exp: chrono::Utc::now().timestamp() + 3600,
            token_version: 1,
            iss: "evil".into(),
        };
        let malicious_token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();
        let result = decode_token(&malicious_token, TEST_SECRET);
        assert!(result.is_err(), "token with iss='evil' should be rejected");
    }

    /// N-1: JWT issuer 校验 — iss="colophon" 的 token 应被接受
    #[test]
    fn security_fix_n1_decode_accepts_colophon_issuer() {
        let token = issue_token(
            TEST_SECRET,
            3600,
            "user-1".into(),
            "alice".into(),
            Role::Admin,
            1,
        )
        .unwrap();
        let claims = decode_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(
            claims.iss.as_deref(),
            Some("colophon"),
            "issued token must carry iss='colophon'"
        );
    }

    /// H-2: JWT issuer 校验测试 — 错误的 issuer 应被拒绝
    #[test]
    fn security_fix_h2_rejects_wrong_issuer() {
        // 手动构造 iss = "evil.com" 的 token
        let header = Header::new(jsonwebtoken::Algorithm::HS256);
        #[derive(serde::Serialize)]
        struct MaliciousClaims {
            sub: String,
            username: String,
            role: Role,
            exp: i64,
            token_version: i32,
            iss: String,
        }
        let claims = MaliciousClaims {
            sub: "attacker".into(),
            username: "evil".into(),
            role: Role::Admin,
            exp: chrono::Utc::now().timestamp() + 3600,
            token_version: 1,
            iss: "evil.com".into(),
        };
        let malicious_token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();
        // 期望：decode_token 应拒绝 issuer 不匹配的 token
        let result = decode_token(&malicious_token, TEST_SECRET);
        assert!(result.is_err(), "token with wrong issuer should be rejected");
    }
}
