
use serde::Serialize;
use sqlx::FromRow;

/// API Key 数据库实体，hash 存储，不存明文
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub permissions: String,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 仅含 user 基本信息的 ApiKey 查询结果，用于认证时构造 AuthUser
#[derive(Debug, Clone, FromRow)]
pub struct ApiKeyWithUser {
    pub api_key_id: String,
    pub user_id: String,
    pub username: String,
    pub role: String,
    pub permissions: String,
    pub api_key_expires_at: Option<String>,
}
