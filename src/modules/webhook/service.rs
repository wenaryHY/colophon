use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use futures_util::future;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use sqlx::SqlitePool;
use tokio::time::sleep;

use crate::{
    bootstrap::config::WebhookConfig,
    modules::plugin::hook::{Hook, HookContext, HookData, HookHandler},
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
};

// ── 重试退避常量 ──
const INITIAL_DELAY_SECONDS_FOR_WEBHOOK_RETRY: u64 = 5;

/// Webhook HTTP client 懒加载，避免 expect 硬崩溃
static WEBHOOK_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_webhook_http_client() -> &'static reqwest::Client {
    WEBHOOK_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new()) // 降级到默认 client
    })
}

/// Webhook 分发器——注册为 HookHandler 监听 post.after_save / post.after_publish 等事件
#[derive(Clone)]
pub struct WebhookDispatcher {
    pool: SqlitePool,
    config: WebhookConfig,
}

impl WebhookDispatcher {
    pub fn new(pool: SqlitePool, config: WebhookConfig) -> Self {
        Self { pool, config }
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
        let event = ctx.hook_name.clone();
        let payload = serialize_hook_data_to_json(&event, &ctx.data);
        let pool = self.pool.clone();

        let config = self.config.clone();
        tokio::spawn(async move {
            dispatch_webhooks_for_event(&pool, get_webhook_http_client(), &event, &payload, &config).await;
        });

        Ok(())
    }
}

/// 将 HookData 序列化为 webhook payload JSON
#[allow(unreachable_patterns)]
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
        _ => serde_json::json!({}),
    };

    serde_json::json!({
        "event": event,
        "timestamp": Utc::now().to_rfc3339(),
        "data": data_value,
    })
}

