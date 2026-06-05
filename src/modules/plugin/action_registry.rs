use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Action 执行状态
#[derive(Debug, Clone)]
pub enum ActionStatus {
    Spawned,
    Running,
    Done,
    Failed(String),
    Timeout,
}

/// 单个 action 的追踪记录
#[derive(Debug, Clone)]
pub struct ActionRecord {
    pub action_id: String,
    pub hook_name: String,
    pub plugin_name: String,
    pub status: ActionStatus,
    pub spawned_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Action 注册表——全局单例，追踪所有 spawned action 的生命周期
pub struct ActionRegistry {
    records: Arc<RwLock<HashMap<String, ActionRecord>>>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册一个新的 action（在 spawn 之前调用）
    pub async fn track(&self, hook_name: &str, plugin_name: &str) -> String {
        let action_id = Uuid::new_v4().to_string();
        let record = ActionRecord {
            action_id: action_id.clone(),
            hook_name: hook_name.to_string(),
            plugin_name: plugin_name.to_string(),
            status: ActionStatus::Spawned,
            spawned_at: chrono::Utc::now(),
            started_at: None,
            finished_at: None,
        };
        self.records.write().await.insert(action_id.clone(), record);
        tracing::info!(
            module = "action_registry",
            action_id = %action_id,
            hook = hook_name,
            plugin = plugin_name,
            "action spawned"
        );
        action_id
    }

    /// 标记 action 开始执行
    pub async fn mark_running(&self, action_id: &str) {
        if let Some(record) = self.records.write().await.get_mut(action_id) {
            record.status = ActionStatus::Running;
            record.started_at = Some(chrono::Utc::now());
            tracing::debug!(
                module = "action_registry",
                action_id = %action_id,
                "action running"
            );
        }
    }

    /// 标记 action 成功完成
    pub async fn mark_done(&self, action_id: &str) {
        if let Some(record) = self.records.write().await.get_mut(action_id) {
            record.status = ActionStatus::Done;
            record.finished_at = Some(chrono::Utc::now());
            tracing::info!(
                module = "action_registry",
                action_id = %action_id,
                "action done"
            );
        }
    }

    /// 标记 action 执行失败
    pub async fn mark_failed(&self, action_id: &str, error: &str) {
        if let Some(record) = self.records.write().await.get_mut(action_id) {
            record.status = ActionStatus::Failed(error.to_string());
            record.finished_at = Some(chrono::Utc::now());
            tracing::error!(
                module = "action_registry",
                action_id = %action_id,
                error = %error,
                "action failed"
            );
        }
    }

    /// 标记 action 超时
    pub async fn mark_timeout(&self, action_id: &str) {
        if let Some(record) = self.records.write().await.get_mut(action_id) {
            record.status = ActionStatus::Timeout;
            record.finished_at = Some(chrono::Utc::now());
            tracing::warn!(
                module = "action_registry",
                action_id = %action_id,
                "action timed out after 5s"
            );
        }
    }

    /// 查询某个 action 的状态
    pub async fn status(&self, action_id: &str) -> Option<ActionRecord> {
        self.records.read().await.get(action_id).cloned()
    }

    /// 惰性清理：删除 completed/failed/timeout 超过 ttl_seconds 的记录
    pub async fn cleanup_expired(&self, ttl_seconds: i64) {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(ttl_seconds);
        let mut guard = self.records.write().await;
        let before = guard.len();
        guard.retain(|_, record| match record.finished_at {
            Some(finished) if finished < cutoff => false,
            None if record.spawned_at < cutoff => false,
            _ => true,
        });
        let removed = before - guard.len();
        tracing::debug!(
            module = "action_registry",
            removed = removed,
            "cleanup expired records"
        );
    }

    /// 返回当前记录数（用于监控）
    pub async fn count(&self) -> usize {
        self.records.read().await.len()
    }
}
