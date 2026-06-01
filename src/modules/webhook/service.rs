use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use sqlx::SqlitePool;
use tokio::time::sleep;

use crate::{
    modules::plugin::hook::{Hook, HookContext, HookData, HookHandler},
    shared::error::{AppError, AppResult},
    state::AppState,
};

use super::{
    domain::{Webhook, WebhookDelivery},
    dto::{CreateWebhookRequest, UpdateWebhookRequest},
    repository,
};

// ── 重试退避常量 ──
const RETRY_BASE_DELAY_SECS: u64 = 5;

/// Webhook 分发器——注册为 HookHandler 监听 post.after_save / post.after_publish 等事件
#[derive(Clone)]
pub struct WebhookDispatcher {
    pool: SqlitePool,
    client: Client,
}

impl WebhookDispatcher {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build reqwest Client for webhook dispatcher"),
        }
    }

    /// 将本分发器包装为 action hooks
    pub fn into_hooks(self) -> Vec<Hook> {
        let handler: Arc<dyn HookHandler> = Arc::new(self);
        vec![
            Hook::new_action("post.after_save", 0, "webhook", handler.clone()),
            Hook::new_action("post.after_publish", 0, "webhook", handler.clone()),
        ]
    }
}

#[async_trait]
impl HookHandler for WebhookDispatcher {
    async fn run(&self, ctx: &mut HookContext) -> AppResult<()> {
        let event = &ctx.hook_name;
        let payload = serialize_hook_data_to_json(event, &ctx.data);
        dispatch_webhooks_for_event(&self.pool, &self.client, event, &payload).await;
        Ok(())
    }
}

/// 将 HookData 序列化为 webhook payload JSON
fn serialize_hook_data_to_json(event: &str, data: &HookData) -> serde_json::Value {
    let data_value = match data {
        HookData::PostAfterSave(d) => serde_json::json!({
            "post_id": d.post_id,
            "title": d.title,
            "slug": d.slug,
            "is_new": d.is_new,
            "status": d.status,
            "old_status": d.old_status,
        }),
        HookData::PostAfterPublish(d) => serde_json::json!({
            "post_id": d.post_id,
            "title": d.title,
            "slug": d.slug,
            "old_status": d.old_status,
            "new_status": d.new_status,
        }),
        HookData::PostBeforeSave(d) => serde_json::json!({
            "title": d.title,
            "slug": d.slug,
            "excerpt": d.excerpt,
            "tags": d.tags,
            "category_id": d.category_id,
            "content_type": d.content_type,
        }),
        HookData::PostBeforeRender(d) => serde_json::json!({
            "post_id": d.post_id,
            "title": d.title,
            "slug": d.slug,
        }),
        HookData::CommentBeforeCreate(d) => serde_json::json!({
            "content": d.content,
            "author_name": d.author_name,
            "post_id": d.post_id,
            "post_title": d.post_title,
        }),
    };

    serde_json::json!({
        "event": event,
        "timestamp": Utc::now().to_rfc3339(),
        "data": data_value,
    })
}

/// 查询匹配的 webhook 并逐个分发
async fn dispatch_webhooks_for_event(
    pool: &SqlitePool,
    client: &Client,
    event: &str,
    payload: &serde_json::Value,
) {
    let webhooks = match repository::list_enabled_webhooks_for_event(pool, event).await {
        Ok(list) => list,
        Err(e) => {
            tracing::error!(
                module = "webhook",
                event = event,
                error = %e,
                "failed to list enabled webhooks for event"
            );
            return;
        }
    };

    let payload_str = payload.to_string();

    for webhook in webhooks {
        let start = Instant::now();
        let (success, status, response_body) =
            send_webhook_with_retry(client, &webhook, &payload_str).await;
        let duration_ms = start.elapsed().as_millis() as i64;

        record_delivery(
            pool,
            &webhook.id,
            event,
            &webhook.url,
            &payload_str,
            status,
            Some(response_body.as_str()),
            duration_ms,
            success,
        )
        .await;

        // 更新最后触发时间
        let now = Utc::now().to_rfc3339();
        let last_error = if success { None } else { Some(response_body.as_str()) };
        let _ = repository::update_webhook_last_trigger(
            pool,
            &webhook.id,
            &now,
            last_error,
        )
        .await;
    }
}

/// 发送单个 webhook 请求，支持重试
async fn send_webhook_with_retry(
    client: &Client,
    webhook: &Webhook,
    payload: &str,
) -> (bool, Option<i64>, String) {
    let max_retries = webhook.max_retries.min(5); // 最多 5 次重试
    let mut last_status: Option<i64> = None;
    let mut last_body = String::new();

    for attempt in 0..=max_retries {
        if attempt > 0 {
            // 指数退避，上限 60 秒
            let delay_secs = (RETRY_BASE_DELAY_SECS.pow(attempt as u32)).min(60);
            tracing::warn!(
                module = "webhook",
                webhook_id = %webhook.id,
                attempt = attempt,
                delay_secs = delay_secs,
                "retrying webhook delivery"
            );
            sleep(Duration::from_secs(delay_secs)).await;
        }

        match try_send_webhook(client, webhook, payload).await {
            Ok((status, body)) => {
                if status >= 200 && status < 300 {
                    return (true, Some(status), body);
                }
                // 4xx 客户端错误不重试
                if status >= 400 && status < 500 {
                    tracing::warn!(
                        module = "webhook",
                        webhook_id = %webhook.id,
                        status = status,
                        "webhook client error, not retrying"
                    );
                    return (false, Some(status), body);
                }
                last_status = Some(status);
                last_body = body;
            }
            Err(e) => {
                last_body = e.to_string();
                tracing::error!(
                    module = "webhook",
                    webhook_id = %webhook.id,
                    attempt = attempt,
                    error = %e,
                    "webhook request failed"
                );
            }
        }
    }

    (false, last_status, last_body)
}

/// 构建签名头
fn build_hmac_signature(secret: &str, body: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(body.as_bytes());
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

/// 尝试发送一次 HTTP 请求
async fn try_send_webhook(
    client: &Client,
    webhook: &Webhook,
    payload: &str,
) -> Result<(i64, String), reqwest::Error> {
    let mut req = client
        .post(&webhook.url)
        .header("Content-Type", "application/json")
        .header("User-Agent", "InkForge-Webhook/1.0")
        .body(payload.to_string());

    // 如果配置了 secret，添加 HMAC 签名
    if let Some(ref secret) = webhook.secret {
        if !secret.is_empty() {
            let signature = build_hmac_signature(secret, payload);
            req = req.header("X-Webhook-Signature", signature);
        }
    }

    let response = req.send().await?;
    let status = response.status().as_u16() as i64;
    let body = response.text().await.unwrap_or_default();
    Ok((status, body))
}

/// 记录投递日志
async fn record_delivery(
    pool: &SqlitePool,
    webhook_id: &str,
    event: &str,
    request_url: &str,
    request_body: &str,
    response_status: Option<i64>,
    response_body: Option<&str>,
    duration_ms: i64,
    success: bool,
) {
    if let Err(e) = repository::insert_delivery(
        pool,
        webhook_id,
        event,
        request_url,
        request_body,
        response_status,
        response_body,
        duration_ms,
        success,
    )
    .await
    {
        tracing::error!(
            module = "webhook",
            webhook_id = webhook_id,
            error = %e,
            "failed to record webhook delivery"
        );
    }
}

// ── CRUD 服务 ──

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
    Ok(serde_json::json!({ "deleted": true }))
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
