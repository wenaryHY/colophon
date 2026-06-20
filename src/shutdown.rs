//! 优雅关闭模块。
//!
//! 全局关闭序列（跨 lib.rs 和本模块）：
//!   1. 等待 OS 信号（SIGTERM / SIGINT / Ctrl+C）—— lib.rs
//!   2. 通知 axum 停止接受新连接，等待 drain —— lib.rs
//!   3. drain 超时兜底 —— lib.rs
//!   4. 调用本模块的 run_shutdown_sequence() 执行清理 —— 以下步骤在本模块：
//!     a. cancel 全局 token，通知所有后台任务
//!     b. 停止备份调度器（shutdown）
//!     c. 等待 WebP worker 排空
//!     d. 停止垃圾清理调度器（CancelToken 优雅退出 + abort 兜底）
//!     e. 停止文件监听器（CancelToken 优雅退出 + abort 兜底）
//!     f. 关闭所有插件（逐个超时保护）
//!     g. 关闭数据库连接池

use std::sync::Arc;
use std::time::Duration;
use crate::state::AppState;

/// 单个插件 shutdown 的超时
const PLUGIN_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// CancelToken 优雅退出等待时间
const CANCEL_TOKEN_GRACE_PERIOD: Duration = Duration::from_secs(2);

/// 关闭序列完成后额外等待时间，确保日志刷新
const FINAL_DRAIN_MS: u64 = 100;

/// 监听 OS 进程信号（SIGTERM / SIGINT / Ctrl+C），返回时表示收到关闭信号。
///
/// Unix: 同时监听 SIGTERM 和 SIGINT，以先到者为准。
/// Windows: 仅监听 Ctrl+C。
pub async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()
        ).expect("failed to register SIGTERM handler");
        let mut sigint = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::interrupt()
        ).expect("failed to register SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!(module = "shutdown", event = "sigterm", "received SIGTERM");
            }
            _ = sigint.recv() => {
                tracing::info!(module = "shutdown", event = "sigint", "received SIGINT");
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.expect("failed to listen for Ctrl+C");
        tracing::info!(module = "shutdown", event = "ctrl_c", "received Ctrl+C");
    }

    tracing::info!(module = "shutdown", "starting graceful shutdown sequence");
}

/// 执行有序关闭清理序列。
///
/// 应在 axum serve task 完成（或超时）后调用。
/// 所有步骤使用 tokio::time::timeout 保护，防止单步卡死。
pub async fn run_shutdown_sequence(state: &Arc<AppState>) {
    let step_timeout = Duration::from_secs(5);

    // Step 1: cancel 全局 token，通知所有后台任务
    state.shutdown_token.cancel();
    tracing::info!(module = "shutdown", step = 1, "cancel_token triggered");

    // Step 2: 停止备份调度器
    tracing::info!(module = "shutdown", step = 2, "stopping backup scheduler...");
    match tokio::time::timeout(
        step_timeout,
        crate::modules::backup::scheduler::stop_backup_scheduler(state),
    ).await {
        Ok(()) => tracing::info!(module = "shutdown", step = 2, "backup scheduler stopped"),
        Err(_) => tracing::warn!(module = "shutdown", step = 2, "backup scheduler stop timed out"),
    }

    // Step 2.5: 等待 WebP worker 排空（让当前正在转换的任务完成）
    {
        let mut guard = state.webp_worker_handle.lock().await;
        if let Some(handle) = guard.take() {
            tracing::info!(module = "shutdown", step = 2.5, "waiting for webp worker to drain...");
            let abort_handle = handle.abort_handle();
            let drain_timeout = Duration::from_secs(60); // 给大图转换足够时间
            if tokio::time::timeout(drain_timeout, async {
                let _ = handle.await;
            }).await.is_err() {
                tracing::warn!(module = "shutdown", step = 2.5, "webp worker drain timed out, aborting");
                abort_handle.abort();
            } else {
                tracing::info!(module = "shutdown", step = 2.5, "webp worker drained");
            }
        }
    }

    // Step 3: 停止垃圾清理调度器
    tracing::info!(module = "shutdown", step = 3, "stopping trash scheduler...");
    if let Some(handle) = state.trash_scheduler_handle.lock().await.take() {
        // 提前获取 AbortHandle，避免 handle 被 move 后无法 abort
        let abort_handle = handle.abort_handle();
        if tokio::time::timeout(CANCEL_TOKEN_GRACE_PERIOD, async {
            let _ = handle.await;
        }).await.is_err() {
            tracing::warn!(module = "shutdown", step = 3, "trash scheduler did not exit gracefully, aborting");
            abort_handle.abort();
        } else {
            tracing::info!(module = "shutdown", step = 3, "trash scheduler stopped gracefully");
        }
    }

    // Step 4: 停止文件监听器
    tracing::info!(module = "shutdown", step = 4, "stopping theme file watcher...");
    if let Some(handle) = state.theme_watcher_handle.lock().await.take() {
        let abort_handle = handle.abort_handle();
        if tokio::time::timeout(CANCEL_TOKEN_GRACE_PERIOD, async {
            let _ = handle.await;
        }).await.is_err() {
            tracing::warn!(module = "shutdown", step = 4, "theme file watcher did not exit gracefully, aborting");
            abort_handle.abort();
        } else {
            tracing::info!(module = "shutdown", step = 4, "theme file watcher stopped gracefully");
        }
    }

    // Step 5: 关闭插件（逐个超时保护）
    tracing::info!(module = "shutdown", step = 5, "shutting down plugins...");
    let manager = state.plugin_manager.read().await;
    for plugin in manager.plugins() {
        let plugin_name = plugin.name().to_string();
        match tokio::time::timeout(PLUGIN_SHUTDOWN_TIMEOUT, plugin.shutdown()).await {
            Ok(Ok(())) => {
                tracing::info!(module = "shutdown", plugin = %plugin_name, "plugin shut down");
            }
            Ok(Err(e)) => {
                tracing::error!(module = "shutdown", plugin = %plugin_name, error = %e, "plugin shutdown failed");
            }
            Err(_elapsed) => {
                tracing::error!(module = "shutdown", plugin = %plugin_name, "plugin shutdown timed out");
            }
        }
    }
    drop(manager);
    tracing::info!(module = "shutdown", step = 5, "all plugins processed");

    // Step 6: 关闭数据库连接池
    tracing::info!(module = "shutdown", step = 6, "closing database pool...");
    match tokio::time::timeout(step_timeout, async {
        state.pool.close().await;
    }).await {
        Ok(()) => tracing::info!(module = "shutdown", step = 6, "database pool closed"),
        Err(_) => tracing::warn!(module = "shutdown", step = 6, "database pool close timed out"),
    }

    // 等待短暂清盘
    tokio::time::sleep(Duration::from_millis(FINAL_DRAIN_MS)).await;
    tracing::info!(module = "shutdown", "graceful shutdown complete");
}