/// 查询匹配的 webhook 并通过有界并发分发
async fn dispatch_webhooks_for_event(
    pool: &SqlitePool,
    client: &Client,
    event: &str,
    payload: &serde_json::Value,
    config: &WebhookConfig,
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
            let payload_str = payload.to_string();
            if let Err(insert_err) = repository::insert_failed_webhook_event(
                pool,
                event,
                &payload_str,
                &e.to_string(),
            )
            .await
            {
                tracing::error!(
                    module = "webhook",
                    event = event,
                    error = %insert_err,
                    "failed to insert event-level failure record — DB may be unavailable"
                );
            }
            return;
        }
    };

    if webhooks.is_empty() {
        return;
    }

    let payload_str = payload.to_string();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(config.max_concurrency));
    let mut handles = Vec::with_capacity(webhooks.len());

    for webhook in webhooks {
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => continue, // semaphore closed
        };
        let pool = pool.clone();
        let client = client.clone();
        let webhook = webhook.clone();
        let payload_str = payload_str.clone();
        let event = event.to_string();

        handles.push(tokio::spawn(async move {
            let _permit = permit; // 持有许可直到当前 webhook 完成

            let start = Instant::now();
            let (success, response_status, response_body) =
                send_webhook_with_retry(&client, &webhook, &payload_str).await;
            let duration_ms = start.elapsed().as_millis() as i64;

            // 记录投递日志
            if let Err(e) = repository::insert_delivery(
                &pool,
                &webhook.id,
                &event,
                &webhook.url,
                &payload_str,
                response_status,
                Some(&response_body),
                duration_ms,
                success,
            )
            .await
            {
                tracing::error!(
                    module = "webhook",
                    webhook_id = %webhook.id,
                    error = %e,
                    "failed to insert delivery record"
                );
            }

            // 更新最后触发时间
            let now = Utc::now().to_rfc3339();
            let last_error = if success { None } else { Some(response_body.as_str()) };
            if let Err(e) = repository::update_webhook_last_trigger(
                &pool,
                &webhook.id,
                &now,
                last_error,
            )
            .await
            {
                tracing::error!(
                    module = "webhook",
                    webhook_id = %webhook.id,
                    error = %e,
                    "failed to update webhook last_trigger"
                );
            }
        }));
    }

    // 等待所有 webhook 完成，带总超时保护
    if tokio::time::timeout(
        Duration::from_secs(config.timeout_seconds),
        future::join_all(handles),
    )
    .await
    .is_err()
    {
        tracing::warn!(
            module = "webhook",
            event = event,
            timeout_seconds = config.timeout_seconds,
            "webhook dispatch batch timed out"
        );
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
            // 标准指数退避: 5 * 2^(attempt-1), 上限 60 秒
            let delay_secs = (INITIAL_DELAY_SECONDS_FOR_WEBHOOK_RETRY
                * 2u64.pow(attempt as u32 - 1))
            .min(60);
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
    let body = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(
                module = "webhook",
                error = %e,
                "failed to read webhook response body"
            );
            String::new()
        }
    };
    Ok((status, body))
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
    if url::Url::parse(body.url.trim()).is_err() {
        return Err(AppError::BadRequest("invalid webhook URL".into()));
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
    use crate::modules::plugin::hook::{
        HookData, PostAfterPublishData, PostAfterSaveData, PostBeforeSaveData,
    };

    #[test]
    fn build_hmac_signature_produces_sha256_prefixed_hex() {
        let sig = build_hmac_signature("my_secret", r#"{"event":"test"}"#);
        assert!(sig.starts_with("sha256="));
        let hex_part = &sig["sha256=".len()..];
        assert_eq!(hex_part.len(), 64);
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn build_hmac_signature_is_deterministic() {
        let s1 = build_hmac_signature("secret", "body");
        let s2 = build_hmac_signature("secret", "body");
        assert_eq!(s1, s2);
    }

    #[test]
    fn build_hmac_signature_differs_with_different_secrets() {
        let s1 = build_hmac_signature("secret_a", "body");
        let s2 = build_hmac_signature("secret_b", "body");
        assert_ne!(s1, s2);
    }

    #[test]
    fn build_hmac_signature_differs_with_different_bodies() {
        let s1 = build_hmac_signature("secret", "body_a");
        let s2 = build_hmac_signature("secret", "body_b");
        assert_ne!(s1, s2);
    }

    #[test]
    fn serialize_post_after_save_event() {
        let data = HookData::PostAfterSave(PostAfterSaveData {
            post_id: "p1".into(),
            title: "Hello".into(),
            slug: "hello".into(),
            is_new: true,
            status: "draft".into(),
            old_status: None,
        });
        let json = serialize_hook_data_to_json("post.after_save", &data);
        assert_eq!(json["event"], "post.after_save");
        assert_eq!(json["data"]["post_id"], "p1");
        assert_eq!(json["data"]["title"], "Hello");
        assert_eq!(json["data"]["is_new"], true);
        assert!(json["timestamp"].as_str().is_some());
    }

    #[test]
    fn serialize_post_after_publish_event() {
        let data = HookData::PostAfterPublish(PostAfterPublishData {
            post_id: "p2".into(),
            title: "World".into(),
            slug: "world".into(),
            old_status: "draft".into(),
            new_status: "published".into(),
        });
        let json = serialize_hook_data_to_json("post.after_publish", &data);
        assert_eq!(json["data"]["old_status"], "draft");
        assert_eq!(json["data"]["new_status"], "published");
    }

    #[test]
    fn serialize_post_before_save_event() {
        let data = HookData::PostBeforeSave(PostBeforeSaveData {
            title: "Draft".into(),
            content_html: "<p>body</p>".into(),
            slug: "draft".into(),
            excerpt: Some("summary".into()),
            tags: vec!["rust".into()],
            category_id: Some("cat1".into()),
            content_type: "post".into(),
            request_ip: None,
            user_agent: None,
        });
        let json = serialize_hook_data_to_json("post.before_save", &data);
        assert_eq!(json["data"]["title"], "Draft");
        assert_eq!(json["data"]["tags"][0], "rust");
    }

    #[tokio::test]
    async fn webhook_dispatcher_produces_two_hooks() {
        let config = WebhookConfig {
            max_concurrency: 4,
            timeout_seconds: 30,
        };
        let pool = SqlitePool::connect_lazy("sqlite::memory:").unwrap();
        let dispatcher = WebhookDispatcher::new(pool, config);
        let hooks = dispatcher.into_hooks();
        assert_eq!(hooks.len(), 2);
    }

    #[test]
    fn default_webhook_dto_values() {
        let json = r#"{"name":"test","url":"https://example.com"}"#;
        let req: CreateWebhookRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.events, "post.after_publish");
        assert!(req.enabled);
        assert_eq!(req.max_retries, 3);
    }
}
