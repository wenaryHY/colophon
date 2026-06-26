use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::shared::error::AppError;

/// 异步哈希密码
///
/// 使用 Argon2id 算法对密码进行哈希。
/// 由于 Argon2 是 CPU 密集型操作，使用 `spawn_blocking` 避免阻塞异步运行时。
pub async fn hash_password(password: &str) -> Result<String, AppError> {
    let password = password.to_string();
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = build_argon2();
        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|err| AppError::BadRequest(format!("failed to hash password: {err}")))
    })
    .await
    .map_err(|err| AppError::Anyhow(anyhow::anyhow!("task join error: {err}")))?
}

/// 异步验证密码
///
/// 验证明文密码是否与哈希值匹配。
/// 由于 Argon2 是 CPU 密集型操作，使用 `spawn_blocking` 避免阻塞异步运行时。
pub async fn verify_password(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let password = password.to_string();
    let password_hash = password_hash.to_string();
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&password_hash)
            .map_err(|_| AppError::BadRequest("invalid password hash".into()))?;
        let argon2 = build_argon2();
        Ok(argon2
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|err| AppError::Anyhow(anyhow::anyhow!("task join error: {err}")))?
}

/// 构建使用显式参数的 Argon2 实例（H-4 修复）
///
/// 参数：m=19456 KiB (19 MiB), t=2 iterations, p=1 parallelism
/// 参考：OWASP Password Storage Cheat Sheet (2024)
fn build_argon2<'a>() -> Argon2<'a> {
    use argon2::{Algorithm, Version, Params};
    let params = Params::new(19456, 2, 1, None)
        .expect("static argon2 params are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// H-4: Argon2 使用强化参数 (m=19456)
    #[tokio::test]
    async fn security_fix_h4_argon2_uses_stronger_params() {
        let hash = hash_password("test_password").await.unwrap();
        assert!(
            hash.contains("m=19456"),
            "hash should contain m=19456, got: {}",
            hash
        );
    }

    /// H-4: Argon2 哈希性能应在可接受范围内（<1秒）
    #[tokio::test]
    async fn security_fix_h4_hash_performance_acceptable() {
        let start = std::time::Instant::now();
        let _ = hash_password("test_password").await.unwrap();
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 1000,
            "argon2 hashing should not exceed 1 second, took {}ms",
            elapsed.as_millis()
        );
    }
}
