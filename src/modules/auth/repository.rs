use uuid::Uuid;

use super::domain::UserRow;

pub async fn user_count<'e, E>(executor: E) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL")
        .fetch_one(executor)
        .await
}

pub async fn exists_by_username_or_email<'e, E>(
    executor: E,
    username: &str,
    email: &str,
) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE (username = ? OR email = ?) AND deleted_at IS NULL)",
    )
        .bind(username)
        .bind(email)
        .fetch_one(executor)
        .await
}

pub async fn insert_user<'e, E>(
    executor: E,
    username: &str,
    email: &str,
    password_hash: &str,
    display_name: &str,
    role: &str,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
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
    .execute(executor)
    .await?;

    Ok(id)
}

pub async fn find_by_login<'e, E>(executor: E, login: &str) -> Result<Option<UserRow>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        UserRow,
        r#"
        SELECT
            id,
            username,
            email,
            password_hash,
            display_name,
            avatar_media_id,
            bio,
            role,
            status,
            theme_preference,
            created_at,
            updated_at,
            last_login_at
        FROM users
        WHERE (username = ? OR email = ?) AND deleted_at IS NULL
        LIMIT 1
        "#,
        login,
        login
    )
    .fetch_optional(executor)
    .await
}

pub async fn touch_last_login<'e, E>(executor: E, user_id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE users SET last_login_at = datetime('now'), updated_at = datetime('now') WHERE id = ?")
        .bind(user_id)
        .execute(executor)
        .await?;
    Ok(())
}

// ── Refresh token operations ──

pub async fn save_refresh_token<'e, E>(
    executor: E,
    id: &str,
    user_id: &str,
    token_hash: &str,
    expires_at: &str,
    family_id: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, family_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .bind(family_id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_valid_refresh_token<'e, E>(
    executor: E,
    token_hash: &str,
) -> Result<Option<(String, String, Option<String>, Option<String>)>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
        "SELECT user_id, expires_at, family_id, used_at FROM refresh_tokens WHERE token_hash = ? AND revoked = 0 AND expires_at > datetime('now') LIMIT 1",
    )
    .bind(token_hash)
    .fetch_optional(executor)
    .await
}

pub async fn revoke_refresh_token<'e, E>(executor: E, token_hash: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE refresh_tokens SET revoked = 1 WHERE token_hash = ?")
        .bind(token_hash)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn mark_token_used<'e, E>(executor: E, token_hash: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE refresh_tokens SET used_at = datetime('now') WHERE token_hash = ?")
        .bind(token_hash)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn revoke_family<'e, E>(executor: E, family_id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE refresh_tokens SET revoked = 1 WHERE family_id = ?")
        .bind(family_id)
        .execute(executor)
        .await?;
    Ok(())
}
