use crate::modules::theme::repository;
use crate::modules::theme::{ThemeConfig, ThemeManifest};
use crate::shared::error::{AppError, AppResult};
use sqlx::SqlitePool;
use std::fs;
use std::path::PathBuf;

pub struct ThemeService {
    themes_dir: PathBuf,
}

impl ThemeService {
    pub fn new(themes_dir: PathBuf) -> Self {
        Self { themes_dir }
    }

    /// 扫描主题目录，加载所有主题清单
    pub fn scan_themes(&self) -> AppResult<Vec<ThemeManifest>> {
        let mut themes = Vec::new();

        if !self.themes_dir.exists() {
            return Ok(themes);
        }

        for entry in fs::read_dir(&self.themes_dir).map_err(|e| AppError::Io(e))? {
            let entry = entry.map_err(|e| AppError::Io(e))?;
            let path = entry.path();

            if path.is_dir() {
                match self.load_manifest(&path) {
                    Ok(manifest) => themes.push(manifest),
                    Err(e) => {
                        tracing::warn!(
                            module = "theme",
                            path = %path.display(),
                            error = %e,
                            "failed to load theme manifest, skipping"
                        );
                    }
                }
            }
        }

        Ok(themes)
    }

    /// 从主题目录加载 theme.toml 清单
    fn load_manifest(&self, theme_dir: &PathBuf) -> AppResult<ThemeManifest> {
        let manifest_path = theme_dir.join("theme.toml");

        if !manifest_path.exists() {
            return Err(AppError::BadRequest(format!(
                "theme.toml not found in {:?}",
                theme_dir
            )));
        }

        let content = fs::read_to_string(&manifest_path).map_err(|e| AppError::Io(e))?;

        let manifest: ThemeManifest = toml::from_str(&content)
            .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Failed to parse theme.toml: {}", e)))?;

        Ok(manifest)
    }

    /// 获取所有主题及其激活状态
    pub async fn list_themes(&self, pool: &SqlitePool) -> AppResult<(Vec<ThemeManifest>, String)> {
        let themes = self.scan_themes()?;
        let active_theme = repository::get_active_theme(pool).await?;

        Ok((themes, active_theme))
    }

    /// 获取主题详情及其配置
    pub async fn get_theme_detail(
        &self,
        pool: &SqlitePool,
        slug: &str,
    ) -> AppResult<(ThemeManifest, ThemeConfig)> {
        let themes = self.scan_themes()?;
        let manifest = themes
            .into_iter()
            .find(|t| t.slug == slug)
            .ok_or(AppError::NotFound(format!("主题 '{}' 未找到", slug)))?;

        let config = repository::get_config(pool, slug)
            .await?
            .unwrap_or_default();

        Ok((manifest, config))
    }

    /// 保存主题配置
    pub async fn save_theme_config(
        &self,
        pool: &SqlitePool,
        slug: &str,
        config: &ThemeConfig,
    ) -> AppResult<()> {
        // 验证主题是否存在
        let themes = self.scan_themes()?;
        if !themes.iter().any(|t| t.slug == slug) {
            return Err(AppError::NotFound(format!("主题 '{}' 未找到", slug)));
        }

        repository::save_config(pool, slug, config).await?;
        Ok(())
    }

    /// 激活主题
    pub async fn activate_theme(&self, pool: &SqlitePool, slug: &str) -> AppResult<()> {
        // 验证主题是否存在
        let themes = self.scan_themes()?;
        if !themes.iter().any(|t| t.slug == slug) {
            return Err(AppError::NotFound(format!("主题 '{}' 未找到", slug)));
        }

        repository::set_active_theme(pool, slug).await?;
        Ok(())
    }
}
