use std::sync::Arc;

use tokio::time::{interval, Duration};

use crate::state::AppState;

use super::{hook_dispatcher, repository};

/// 启动定时发布调度器。
///
/// 每 60 秒轮询一次数据库，将到期的定时文章原子发布。
/// 使用 `tokio::time::interval` 而非 cron，因为：
/// 1. 每分钟检查一次，不需要精确到秒级的 cron 表达式
/// 2. `publish_scheduled_posts` 是原子且幂等的，重复调用安全
/// 3. 代码更简单，依赖更少
pub async fn start_post_scheduler(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(60));

    // 启动时跳过第一次 tick（避免重启时立即触发）
    tick.tick().await;

    tracing::info!(module = "post_scheduler", "post scheduler started (interval: 60s)");

    loop {
        tick.tick().await;

        match repository::publish_scheduled_posts(&state.pool).await {
            Ok(published_ids) => {
                if !published_ids.is_empty() {
                    tracing::info!(
                        count = published_ids.len(),
                        "scheduled posts published"
                    );

                    // 触发 after_publish 钩子
                    for post_id in &published_ids {
                        match repository::get_admin_post(&state.pool, post_id).await {
                            Ok(Some(post)) => {
                                // 触发 after_save 钩子（与手动发布保持一致）
                                hook_dispatcher::dispatch_post_after_save(
                                    state.as_ref(),
                                    post_id.clone(),
                                    post.title.clone(),
                                    post.slug.clone(),
                                    false, // not new
                                    "published".to_string(),
                                    "scheduled".to_string(),
                                )
                                .await;

                                // 触发 after_publish 钩子
                                hook_dispatcher::dispatch_post_after_publish(
                                    state.as_ref(),
                                    post_id.clone(),
                                    post.title.clone(),
                                    post.slug.clone(),
                                    "scheduled".to_string(),
                                    "published".to_string(),
                                )
                                .await;
                            }
                            Ok(None) => {
                                tracing::warn!(
                                    post_id = %post_id,
                                    "scheduled post published but not found in DB"
                                );
                            }
                            Err(err) => {
                                tracing::error!(
                                    post_id = %post_id,
                                    error = ?err,
                                    "failed to fetch published post for hook dispatch"
                                );
                            }
                        }
                    }
                }
            }
            Err(err) => {
                tracing::error!(
                    error = ?err,
                    "scheduled post publishing failed"
                );
            }
        }
    }
}
