use uuid::Uuid;

use super::domain::{Webhook, WebhookDelivery};

/// 列出所有 webhook
pub async fn list_all_webhooks<'e, E>(executor: E) -> Result<Vec<Webhook>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks ORDER BY created_at DESC")
        .fetch_all(executor)
        .await
}

/// 根据 ID 获取单个 webhook
pub async fn get_webhook_by_id<'e, E>(executor: E, id: &str) -> Result<Option<Webhook>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(executor)
        .await
}

/// 查询所有启用且事件列表匹配指定事件的 webhook
pub async fn list_enabled_webhooks_for_event<'e, E>(
    executor: E,
    event: &str,
) -> Result<Vec<Webhook>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let pattern = format!("%{}%", event);
    sqlx::query_as::<_, Webhook>(
        "SELECT * FROM webhooks WHERE enabled = 1 AND events LIKE ? ORDER BY created_at ASC",
    )
    .bind(&pattern)
    .fetch_all(executor)
    .await
}

/// 插入新 webhook
pub async fn insert_webhook<'e, E>(
    executor: E,
    name: &str,
    url: &str,
    events: &str,
    secret: Option<&str>,
    enabled: bool,
    max_retries: i64,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    let enabled_int: i64 = if enabled { 1 } else { 0 };
    sqlx::query(
        "INSERT INTO webhooks (id, name, url, events, secret, enabled, max_retries) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(url)
    .bind(events)
    .bind(secret)
    .bind(enabled_int)
    .bind(max_retries)
    .execute(executor)
    .await?;
    Ok(id)
}

/// 更新 webhook（逐字段更新）
pub async fn update_webhook<'e, E>(
    executor: E,
    id: &str,
    name: Option<&str>,
    url: Option<&str>,
    events: Option<&str>,
    secret: Option<Option<&str>>,
    enabled: Option<bool>,
    max_retries: Option<i64>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    if let Some(n) = name {
        sqlx::query("UPDATE webhooks SET name = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(n)
            .bind(id)
            .execute(executor)
            .await?;
    }
    if let Some(u) = url {
        sqlx::query("UPDATE webhooks SET url = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(u)
            .bind(id)
            .execute(executor)
            .await?;
    }
    if let Some(e) = events {
        sqlx::query("UPDATE webhooks SET events = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(e)
            .bind(id)
            .execute(executor)
            .await?;
    }
    if let Some(s) = secret {
        sqlx::query("UPDATE webhooks SET secret = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(s)
            .bind(id)
            .execute(executor)
            .await?;
    }
    if let Some(en) = enabled {
        let en_int: i64 = if en { 1 } else { 0 };
        sqlx::query("UPDATE webhooks SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(en_int)
            .bind(id)
            .execute(executor)
            .await?;
    }
    if let Some(mr) = max_retries {
        sqlx::query("UPDATE webhooks SET max_retries = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(mr)
            .bind(id)
            .execute(executor)
            .await?;
    }

    Ok(())
}

/// 删除 webhook
pub async fn delete_webhook<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("DELETE FROM webhooks WHERE id = ?")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

/// 更新 webhook 的最后触发时间和错误信息
pub async fn update_webhook_last_trigger<'e, E>(
    executor: E,
    id: &str,
    last_triggered_at: &str,
    last_error: Option<&str>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE webhooks SET last_triggered_at = ?, last_error = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(last_triggered_at)
    .bind(last_error)
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}

/// 插入投递记录
pub async fn insert_delivery<'e, E>(
    executor: E,
    webhook_id: &str,
    event: &str,
    request_url: &str,
    request_body: &str,
    response_status: Option<i64>,
    response_body: Option<&str>,
    duration_ms: i64,
    success: bool,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    let success_int: i64 = if success { 1 } else { 0 };
    sqlx::query(
        "INSERT INTO webhook_deliveries (id, webhook_id, event, request_url, request_body, response_status, response_body, duration_ms, success) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(webhook_id)
    .bind(event)
    .bind(request_url)
    .bind(request_body)
    .bind(response_status)
    .bind(response_body)
    .bind(duration_ms)
    .bind(success_int)
    .execute(executor)
    .await?;
    Ok(id)
}

/// 获取某个 webhook 的投递记录列表
pub async fn list_deliveries_for_webhook<'e, E>(
    executor: E,
    webhook_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<WebhookDelivery>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, WebhookDelivery>(
        "SELECT * FROM webhook_deliveries WHERE webhook_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(webhook_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await
}

/// 统计某个 webhook 的投递记录总数
pub async fn count_deliveries_for_webhook<'e, E>(
    executor: E,
    webhook_id: &str,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM webhook_deliveries WHERE webhook_id = ?")
            .bind(webhook_id)
            .fetch_one(executor)
            .await?;
    Ok(row.0)
}

/// 删除指定 webhook 的所有投递记录
#[allow(dead_code)]
pub async fn delete_deliveries_for_webhook<'e, E>(
    executor: E,
    webhook_id: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("DELETE FROM webhook_deliveries WHERE webhook_id = ?")
        .bind(webhook_id)
        .execute(executor)
        .await?;
    Ok(())
}
