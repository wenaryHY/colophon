
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    /// API Key 权限：`"read_only"` 或 `"read_write"`，默认为 `"read_only"`
    #[serde(default = "default_permissions")]
    pub permissions: String,
    /// ISO 8601 格式的过期时间，例如 "2026-12-31T23:59:59Z"，None 表示永不过期
    pub expires_at: Option<String>,
}

fn default_permissions() -> String {
    "read_only".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub name: Option<String>,
}

/// 创建后返回的响应，仅此一次展示完整 key 明文
#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    #[serde(rename = "api_key")]
    pub full_key: String,
    pub permissions: String,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyListItem {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub permissions: String,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
}
