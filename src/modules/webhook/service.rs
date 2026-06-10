use std::net::Ipv6Addr;
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
            .redirect(reqwest::redirect::Policy::limited(3)) // 允许最多 3 次重定向，但最终 URL 会再次检查
            .build()
            .unwrap_or_else(|_| reqwest::Client::new()) // 降级到默认 client
    })
}

/// 检查 URL 是否指向私有 IP 或 localhost
/// 
/// 防止 SSRF 攻击：拒绝 webhook 指向内网地址
/// 设置 `INKFORGE_TEST_MODE=true` 环境变量可跳过检查（仅供集成测试使用）
fn is_private_or_local_url(url: &str) -> Result<bool, AppError> {
    // 集成测试模式下跳过 SSRF 检查，允许 webhook 使用 localhost 进行端到端测试
    if std::env::var("INKFORGE_TEST_MODE").is_ok() {
        return Ok(false);
    }

    let parsed = url::Url::parse(url)
        .map_err(|_| AppError::BadRequest("无效的 URL 格式".into()))?;

    // url::Host 枚举区分 Domain / Ipv4 / Ipv6，避免手动处理 IPv6 的方括号
    let host = parsed
        .host()
        .ok_or_else(|| AppError::BadRequest("URL 缺少 host".into()))?;

    match host {
        url::Host::Domain(domain) => {
            let lowered = domain.to_ascii_lowercase();
            // localhost 及其子域
            if lowered == "localhost" || lowered.ends_with(".localhost") {
                return Ok(true);
            }
            // 域名走 DNS，无法在此判定是否解析到内网；交给后续传输层即可
            // 注：理想方案是 resolve 后再比对 IP，但 DNS 重绑定攻击需要更深防御
            Ok(false)
        }
        url::Host::Ipv4(ipv4) => {
            // 0.0.0.0/8
            if ipv4.octets()[0] == 0 {
                return Ok(true);
            }
            // 10.0.0.0/8
            if ipv4.octets()[0] == 10 {
                return Ok(true);
            }
            // 127.0.0.0/8 (loopback)
            if ipv4.octets()[0] == 127 {
                return Ok(true);
            }
            // 169.254.0.0/16 (链路本地)
            if ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254 {
                return Ok(true);
            }
            // 172.16.0.0/12
            if ipv4.octets()[0] == 172 && (ipv4.octets()[1] >= 16 && ipv4.octets()[1] <= 31) {
                return Ok(true);
            }
            // 192.168.0.0/16
            if ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168 {
                return Ok(true);
            }
            Ok(false)
        }
        url::Host::Ipv6(ipv6) => {
            // ::1 (loopback)
            if ipv6 == Ipv6Addr::LOCALHOST {
                return Ok(true);
            }
            // :: (unspecified)
            if ipv6.is_unspecified() {
                return Ok(true);
            }
            // fe80::/10 (链路本地)
            if ipv6.segments()[0] & 0xffc0 == 0xfe80 {
                return Ok(true);
            }
            // fc00::/7 (唯一本地)
            if ipv6.segments()[0] & 0xfe00 == 0xfc00 {
                return Ok(true);
            }
            // ::ffff:0:0/96 (IPv4-mapped) — 转回 IPv4 检查
            if let Some(v4) = ipv6.to_ipv4_mapped() {
                let mapped = format!("http://{}/", v4);
                return is_private_or_local_url(&mapped);
            }
            Ok(false)
        }
    }
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

/// 检查 IP 地址是否为私有或本地地址
fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            // 0.0.0.0/8
            ipv4.octets()[0] == 0
                // 10.0.0.0/8
                || ipv4.octets()[0] == 10
                // 127.0.0.0/8 (loopback)
                || ipv4.octets()[0] == 127
                // 169.254.0.0/16 (link-local)
                || (ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254)
                // 172.16.0.0/12
                || (ipv4.octets()[0] == 172 && (ipv4.octets()[1] >= 16 && ipv4.octets()[1] <= 31))
                // 192.168.0.0/16
                || (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168)
        }
        std::net::IpAddr::V6(ipv6) => {
            // ::1 (loopback)
            *ipv6 == Ipv6Addr::LOCALHOST
                // :: (unspecified)
                || ipv6.is_unspecified()
                // fe80::/10 (link-local)
                || (ipv6.segments()[0] & 0xffc0 == 0xfe80)
                // fc00::/7 (unique local)
                || (ipv6.segments()[0] & 0xfe00 == 0xfc00)
        }
    }
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
/// 
/// 🔒 P2-3 修复：DNS 重绑定防护
/// 在实际发送前二次解析域名，防止攻击者在创建后修改 DNS 记录指向内网
async fn try_send_webhook(
    client: &Client,
    webhook: &Webhook,
    payload: &str,
) -> Result<(i64, String), String> {
    // 🔒 DNS 重绑定防护：触发时二次检查 URL 是否解析到私有地址
    if let Ok(parsed) = url::Url::parse(&webhook.url) {
        if let Some(host) = parsed.host_str() {
            // 只对域名进行 DNS 解析检查（跳过直接 IP 地址，因为已在创建时检查）
            if host.parse::<std::net::IpAddr>().is_err() {
                // 这是域名，需要解析并检查所有 IP
                let port = parsed.port_or_known_default().unwrap_or(443);
                match tokio::net::lookup_host(format!("{}:{}", host, port)).await {
                    Ok(addrs) => {
                        for addr in addrs {
                            if is_private_ip(&addr.ip()) {
                                let error_msg = format!(
                                    "DNS rebinding attack detected: domain '{}' resolved to private IP {} at delivery time",
                                    host, addr.ip()
                                );
                                tracing::warn!(
                                    module = "webhook",
                                    webhook_id = %webhook.id,
                                    domain = host,
                                    resolved_ip = %addr.ip(),
                                    "DNS rebinding attack detected"
                                );
                                return Err(error_msg);
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("DNS resolution failed: {}", e);
                        tracing::error!(
                            module = "webhook",
                            webhook_id = %webhook.id,
                            domain = host,
                            error = %e,
                            "DNS resolution failed during webhook delivery"
                        );
                        return Err(error_msg);
                    }
                }
            }
        }
    }

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

    let response = req.send().await.map_err(|e| e.to_string())?;

    // 🔒 重定向安全检查：验证最终 URL 是否指向私有地址（防止通过 302 绕过初始检查）
    let final_url = response.url().as_str();
    if is_private_or_local_url(final_url)
        .unwrap_or_else(|_| false)
    {
        let error_msg = format!(
            "final request URL {} resolves to private address (potential SSRF attack via redirect)",
            final_url
        );
        tracing::warn!(
            module = "webhook",
            webhook_id = %webhook.id,
            original_url = %webhook.url,
            final_url = final_url,
            "SSRF attack prevented: redirect to private address"
        );
        return Err(error_msg);
    }

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

    // 🔒 SSRF 防护：拒绝私有 IP
    if is_private_or_local_url(body.url.trim())? {
        return Err(AppError::BadRequest(
            "禁止 webhook 指向内网地址或 localhost".into()
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
                "禁止 webhook 指向内网地址或 localhost".into()
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

    #[test]
    fn test_is_private_or_local_url() {
        // localhost
        assert!(is_private_or_local_url("http://localhost/api").unwrap());
        assert!(is_private_or_local_url("http://127.0.0.1:8080/").unwrap());
        assert!(is_private_or_local_url("http://[::1]/").unwrap());
        assert!(is_private_or_local_url("http://app.localhost/").unwrap());

        // 私有 IP (10.x.x.x)
        assert!(is_private_or_local_url("http://10.0.0.1/").unwrap());
        assert!(is_private_or_local_url("http://10.255.255.255/").unwrap());

        // 私有 IP (172.16-31.x.x)
        assert!(is_private_or_local_url("http://172.16.0.1/").unwrap());
        assert!(is_private_or_local_url("http://172.31.255.255/").unwrap());

        // 私有 IP (192.168.x.x)
        assert!(is_private_or_local_url("http://192.168.1.1/").unwrap());

        // 链路本地 (169.254.x.x)
        assert!(is_private_or_local_url("http://169.254.1.1/").unwrap());

        // 0.0.0.0/8
        assert!(is_private_or_local_url("http://0.0.0.0/").unwrap());

        // IPv6 私有/链路本地/唯一本地
        assert!(is_private_or_local_url("http://[fe80::1]/").unwrap());
        assert!(is_private_or_local_url("http://[fc00::1]/").unwrap());
        assert!(is_private_or_local_url("http://[fd00::1]/").unwrap());

        // 公网 IP（允许）
        assert!(!is_private_or_local_url("https://api.example.com/webhook").unwrap());
        assert!(!is_private_or_local_url("http://8.8.8.8/").unwrap());
        assert!(!is_private_or_local_url("https://1.1.1.1/").unwrap());

        // 边界：172.15.x.x 与 172.32.x.x 不在私有段
        assert!(!is_private_or_local_url("http://172.15.0.1/").unwrap());
        assert!(!is_private_or_local_url("http://172.32.0.1/").unwrap());

        // 边界：192.169.x.x 不在私有段
        assert!(!is_private_or_local_url("http://192.169.1.1/").unwrap());

        // 无效 URL
        assert!(is_private_or_local_url("not a url").is_err());
    }

    #[test]
    fn test_is_private_ip() {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

        // IPv4 私有地址
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))));

        // IPv4 公网地址
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 15, 0, 1))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1))));

        // IPv6 私有地址
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1))));

        // IPv6 公网地址
        assert!(!is_private_ip(&IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888))));
    }
}
