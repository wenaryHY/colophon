use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssetKind {
    #[serde(rename = "css")]
    Css,
    #[serde(rename = "js")]
    Js,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssetPlacement {
    #[serde(rename = "head")]
    Head,
    #[serde(rename = "body")]
    Body,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAsset {
    pub plugin_slug: String,
    pub path: String,
    pub kind: AssetKind,
    pub placement: AssetPlacement,
}

impl PluginAsset {
    pub fn css(plugin_slug: &str, path: &str, placement: AssetPlacement) -> Self {
        Self {
            plugin_slug: plugin_slug.to_string(),
            path: path.to_string(),
            kind: AssetKind::Css,
            placement,
        }
    }

    pub fn js(plugin_slug: &str, path: &str, placement: AssetPlacement) -> Self {
        Self {
            plugin_slug: plugin_slug.to_string(),
            path: path.to_string(),
            kind: AssetKind::Js,
            placement,
        }
    }

    pub fn render_html(&self) -> String {
        let url = format!("/static/plugins/{}/{}", self.plugin_slug, self.path);
        match self.kind {
            AssetKind::Css => format!(r#"<link rel="stylesheet" href="{}">"#, url),
            AssetKind::Js => format!(r#"<script src="{}"></script>"#, url),
        }
    }
}
