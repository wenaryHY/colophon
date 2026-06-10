use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 主题配置字段定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ThemeConfigField {
    #[serde(rename = "text")]
    Text {
        label: String,
        default: Option<String>,
    },
    #[serde(rename = "color")]
    Color {
        label: String,
        default: Option<String>,
    },
    #[serde(rename = "select")]
    Select {
        label: String,
        default: Option<String>,
        options: Vec<SelectOption>,
    },
    #[serde(rename = "number")]
    Number { label: String, default: Option<i32> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub label: String,
    pub value: String,
}

/// 主题配置 Schema
pub type ThemeConfigSchema = HashMap<String, ThemeConfigField>;

/// 主题配置值
pub type ThemeConfig = HashMap<String, serde_json::Value>;

/// 主题清单（从 theme.toml 读取）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeManifest {
    pub name: String,
    pub slug: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub min_colophon_version: String,
    #[serde(default)]
    pub preview_image: Option<String>,
    #[serde(default)]
    pub config: ThemeConfigSchema,
}

/// 主题摘要（后台展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSummary {
    pub active: bool,
    pub manifest: ThemeManifest,
}
