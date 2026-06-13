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
        Argon2::default()
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
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|err| AppError::Anyhow(anyhow::anyhow!("task join error: {err}")))?
}
