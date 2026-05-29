use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub engine: Option<EngineMeta>,
    pub hooks: Option<HooksMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub id: String,
    pub title: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineMeta {
    pub inkforge: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksMeta {
    pub template: Option<bool>,
    pub routes: Option<bool>,
    pub assets: Option<Vec<String>>,
}

impl PluginManifest {
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = toml::from_str(&content)?;
        Ok(manifest)
    }
}
