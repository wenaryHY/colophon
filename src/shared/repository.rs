use sqlx::SqlitePool;

use crate::shared::error::AppResult;

/// 可通过 slug 查询的实体
pub trait HasSlug {
    fn slug(&self) -> &str;
}

/// 通用内容 CRUD trait —— 为未来的 Schema-as-Code 提供基础抽象
///
/// 当前 category/tag 的 service 层有 slug 生成和唯一性检查的业务逻辑，
/// 本 trait 只抽象纯 CRUD，提供类型级别的契约和文档作用，
/// 暂不完全替换 service 层的定制逻辑。
#[async_trait::async_trait]
pub trait ContentRepository<T: Send + Sync + Unpin + 'static> {
    /// 列表查询（按创建时间倒序）
    async fn list(pool: &SqlitePool) -> AppResult<Vec<T>>;

    /// 按 ID 查询
    async fn get_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<T>>;

    /// 插入新记录，返回新 ID
    async fn insert(pool: &SqlitePool, params: &T) -> AppResult<String>;

    /// 更新记录
    async fn update(pool: &SqlitePool, id: &str, params: &T) -> AppResult<()>;

    /// 软删除
    async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()>;
}
