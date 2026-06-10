use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub engine: Option<EngineMeta>,
    pub hooks: Option<HooksMeta>,
    #[serde(default)]
    pub resources: Option<ResourcesMeta>,
    #[serde(default)]
    pub admin: Option<AdminMeta>,
    #[serde(default)]
    pub settings: Option<Vec<SettingDef>>,
    #[serde(default)]
    pub slots: Option<Vec<SlotDef>>,
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
    pub colophon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksMeta {
    pub template: Option<bool>,
    pub routes: Option<bool>,
    pub assets: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesMeta {
    pub admin_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminMeta {
    pub enabled: Option<bool>,
    pub entry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingDef {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub setting_type: String,
    #[serde(default)]
    pub default: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub options: Option<Vec<SettingOption>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotDef {
    pub target: String,
    pub label: String,
    pub entry: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

impl PluginManifest {
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = toml::from_str(&content)?;
        Ok(manifest)
    }
}
