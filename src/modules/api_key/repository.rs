use uuid::Uuid;

use super::domain::{ApiKey, ApiKeyWithUser};

pub async fn insert_api_key<'e, E>(
    executor: E,
    user_id: &str,
    name: &str,
    key_prefix: &str,
    key_hash: &str,
    permissions: &str,
    expires_at: Option<&str>,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO api_keys (id, user_id, name, key_prefix, key_hash, permissions, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(name)
    .bind(key_prefix)
    .bind(key_hash)
    .bind(permissions)
    .bind(expires_at)
    .execute(executor)
    .await?;
    Ok(id)
}

pub async fn get_api_key_by_id<'e, E>(executor: E, id: &str) -> Result<Option<ApiKey>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        ApiKey,
        r#"
        SELECT
            id as "id!",
            user_id as "user_id!",
            name as "name!",
            key_prefix as "key_prefix!",
            key_hash as "key_hash!",
            permissions as "permissions!",
            last_used_at,
            expires_at,
            created_at as "created_at!",
            updated_at as "updated_at!"
        FROM api_keys
        WHERE id = ?
        LIMIT 1
        "#,
        id
    )
    .fetch_optional(executor)
    .await
}

pub async fn list_api_keys_by_user_id<'e, E>(
    executor: E,
    user_id: &str,
) -> Result<Vec<ApiKey>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        ApiKey,
        r#"
        SELECT
            id as "id!",
            user_id as "user_id!",
            name as "name!",
            key_prefix as "key_prefix!",
            key_hash as "key_hash!",
            permissions as "permissions!",
            last_used_at,
            expires_at,
            created_at as "created_at!",
            updated_at as "updated_at!"
        FROM api_keys
        WHERE user_id = ?
        ORDER BY created_at DESC
        "#,
        user_id
    )
    .fetch_all(executor)
    .await
}

/// 根据 key_hash 查询 ApiKey 及其关联的 user 信息，用于认证
pub async fn find_api_key_with_user_by_hash<'e, E>(
    executor: E,
    key_hash: &str,
) -> Result<Option<ApiKeyWithUser>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        ApiKeyWithUser,
        r#"
        SELECT
            ak.id AS "api_key_id!",
            ak.user_id as "user_id!",
            u.username as "username!",
            u.role as "role!",
            ak.permissions as "permissions!",
            ak.expires_at AS api_key_expires_at
        FROM api_keys ak
        JOIN users u ON u.id = ak.user_id
        WHERE ak.key_hash = ?
        LIMIT 1
        "#,
        key_hash
    )
    .fetch_optional(executor)
    .await
}

pub async fn update_api_key_last_used_at<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE api_keys SET last_used_at = datetime('now'), updated_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn update_api_key_name<'e, E>(
    executor: E,
    id: &str,
    name: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE api_keys SET name = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn delete_api_key<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("DELETE FROM api_keys WHERE id = ?")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}
