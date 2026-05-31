use crate::modules::theme::{ThemeConfig, ThemeConfigSchema, ThemeManifest};
use serde::{Deserialize, Serialize};

/// 主题详情响应（包含 manifest + 当前配置 + schema）
#[derive(Debug, Serialize)]
pub struct ThemeDetailResponse {
    pub manifest: ThemeManifest,
    pub config: ThemeConfig,
    pub schema: ThemeConfigSchema,
}

/// 保存主题配置请求
#[derive(Debug, Deserialize)]
pub struct SaveThemeConfigRequest {
    pub config: ThemeConfig,
}

/// 主题上传响应
#[derive(Debug, Serialize)]
pub struct ThemeUploadResponse {
    pub slug: String,
    pub name: String,
    pub version: String,
    pub message: String,
}

/// 预览请求
#[derive(Debug, Deserialize)]
pub struct PreviewContentRequest {
    /// Markdown 内容（必填，不能为空）
    pub content: String,
    /// 内容类型: "post" | "page"，默认 "post"
    #[serde(default = "default_content_type")]
    pub content_type: String,
}

fn default_content_type() -> String {
    "post".to_string()
}

/// 主题预览请求
#[derive(Debug, Deserialize)]
pub struct PreviewThemeRequest {
    /// Markdown 内容
    pub content: String,
    /// 内容类型: "post" | "page"
    #[serde(default = "default_content_type")]
    pub content_type: String,
    /// 目标主题 slug（不传则使用当前激活主题）
    #[serde(default)]
    pub theme_slug: Option<String>,
    /// 主题配置覆写（JSON 字符串）
    #[serde(default)]
    pub theme_config: Option<String>,
}
