use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub storage: StorageConfig,
    pub theme: ThemeConfig,
    pub paths: PathsConfig,
    pub runtime: RuntimeConfig,
    pub webhook: WebhookConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub secret: String,
    /// 访问令牌默认存活时长（秒），必须为正整数
    pub expires_in_seconds: u64,
    pub allow_insecure_default_secret: bool,
    /// Cloudflare Turnstile secret key（可选，为空则跳过验证）
    #[serde(default)]
    pub turnstile_secret: String,
    /// Cloudflare Turnstile 前端 site key（可选，为空则不渲染 widget）
    #[serde(default)]
    pub turnstile_site_key: String,
    /// 是否给 cookie 加 Secure 标记（默认 false；ACME 成功后自动改 true，或手动设环境变量）
    #[serde(default)]
    pub cookie_secure: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub upload_dir: String,
    pub max_upload_size_mb: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ThemeConfig {
    pub theme_dir: String,
    pub active_theme_fallback: String,
    pub default_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    pub admin_dist_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeConfig {
    pub mode: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebhookConfig {
    #[serde(default = "default_webhook_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_webhook_timeout_seconds")]
    pub timeout_seconds: u64,
}

fn default_webhook_max_concurrency() -> usize { 5 }
fn default_webhook_timeout_seconds() -> u64 { 60 }

impl AppConfig {
    /// 是否为生产模式（运行时判断，非编译期）
    pub fn is_production(&self) -> bool {
        self.runtime.mode.eq_ignore_ascii_case("production")
    }

    /// cookie 是否加 Secure 标记（独立于 runtime.mode，由 INKFORGE__AUTH__COOKIE_SECURE 控制）
    pub fn cookie_secure(&self) -> bool {
        self.auth.cookie_secure
    }

    pub fn load() -> Result<Self> {
        Ok(config::Config::builder()
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(config::File::with_name("config/local").required(false))
            .add_source(config::Environment::with_prefix("INKFORGE").separator("__"))
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 2000)?
            .set_default("database.url", "sqlite://inkforge.db?mode=rwc")?
            .set_default("auth.secret", "change-me-in-production-please")?
            .set_default("auth.expires_in_seconds", 900)?
            .set_default("auth.allow_insecure_default_secret", false)?
            .set_default("auth.turnstile_secret", "")?
            .set_default("auth.turnstile_site_key", "")?
            .set_default("auth.cookie_secure", false)?
            .set_default("storage.upload_dir", "uploads")?
            .set_default("storage.max_upload_size_mb", 10)?
            .set_default("theme.theme_dir", "themes")?
            .set_default("theme.active_theme_fallback", "default")?
            .set_default("theme.default_mode", "system")?
            .set_default("paths.admin_dist_dir", "src/admin/dist")?
            .set_default("runtime.mode", "development")?
            .set_default("webhook.max_concurrency", 5)?
            .set_default("webhook.timeout_seconds", 60)?
            .build()?
            .try_deserialize()?)
    }

    pub fn validate(&self) -> Result<()> {
        const UNSAFE_SECRETS: &[&str] = &[
            "inkforge-change-me-in-production",
            "change-me-in-production-please",
        ];
        if !UNSAFE_SECRETS.contains(&self.auth.secret.as_str()) {
            return Ok(());
        }

        if self.is_production()
            && !self.auth.allow_insecure_default_secret
        {
            bail!(
                "unsafe default JWT secret is blocked in production; set INKFORGE__AUTH__SECRET or explicitly set INKFORGE__AUTH__ALLOW_INSECURE_DEFAULT_SECRET=true"
            );
        }

        tracing::warn!(
            "⚠️  JWT secret is using default value. Set INKFORGE__AUTH__SECRET before any non-development deployment."
        );
        Ok(())
    }

    pub fn resolve_path(raw: &str) -> Result<PathBuf> {
        let path = Path::new(raw);
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }

        Ok(std::env::current_dir()?.join(path))
    }
}
