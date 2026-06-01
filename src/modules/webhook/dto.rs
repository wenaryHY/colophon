use serde::Deserialize;

/// 创建 Webhook 请求
#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub url: String,
    #[serde(default = "default_events")]
    pub events: String,
    pub secret: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_retries")]
    pub max_retries: i64,
}

/// 更新 Webhook 请求
#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub events: Option<String>,
    pub secret: Option<String>,
    pub enabled: Option<bool>,
    pub max_retries: Option<i64>,
}

fn default_events() -> String {
    "post.after_publish".to_string()
}

fn default_enabled() -> bool {
    true
}

fn default_max_retries() -> i64 {
    3
}
