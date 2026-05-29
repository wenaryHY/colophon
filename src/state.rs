use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use minijinja::Environment;
use sqlx::SqlitePool;
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::{
    bootstrap::config::AppConfig,
    modules::plugin::manager::PluginManager,
    modules::setup::domain::SetupStage,
    modules::theme::cache::TemplateContextCache,
    shared::security::LoginRateLimiter,
    ws::ServerEvent,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: AppConfig,
    pub upload_dir: PathBuf,
    pub theme_dir: PathBuf,
    pub admin_dist_dir: PathBuf,
    /// Path to the SQLite database file, for backup/restore
    pub db_path: PathBuf,
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
    /// Cached template context with TTL-based invalidation.
    pub template_cache: Arc<TemplateContextCache>,
    /// Cached MiniJinja Environment per active_theme slug (synchronous access).
    /// Stores the "base" Environment (loader + static filters + theme_assets_url)
    /// without per-request data. Cloned and extended on each request.
    pub template_env_cache: Arc<std::sync::RwLock<HashMap<String, Environment<'static>>>>,
    pub plugin_manager: Arc<tokio::sync::RwLock<PluginManager>>,
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
            theme_dir: AppConfig::resolve_path(&config.theme.theme_dir)?,
            admin_dist_dir: AppConfig::resolve_path(&config.paths.admin_dist_dir)?,
            db_path,
            pool,
            config,
            event_tx,
            site_url: Arc::new(RwLock::new(site_url)),
            admin_url: Arc::new(RwLock::new(admin_url)),
            setup_stage: Arc::new(RwLock::new(setup_stage)),
            login_rate_limiter: Arc::new(Mutex::new(LoginRateLimiter::new())),
            template_cache: Arc::new(TemplateContextCache::with_default_ttl()),
            template_env_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            plugin_manager,
        })
    }

    /// 统一失效所有模板相关缓存（切换主题或修改设置时调用）
    pub async fn invalidate_all_caches(&self) {
        self.template_cache.invalidate().await;
        self.template_env_cache.write().unwrap().clear();
    }

    pub fn backup_root_dir() -> anyhow::Result<PathBuf> {
        AppConfig::resolve_path("backups")
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
        let path = parse_sqlite_url("sqlite://inkforge.db?mode=rwc").unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with("inkforge.db"));
    }

    #[test]
    fn parse_sqlite_url_preserves_absolute_paths() {
        let expected = if cfg!(windows) {
            PathBuf::from("C:/inkforge/data.db")
        } else {
            PathBuf::from("/app/data/inkforge.db")
        };
        let url = if cfg!(windows) {
            "sqlite:///C:/inkforge/data.db?mode=rwc"
        } else {
            "sqlite:///app/data/inkforge.db?mode=rwc"
        };
        let path = parse_sqlite_url(url).unwrap();
        assert_eq!(path, expected);
    }
}
