use sqlx::SqlitePool;
use uuid::Uuid;

use super::domain::UserRow;

pub async fn user_count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL")
        .fetch_one(pool)
        .await
}

pub async fn exists_by_username_or_email(
    pool: &SqlitePool,
    username: &str,
    email: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE (username = ? OR email = ?) AND deleted_at IS NULL)",
    )
        .bind(username)
        .bind(email)
        .fetch_one(pool)
        .await
}

pub async fn insert_user(
    pool: &SqlitePool,
    username: &str,
    email: &str,
    password_hash: &str,
    display_name: &str,
    role: &str,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (
            id, username, email, password_hash, display_name, role, status, theme_preference
        ) VALUES (?, ?, ?, ?, ?, ?, 'active', 'system')",
    )
    .bind(&id)
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .bind(display_name)
    .bind(role)
    .execute(pool)
    .await?;

    Ok(id)
}

pub async fn find_by_login(pool: &SqlitePool, login: &str) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT * FROM users WHERE (username = ? OR email = ?) AND deleted_at IS NULL LIMIT 1",
    )
        .bind(login)
        .bind(login)
        .fetch_optional(pool)
        .await
}

pub async fn touch_last_login(pool: &SqlitePool, user_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET last_login_at = datetime('now'), updated_at = datetime('now') WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── Refresh token operations ──

pub async fn save_refresh_token(
    pool: &SqlitePool,
    id: &str,
    user_id: &str,
    token_hash: &str,
    expires_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(id)
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_valid_refresh_token(
    pool: &SqlitePool,
    token_hash: &str,
) -> Result<Option<(String, String)>, sqlx::Error> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT user_id, expires_at FROM refresh_tokens WHERE token_hash = ? AND revoked = 0 AND expires_at > datetime('now') LIMIT 1",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
}

pub async fn revoke_refresh_token(
    pool: &SqlitePool,
    token_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE refresh_tokens SET revoked = 1 WHERE token_hash = ?")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}
