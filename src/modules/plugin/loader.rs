use std::collections::HashSet;
use std::path::PathBuf;
use semver::VersionReq;

use crate::shared::error::{AppError, AppResult};

use super::manifest::PluginManifest;
use super::status;

pub struct DiscoveredPlugin {
    pub manifest: PluginManifest,
    pub dir_path: PathBuf,
}

pub struct PluginLoader {
    plugin_dir: PathBuf,
    host_version: String,
}

impl PluginLoader {
    pub fn new(plugin_dir: PathBuf, host_version: &str) -> Self {
        Self {
            plugin_dir,
            host_version: host_version.to_string(),
        }
    }

    pub fn scan_manifests(&self) -> AppResult<Vec<PluginManifest>> {
        let mut manifests = Vec::new();
        let dir = match std::fs::read_dir(&self.plugin_dir) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(
                    module = "plugin",
                    dir = %self.plugin_dir.display(),
                    "plugin directory not found, creating"
                );
                std::fs::create_dir_all(&self.plugin_dir)?;
                return Ok(manifests);
            }
            Err(e) => {
                return Err(AppError::Io(e));
            }
        };

        for entry in dir {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(module = "plugin", error = %e, "failed to read plugin directory entry");
                    continue;
                }
            };

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if !file_type.is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();
            let manifest_path = entry.path().join("plugin.toml");

            if !manifest_path.exists() {
                tracing::warn!(
                    module = "plugin",
                    dir = %dir_name,
                    "missing plugin.toml, skipping"
                );
                continue;
            }

            let manifest = match PluginManifest::from_file(&manifest_path) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!(
                        module = "plugin",
                        dir = %dir_name,
                        error = %e,
                        "failed to parse plugin.toml"
                    );
                    continue;
                }
            };

            if manifest.plugin.id != dir_name {
                tracing::error!(
                    module = "plugin",
                    expected = %dir_name,
                    found = %manifest.plugin.id,
                    "plugin id mismatch with directory name"
                );
                continue;
            }

            manifests.push(manifest);
        }

        Ok(manifests)
    }

    pub fn check_version(&self, manifest: &PluginManifest) -> Result<bool, semver::Error> {
        let req_str = manifest
            .engine
            .as_ref()
            .and_then(|e| e.colophon.as_deref())
            .unwrap_or("*");

        let req = VersionReq::parse(req_str)?;
        let host_ver = semver::Version::parse(&self.host_version)?;
        Ok(req.matches(&host_ver))
    }

    pub async fn discover<E>(&self, executor: E) -> AppResult<Vec<DiscoveredPlugin>>
    where
        for<'e> E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
    {
        let manifests = self.scan_manifests()?;
        let enabled_ids = status::get_enabled_ids(executor).await?;
        let enabled_set: HashSet<String> = enabled_ids.into_iter().collect();

        let mut discovered = Vec::new();

        for manifest in manifests {
            match self.check_version(&manifest) {
                Ok(false) => {
                    tracing::warn!(
                        module = "plugin",
                        id = %manifest.plugin.id,
                        version = %manifest.plugin.version,
                        "plugin requires newer host version, skipping"
                    );
                    continue;
                }
                Err(e) => {
                    tracing::error!(
                        module = "plugin",
                        id = %manifest.plugin.id,
                        error = %e,
                        "version check failed"
                    );
                    continue;
                }
                Ok(true) => {}
            }

            if !enabled_set.is_empty() && !enabled_set.contains(&manifest.plugin.id) {
                tracing::info!(
                    module = "plugin",
                    id = %manifest.plugin.id,
                    "plugin not enabled, skipping"
                );
                continue;
            }

            status::ensure_installed(
                executor,
                &manifest.plugin.id,
                &manifest.plugin.title,
                &manifest.plugin.version,
            )
            .await?;

            let dir_path = self.plugin_dir.join(&manifest.plugin.id);
            discovered.push(DiscoveredPlugin { manifest, dir_path });
        }

        Ok(discovered)
    }
}
