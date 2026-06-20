//! 主题文件监听器（仅开发模式启用）
//!
//! 监听 `themes/` 目录下的文件变化，自动清空模板缓存
//! （`AppState::template_cache` 与 `AppState::template_env_cache`），
//! 让开发者修改主题模板后无需重启服务即可看到效果。
//!
//! 生产模式下不启用：模板在生产环境不应被频繁改动，watcher 也存在
//! 文件句柄、线程等额外资源占用。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

/// 缓存失效防抖窗口：第一个事件到达后等待此时长，期间合并所有后续事件，
/// 避免编辑器单次保存触发的多个文件系统事件造成多次缓存清理与日志噪音。
const CACHE_INVALIDATION_DEBOUNCE: Duration = Duration::from_millis(100);

/// notify backend 的轮询间隔（仅当当前 OS 不支持原生 inotify/FSEvents/ReadDirectoryChangesW 时回退到轮询）。
const NOTIFY_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// 启动主题文件监听器。
///
/// 行为：
/// - 仅当 `runtime.mode != "production"` 时启用，生产模式直接返回 `Ok(())`，不创建任何资源。
/// - 监听 `theme_dir` 递归下所有文件变化，过滤编辑器临时文件、隐藏目录、`node_modules`。
/// - 检测到相关变化后调用 [`AppState::invalidate_all_caches`] 清空所有模板缓存。
///
/// 失败不应阻止服务启动 —— 调用方应捕获错误并降级为"修改后手动重启"模式。
pub async fn spawn_theme_watcher(state: Arc<AppState>, theme_dir: &Path) -> anyhow::Result<()> {
    if state.config.is_production() {
        tracing::info!("production mode: theme file watcher disabled");
        return Ok(());
    }

    let theme_dir = theme_dir.to_path_buf();

    // notify 回调在 watcher 内部线程同步执行，使用 unbounded 通道避免阻塞 watcher 线程。
    // 主题文件变化频率极低（仅人工编辑），通道不会积压。
    let (tx, rx) = mpsc::unbounded_channel::<Event>();

    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                if is_relevant_event_kind(&event) {
                    // 接收端关闭说明后台任务已退出，忽略发送错误
                    let _ = tx.send(event);
                }
            }
            Err(e) => {
                tracing::warn!(error = ?e, "theme file watcher received error event");
            }
        },
        Config::default().with_poll_interval(NOTIFY_POLL_INTERVAL),
    )?;

    watcher.watch(&theme_dir, RecursiveMode::Recursive)?;

    let cancel_token = state.shutdown_token.clone();
    let handle = tokio::spawn(run_cache_invalidation_loop(
        state.clone(),
        watcher,
        rx,
        theme_dir,
        cancel_token,
    ));

    *state.theme_watcher_handle.lock().await = Some(handle);

    Ok(())
}

/// 后台事件处理循环：接收 watcher 事件，按路径过滤后防抖合并，最后清空缓存。
///
/// `_watcher` 必须随循环持有，watcher 一旦被 drop，notify 内部线程退出，事件不再产生。
async fn run_cache_invalidation_loop(
    state: Arc<AppState>,
    _watcher: RecommendedWatcher,
    mut rx: mpsc::UnboundedReceiver<Event>,
    theme_dir: PathBuf,
    cancel_token: CancellationToken,
) {
    tracing::info!(
        theme_dir = %theme_dir.display(),
        "theme file watcher started (development mode)"
    );

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                tracing::info!(module = "file_watcher", "shutdown signal received, stopping theme file watcher");
                break;
            }
            first_event = rx.recv() => {
                let Some(first_event) = first_event else { break; };

                let mut changed_paths = collect_relevant_paths(&first_event);

                // 防抖：等待短暂时间后 drain 通道中累积的事件，合并为一次缓存失效。
                tokio::time::sleep(CACHE_INVALIDATION_DEBOUNCE).await;
                while let Ok(event) = rx.try_recv() {
                    changed_paths.extend(collect_relevant_paths(&event));
                }

                if changed_paths.is_empty() {
                    continue;
                }

                state.invalidate_all_caches().await;

                tracing::info!(
                    files = ?changed_paths,
                    "theme files changed, caches invalidated"
                );
            }
        }
    }

    tracing::info!("theme file watcher stopped");
}

/// 仅处理 Create/Modify/Remove 事件，忽略 Access 等噪音事件。
fn is_relevant_event_kind(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

/// 从事件中提取应触发缓存失效的路径，过滤编辑器临时文件、隐藏目录、`node_modules`。
fn collect_relevant_paths(event: &Event) -> Vec<PathBuf> {
    event
        .paths
        .iter()
        .filter(|p| should_invalidate_cache_for_path(p))
        .cloned()
        .collect()
}

/// 判断单个路径是否应触发缓存失效。
///
/// 排除：
/// - 任意路径段以 `.` 开头（如 `.git/`、`.DS_Store`、`.cache/`），但保留 `.` 与 `..` 自身
/// - Vim/Emacs swap 文件（`.swp` / `~` 结尾）与通用临时文件（`.tmp`）
/// - `node_modules` 目录下的所有文件（主题构建依赖，与运行时模板无关）
fn should_invalidate_cache_for_path(path: &Path) -> bool {
    for component in path.components() {
        let segment = component.as_os_str().to_string_lossy();
        if segment == "node_modules" {
            return false;
        }
        if segment.starts_with('.') && segment != "." && segment != ".." {
            return false;
        }
    }

    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };

    if name.ends_with('~') || name.ends_with(".swp") || name.ends_with(".tmp") {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_hidden_files_at_any_depth() {
        assert!(!should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/.DS_Store"
        )));
        assert!(!should_invalidate_cache_for_path(&PathBuf::from(
            "themes/.git/HEAD"
        )));
        assert!(!should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/.cache/foo"
        )));
    }

    #[test]
    fn skips_editor_temp_files() {
        assert!(!should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/index.html.swp"
        )));
        assert!(!should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/index.html~"
        )));
        assert!(!should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/foo.tmp"
        )));
    }

    #[test]
    fn skips_node_modules_at_any_depth() {
        assert!(!should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/node_modules/foo/bar.js"
        )));
        assert!(!should_invalidate_cache_for_path(&PathBuf::from(
            "node_modules/x"
        )));
    }

    #[test]
    fn allows_normal_template_and_static_files() {
        assert!(should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/index.html"
        )));
        assert!(should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/templates/post.html"
        )));
        assert!(should_invalidate_cache_for_path(&PathBuf::from(
            "themes/default/static/css/theme.css"
        )));
    }
}
