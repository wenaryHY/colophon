use super::domain::{AuthorProfile, CurrentUser};

pub async fn find_current<'e, E>(
    executor: E,
    user_id: &str,
) -> Result<Option<CurrentUser>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        CurrentUser,
        r#"
        SELECT
            id,
            username,
            email,
            display_name,
            avatar_media_id,
            bio,
            role,
            status,
            theme_preference,
            language,
            created_at,
            updated_at,
            deleted_at
        FROM users
        WHERE id = ? AND deleted_at IS NULL
        "#,
        user_id
    )
    .fetch_optional(executor)
    .await
}

pub async fn find_password_hash<'e, E>(
    executor: E,
    user_id: &str,
) -> Result<Option<String>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(executor)
        .await
}

pub async fn update_profile<'e, E>(
    executor: E,
    user_id: &str,
    display_name: &str,
    bio: Option<&str>,
    avatar_media_id: Option<&str>,
    theme_preference: &str,
    language: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE users
         SET display_name = ?, bio = ?, avatar_media_id = ?, theme_preference = ?, language = ?, updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(display_name)
    .bind(bio)
    .bind(avatar_media_id)
    .bind(theme_preference)
    .bind(language)
    .bind(user_id)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn update_password<'e, E>(
    executor: E,
    user_id: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(password_hash)
        .bind(user_id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn find_public_by_username<'e, E>(
    executor: E,
    username: &str,
) -> Result<Option<AuthorProfile>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        AuthorProfile,
        r#"
        SELECT username, display_name, bio, avatar_media_id
        FROM users
        WHERE username = ? AND deleted_at IS NULL
        "#,
        username
    )
    .fetch_optional(executor)
    .await
}

/// 查询用户当前的 token_version，用于 JWT 签发时打入 claims
pub async fn find_token_version<'e, E>(
    executor: E,
    user_id: &str,
) -> Result<Option<i32>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar("SELECT token_version FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(executor)
        .await
}

/// 自增 token_version，使该用户所有已签发 JWT 立即失效
pub async fn increment_token_version<'e, E>(
    executor: E,
    user_id: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE users SET token_version = token_version + 1 WHERE id = ?")
        .bind(user_id)
        .execute(executor)
        .await?;
    Ok(())
}
