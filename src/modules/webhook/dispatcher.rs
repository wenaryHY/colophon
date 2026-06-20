//! Webhook 投递管线（深模块）
//!
//! 监听 `post.after_save` / `post.after_publish` 等事件，将事件序列化为 payload，
//! 经 SSRF / DNS 重绑定校验、HMAC 签名、指数退避重试后投递到已启用的 webhook，
//! 并记录投递日志。安全敏感的 IP 段判定委托给 [`super::ssrf`]。

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use futures_util::future;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::SqlitePool;
use tokio::time::sleep;

use crate::{
    bootstrap::config::WebhookConfig,
    modules::plugin::hook::{Hook, HookContext, HookData, HookHandler},
    shared::error::AppResult,
};

use super::{
    dns::DnsResolver,
    domain::Webhook,
    repository,
    ssrf::is_private_ip,
};

// ── 重试退避常量 ──
const INITIAL_DELAY_SECONDS_FOR_WEBHOOK_RETRY: u64 = 5;

/// Webhook 分发器——注册为 HookHandler 监听 post.after_save / post.after_publish 等事件
#[derive(Clone)]
pub struct WebhookDispatcher {
    pool: SqlitePool,
    config: WebhookConfig,
    resolver: Arc<dyn DnsResolver>,
}

impl WebhookDispatcher {
    pub fn new(pool: SqlitePool, config: WebhookConfig, resolver: Arc<dyn DnsResolver>) -> Self {
        Self {
            pool,
            config,
            resolver,
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
        let event = ctx.hook_name.clone();
        let payload = serialize_hook_data_to_json(&event, &ctx.data);
        let pool = self.pool.clone();

        let config = self.config.clone();
        let resolver = self.resolver.clone();
        tokio::spawn(async move {
            dispatch_webhooks_for_event(&pool, &event, &payload, &config, resolver).await;
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
    event: &str,
    payload: &serde_json::Value,
    config: &WebhookConfig,
    resolver: Arc<dyn DnsResolver>,
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
            if let Err(insert_err) =
                repository::insert_failed_webhook_event(pool, event, &payload_str, &e.to_string())
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
        let webhook = webhook.clone();
        let payload_str = payload_str.clone();
        let event = event.to_string();
        let resolver = Arc::clone(&resolver);

        handles.push(tokio::spawn(async move {
            let _permit = permit; // 持有许可直到当前 webhook 完成

            let start = Instant::now();
            let (success, response_status, response_body) =
                send_webhook_with_retry(&webhook, &payload_str, &*resolver).await;
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
            let last_error = if success {
                None
            } else {
                Some(response_body.as_str())
            };
            if let Err(e) =
                repository::update_webhook_last_trigger(&pool, &webhook.id, &now, last_error).await
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
    webhook: &Webhook,
    payload: &str,
    resolver: &dyn DnsResolver,
) -> (bool, Option<i64>, String) {
    let max_retries = webhook.max_retries.min(5); // 最多 5 次重试
    let mut last_status: Option<i64> = None;
    let mut last_body = String::new();

    for attempt in 0..=max_retries {
        if attempt > 0 {
            // 标准指数退避: 5 * 2^(attempt-1), 上限 60 秒
            let delay_secs =
                (INITIAL_DELAY_SECONDS_FOR_WEBHOOK_RETRY * 2u64.pow(attempt as u32 - 1)).min(60);
            tracing::warn!(
                module = "webhook",
                webhook_id = %webhook.id,
                attempt = attempt,
                delay_secs = delay_secs,
                "retrying webhook delivery"
            );
            sleep(Duration::from_secs(delay_secs)).await;
        }

        match try_send_webhook(webhook, payload, resolver).await {
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
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(body.as_bytes());
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

/// 尝试发送一次 HTTP 请求
///
/// 🔒 DNS 重绑定防护：
/// 先用 tokio::net::lookup_host 解析域名，找到第一个非内网 IP，
/// 再通过 `reqwest::Client::builder().resolve()` 将域名强制绑定到该 IP。
/// 这样 reqwest 不会独立解析 DNS，消除了两次解析之间 TTL=0 攻击窗口。
/// 同时关闭自动重定向，防止通过 302 跳转到内网地址。
///
/// 注意：此处为每次调用构建临时 Client（而非复用全局连接池），
/// 这是安全换性能的正确取舍——连接池复用会让 DNS 重绑定防护形同虚设。
async fn try_send_webhook(
    webhook: &Webhook,
    payload: &str,
    resolver: &dyn DnsResolver,
) -> Result<(i64, String), String> {
    let parsed = url::Url::parse(&webhook.url)
        .map_err(|e| format!("invalid webhook URL: {}", e))?;
    let host_str = parsed
        .host_str()
        .ok_or_else(|| "webhook URL missing host".to_string())?;
    let port = parsed.port_or_known_default().unwrap_or(443);

    // DNS 解析：找到第一个非内网 IP（域名和 IP 地址统一走此路径）
    let addrs = resolver
        .lookup_host(host_str, port)
        .await
        .map_err(|e| format!("DNS resolution failed: {}", e))?;

    let safe_addr = addrs
        .into_iter()
        .find(|addr| !is_private_ip(&addr.ip()))
        .ok_or_else(|| {
            format!(
                "SSRF prevention: domain '{}' resolved exclusively to private/internal IPs at delivery time",
                host_str
            )
        })?;

    // 构建临时 client，强制 reqwest 使用我们解析出的 IP，杜绝独立 DNS 解析
    // 注：不使用全局共享 client（OnceLock），否则 DNS 重绑定防护无效
    let transient_client = reqwest::Client::builder()
        .resolve(host_str, safe_addr)
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("failed to build HTTP client: {}", e))?;

    let mut req = transient_client
        .post(webhook.url.clone())
        .header("Content-Type", "application/json")
        .header("User-Agent", "Colophon-Webhook/1.0")
        .body(payload.to_string());

    // 如果配置了 secret，添加 HMAC 签名
    if let Some(ref secret) = webhook.secret {
        if !secret.is_empty() {
            let signature = build_hmac_signature(secret, payload);
            req = req.header("X-Webhook-Signature", signature);
        }
    }

    let response = req.send().await.map_err(|e| e.to_string())?;

    // 已禁用自动重定向（Policy::none），无需额外检查最终 URL

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
        let resolver = Arc::new(crate::modules::webhook::dns::TokioDnsResolver);
        let dispatcher = WebhookDispatcher::new(pool, config, resolver);
        let hooks = dispatcher.into_hooks();
        assert_eq!(hooks.len(), 2);
    }

    // ── MockResolver ──

    /// Mock DNS 解析器，返回预设的 SocketAddr 列表，无需真实网络。
    struct MockResolver {
        addrs: Vec<std::net::SocketAddr>,
    }

    impl DnsResolver for MockResolver {
        fn lookup_host(
            &self,
            _host: &str,
            _port: u16,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<Vec<std::net::SocketAddr>, std::io::Error>>
                    + Send
                    + '_,
            >,
        > {
            let addrs = self.addrs.clone();
            Box::pin(async move { Ok(addrs) })
        }
    }

    fn make_test_webhook(url: &str) -> Webhook {
        Webhook {
            id: "wh_test".into(),
            name: "test".into(),
            url: url.to_string(),
            events: "post.after_save".into(),
            secret: None,
            enabled: 1,
            retry_count: 0,
            max_retries: 0,
            last_triggered_at: None,
            last_error: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    // ── SSRF DNS 重绑定测试 ──

    /// Mock 返回内网 IP 时，try_send_webhook 应拒绝（SSRF 拦截）。
    #[tokio::test]
    async fn ssrf_blocks_mock_resolved_private_ip() {
        let resolver = MockResolver {
            addrs: vec!["10.0.0.1:443".parse().unwrap()],
        };
        let webhook = make_test_webhook("https://evil.internal/hook");
        let result = try_send_webhook(&webhook, r#"{"event":"test"}"#, &resolver).await;
        assert!(result.is_err(), "should reject private IP");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("SSRF prevention") || err_msg.contains("private"),
            "error should mention SSRF/private, got: {}",
            err_msg
        );
    }

    /// Mock 返回链路本地 IP（169.254.x.x）同样应被 SSRF 拦截。
    #[tokio::test]
    async fn ssrf_blocks_link_local_ip() {
        let resolver = MockResolver {
            addrs: vec!["169.254.169.254:80".parse().unwrap()],
        };
        let webhook = make_test_webhook("http://metadata.internal/latest");
        let result = try_send_webhook(&webhook, r#"{"event":"test"}"#, &resolver).await;
        assert!(result.is_err(), "should reject link-local IP");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("SSRF prevention") || err_msg.contains("private"),
            "error should mention SSRF/private, got: {}",
            err_msg
        );
    }

    /// Mock 返回 127.0.0.1 应被 SSRF 拦截。
    #[tokio::test]
    async fn ssrf_blocks_loopback_ip() {
        let resolver = MockResolver {
            addrs: vec!["127.0.0.1:8080".parse().unwrap()],
        };
        let webhook = make_test_webhook("http://localhost/hook");
        let result = try_send_webhook(&webhook, r#"{"event":"test"}"#, &resolver).await;
        assert!(result.is_err(), "should reject loopback IP");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("SSRF prevention") || err_msg.contains("private"),
            "error should mention SSRF/private, got: {}",
            err_msg
        );
    }

    /// Mock 返回公网 IP 时，try_send_webhook 不应因 IP 被拒绝。
    /// 注：实际 HTTP 请求会因无服务端而失败，但错误不应是 SSRF/DNS。
    #[tokio::test]
    async fn ssrf_allows_mock_resolved_public_ip() {
        let resolver = MockResolver {
            addrs: vec!["93.184.216.34:443".parse().unwrap()], // example.com
        };
        let webhook = make_test_webhook("https://example.com/hook");
        let result = try_send_webhook(&webhook, r#"{"event":"test"}"#, &resolver).await;
        // 公网 IP 不会被 SSRF 拦截，但后续 HTTP 请求会失败（连接超时/拒绝）。
        // 确保错误不包含 "SSRF prevention" 或 "private"。
        if let Err(e) = &result {
            assert!(
                !e.contains("SSRF prevention") && !e.contains("private"),
                "public IP should NOT be blocked by SSRF, but got: {}",
                e
            );
        }
        // 如果成功（极不可能，但类型安全），也视为通过
    }

    /// Mock 返回混合地址（内网+公网）时，应选择公网 IP 放行。
    #[tokio::test]
    async fn ssrf_picks_public_from_mixed_addresses() {
        let resolver = MockResolver {
            addrs: vec![
                "10.0.0.1:443".parse().unwrap(),
                "93.184.216.34:443".parse().unwrap(),
            ],
        };
        let webhook = make_test_webhook("https://example.com/hook");
        let result = try_send_webhook(&webhook, r#"{"event":"test"}"#, &resolver).await;
        // 应跳过 10.0.0.1，选择 93.184.216.34（公网）
        if let Err(e) = &result {
            assert!(
                !e.contains("SSRF prevention") && !e.contains("private"),
                "should pick public IP from mixed list, but got: {}",
                e
            );
        }
    }
}
