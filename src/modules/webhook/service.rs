//! Webhook CRUD 服务
//!
//! 仅承担管理面的增删改查；投递管线见 [`super::dispatcher`]，
//! 目标 URL 的 SSRF 校验见 [`super::ssrf`]。

use std::sync::Arc;

use crate::{
    shared::{
        error::{AppError, AppResult},
        response::deleted_json,
    },
    state::AppState,
};

use super::{
    domain::{Webhook, WebhookDelivery},
    dto::{CreateWebhookRequest, UpdateWebhookRequest},
    repository,
    ssrf::is_private_or_local_url,
};

/// 列出所有 webhook
pub async fn list_webhooks(state: Arc<AppState>) -> AppResult<Vec<Webhook>> {
    Ok(repository::list_all_webhooks(&state.pool).await?)
}

/// 获取单个 webhook
pub async fn get_webhook(state: Arc<AppState>, id: &str) -> AppResult<Webhook> {
    repository::get_webhook_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 创建 webhook
pub async fn create_webhook(
    state: Arc<AppState>,
    body: CreateWebhookRequest,
) -> AppResult<Webhook> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("webhook name is required".into()));
    }
    if body.url.trim().is_empty() {
        return Err(AppError::BadRequest("webhook url is required".into()));
    }
    if url::Url::parse(body.url.trim()).is_err() {
        return Err(AppError::BadRequest("invalid webhook URL".into()));
    }

    // 🔒 SSRF 防护：拒绝私有 IP
    if is_private_or_local_url(body.url.trim())? {
        return Err(AppError::BadRequest(
            "禁止 webhook 指向内网地址或 localhost".into(),
        ));
    }

    let events = if body.events.trim().is_empty() {
        "post.after_publish".to_string()
    } else {
        body.events.trim().to_string()
    };

    let secret = body.secret.filter(|s| !s.is_empty());
    let id = repository::insert_webhook(
        &state.pool,
        body.name.trim(),
        body.url.trim(),
        &events,
        secret.as_deref(),
        body.enabled,
        body.max_retries,
    )
    .await?;

    repository::get_webhook_by_id(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 更新 webhook
pub async fn update_webhook(
    state: Arc<AppState>,
    id: &str,
    body: UpdateWebhookRequest,
) -> AppResult<Webhook> {
    let _existing = repository::get_webhook_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    // 校验 name / url 非空
    if let Some(ref name) = body.name {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("webhook name cannot be empty".into()));
        }
    }
    if let Some(ref url) = body.url {
        if url.trim().is_empty() {
            return Err(AppError::BadRequest("webhook url cannot be empty".into()));
        }
        if url::Url::parse(url.trim()).is_err() {
            return Err(AppError::BadRequest("invalid webhook URL".into()));
        }
        // 🔒 SSRF 防护：拒绝私有 IP
        if is_private_or_local_url(url.trim())? {
            return Err(AppError::BadRequest(
                "禁止 webhook 指向内网地址或 localhost".into(),
            ));
        }
    }

    let name = body.name.as_deref().map(|s| s.trim());
    let url = body.url.as_deref().map(|s| s.trim());
    let events = body.events.as_deref().map(|s| s.trim());
    let secret = match &body.secret {
        Some(s) if s.is_empty() => Some(None),
        Some(s) => Some(Some(s.as_str())),
        None => None,
    };
    let enabled = body.enabled;
    let max_retries = body.max_retries;

    repository::update_webhook(
        &state.pool,
        id,
        name,
        url,
        events,
        secret,
        enabled,
        max_retries,
    )
    .await?;

    repository::get_webhook_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 删除 webhook
pub async fn delete_webhook(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    repository::delete_webhook(&state.pool, id).await?;
    deleted_json()
}

/// 获取 webhook 投递记录列表
pub async fn list_deliveries(
    state: Arc<AppState>,
    webhook_id: &str,
    page: i64,
    page_size: i64,
) -> AppResult<(Vec<WebhookDelivery>, i64)> {
    // 校验 webhook 存在
    repository::get_webhook_by_id(&state.pool, webhook_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let page = page.max(1);
    let page_size = page_size.min(100).max(1);
    let offset = (page - 1) * page_size;

    let deliveries =
        repository::list_deliveries_for_webhook(&state.pool, webhook_id, page_size, offset).await?;
    let total = repository::count_deliveries_for_webhook(&state.pool, webhook_id).await?;

    Ok((deliveries, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_webhook_dto_values() {
        let json = r#"{"name":"test","url":"https://example.com"}"#;
        let req: CreateWebhookRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.events, "post.after_publish");
        assert!(req.enabled);
        assert_eq!(req.max_retries, 3);
    }
}
