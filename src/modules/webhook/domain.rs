use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Webhook 配置
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Webhook {
    pub id: String,
    pub name: String,
    pub url: String,
    pub events: String,
    pub secret: Option<String>,
    pub enabled: i64,
    pub retry_count: i64,
    pub max_retries: i64,
    pub last_triggered_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Webhook 投递记录
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebhookDelivery {
    pub id: String,
    pub webhook_id: String,
    pub event: String,
    pub request_url: String,
    pub request_body: Option<String>,
    pub response_status: Option<i64>,
    pub response_body: Option<String>,
    pub duration_ms: Option<i64>,
    pub success: i64,
    pub created_at: String,
}
