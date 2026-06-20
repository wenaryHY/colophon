use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use minijinja::Environment;
use sqlx::SqlitePool;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use tokio_cron_scheduler::JobScheduler;

use crate::{
    bootstrap::config::AppConfig, modules::plugin::manager::PluginManager,
    modules::setup::domain::SetupStage, modules::theme::cache::TemplateContextCache,
    shared::security::LoginRateLimiter, ws::ServerEvent,
};

/// 静态资源版本控制 manifest，从构建时生成的 JSON 加载
#[derive(Clone)]
pub struct AssetManifest {
    map: HashMap<String, String>,
}

impl AssetManifest {
    /// 从编译期嵌入的 JSON 加载 manifest
    pub fn load() -> Self {
        const MANIFEST_JSON: &str = include_str!("../target/generated/asset-manifest.json");
        let map: HashMap<String, String> = serde_json::from_str(MANIFEST_JSON).unwrap_or_default();
        Self { map }
    }

    /// 将主题路径和文件路径转换为带版本号的 URL 路径
    ///
    /// # 参数
    /// - `theme`: 主题名称（如 "default"）
    /// - `path`: 相对于 static 目录的路径（如 "css/theme.css"）
    ///
    /// # 返回
    /// 带版本号的路径，如 "css/theme.css?v=abc12345"
    /// 若未找到对应文件，降级返回原始路径（不带版本号）
    pub fn resolve(&self, theme: &str, path: &str) -> String {
        let key = format!("{}/{}", theme, path);
        self.map
            .get(&key)
            .cloned()
            .unwrap_or_else(|| path.to_string())
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: AppConfig,
    pub upload_dir: PathBuf,
    pub static_dir: PathBuf,
    pub theme_dir: PathBuf,
    pub admin_dist_dir: PathBuf,
    /// Path to the SQLite database file, for backup/restore
    pub db_path: PathBuf,
    /// Backup directory path (default: "backups", override in tests)
    pub backup_dir: PathBuf,
    /// Broadcast sender for WebSocket real-time notifications (容量 256)
    pub event_tx: broadcast::Sender<ServerEvent>,
    /// Cached site_url from DB, updated on setting change.
    /// Used by CORS, SEO, and theme rendering without hitting DB every request.
    pub site_url: Arc<RwLock<String>>,
    /// Cached admin_url from DB for redirects and theme entry links.
    pub admin_url: Arc<RwLock<String>>,
    /// Cached setup stage used by entry routing and auth guards.
    pub setup_stage: Arc<RwLock<SetupStage>>,
    /// In-memory login rate limiter for basic brute-force protection.
    pub login_rate_limiter: Arc<Mutex<LoginRateLimiter>>,
    /// In-memory comment rate limiter: user_id → last comment Instant.
    /// Replaces per-request DB query for rate checking.
    pub comment_rate_limiter: Arc<Mutex<HashMap<String, Instant>>>,
    /// Cached template context with TTL-based invalidation.
    pub template_cache: Arc<TemplateContextCache>,
    /// Cached MiniJinja Environment per active_theme slug (synchronous access).
    /// Stores the "base" Environment (loader + static filters + theme_assets_url)
    /// without per-request data. Cloned and extended on each request.
    pub template_env_cache: Arc<RwLock<HashMap<String, Environment<'static>>>>,
    pub plugin_manager: Arc<tokio::sync::RwLock<PluginManager>>,
    /// Handle to the backup cron scheduler, stored for dynamic stop/restart.
    pub backup_scheduler: Arc<tokio::sync::Mutex<Option<JobScheduler>>>,
    /// 静态资源版本控制 manifest，构建时生成
    pub asset_manifest: Arc<AssetManifest>,
    /// 全局取消信号。关闭时 cancel() 通知所有后台任务退出。
    /// 文件监听器、垃圾清理调度器等通过 clone() 订阅。
    pub shutdown_token: CancellationToken,
    /// 回收站清理调度器的 JoinHandle，关闭时用于等待或 abort
    pub trash_scheduler_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 文件监听器的 JoinHandle，关闭时用于等待或 abort
    pub theme_watcher_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// WebP 转换器 channel 的发送端。
    /// 上传图片后通过此 channel 向后台 worker 投递转换任务。
    /// 当 `config.media.webp_enabled == false` 时为 None（worker 不启动）。
    pub converter_send: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::Sender<crate::modules::media::worker::ConversionJob>>>>,
    /// WebP worker 的 JoinHandle，关闭时用于等待排空或 abort。
    pub webp_worker_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        pool: SqlitePool,
        event_tx: broadcast::Sender<ServerEvent>,
        site_url: String,
        admin_url: String,
        setup_stage: SetupStage,
        plugin_manager: Arc<tokio::sync::RwLock<PluginManager>>,
    ) -> anyhow::Result<Self> {
        let db_path = parse_sqlite_url(&config.database.url)?;
        Ok(Self {
            upload_dir: AppConfig::resolve_path(&config.storage.upload_dir)?,
            static_dir: AppConfig::resolve_path(&config.storage.static_dir)?,
            theme_dir: AppConfig::resolve_path(&config.theme.theme_dir)?,
            admin_dist_dir: AppConfig::resolve_path(&config.paths.admin_dist_dir)?,
            db_path,
            backup_dir: AppConfig::resolve_path("backups")?,
            pool,
            config,
            event_tx,
            site_url: Arc::new(RwLock::new(site_url)),
            admin_url: Arc::new(RwLock::new(admin_url)),
            setup_stage: Arc::new(RwLock::new(setup_stage)),
            login_rate_limiter: Arc::new(Mutex::new(LoginRateLimiter::new())),
            comment_rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            template_cache: Arc::new(TemplateContextCache::with_default_ttl()),
            template_env_cache: Arc::new(RwLock::new(HashMap::new())),
            plugin_manager,
            backup_scheduler: Arc::new(tokio::sync::Mutex::new(None)),
            asset_manifest: Arc::new(AssetManifest::load()),
            shutdown_token: CancellationToken::new(),
            trash_scheduler_handle: Arc::new(tokio::sync::Mutex::new(None)),
            theme_watcher_handle: Arc::new(tokio::sync::Mutex::new(None)),
            converter_send: Arc::new(tokio::sync::Mutex::new(None)),
            webp_worker_handle: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    /// 统一失效所有模板相关缓存（切换主题或修改设置时调用）
    pub async fn invalidate_all_caches(&self) {
        self.template_cache.invalidate().await;
        self.template_env_cache.write().await.clear();
    }

    pub fn backup_root_dir(&self) -> PathBuf {
        self.backup_dir.clone()
    }
}

fn normalize_sqlite_path(raw: &str) -> &str {
    if cfg!(windows) && raw.starts_with('/') && raw.chars().nth(2) == Some(':') {
        &raw[1..]
    } else {
        raw
    }
}

/// 从 sqlite:// URL 中提取文件路径（支持相对和绝对路径）
fn parse_sqlite_url(url: &str) -> anyhow::Result<PathBuf> {
    let raw = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))
        .unwrap_or(url);
    let raw = raw.split('?').next().unwrap_or(raw);
    let path = PathBuf::from(normalize_sqlite_path(raw));
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(&path))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_sqlite_url;
    use std::path::PathBuf;

    #[test]
    fn parse_sqlite_url_resolves_relative_paths() {
        let path = parse_sqlite_url("sqlite://colophon.db?mode=rwc").unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with("colophon.db"));
    }

    #[test]
    fn parse_sqlite_url_preserves_absolute_paths() {
        let expected = if cfg!(windows) {
            PathBuf::from("C:/colophon/data.db")
        } else {
            PathBuf::from("/app/data/colophon.db")
        };
        let url = if cfg!(windows) {
            "sqlite:///C:/colophon/data.db?mode=rwc"
        } else {
            "sqlite:///app/data/colophon.db?mode=rwc"
        };
        let path = parse_sqlite_url(url).unwrap();
        assert_eq!(path, expected);
    }
}
