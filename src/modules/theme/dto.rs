use crate::modules::post::post_types::ContentType;
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
    /// 内容类型: post 或 page，默认 post
    #[serde(default)]
    pub content_type: ContentType,
}

/// 主题预览请求
#[derive(Debug, Deserialize)]
pub struct PreviewThemeRequest {
    /// Markdown 内容
    pub content: String,
    /// 内容类型: post 或 page
    #[serde(default)]
    pub content_type: ContentType,
    /// 目标主题 slug（不传则使用当前激活主题）
    #[serde(default)]
    pub theme_slug: Option<String>,
    /// 主题配置覆写（JSON 字符串）
    #[serde(default)]
    pub theme_config: Option<String>,
}

/// 前台搜索页查询参数
#[derive(Debug, Deserialize)]
pub struct SearchPageQuery {
    #[serde(default)]
    pub keyword: String,
    #[serde(default = "default_search_page")]
    pub page: u32,
}

fn default_search_page() -> u32 { 1 }
