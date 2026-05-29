use async_trait::async_trait;
use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use minijinja::Environment;
use std::sync::Arc;

use crate::modules::plugin::asset::{AssetPlacement, PluginAsset};
use crate::modules::plugin::Plugin;
use crate::shared::error::AppResult;
use crate::state::AppState;

#[derive(Default)]
pub struct HelloWorldPlugin;

impl HelloWorldPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for HelloWorldPlugin {
    fn name(&self) -> &str {
        "hello-world-a3f9b2c1"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    async fn init(&self, _state: &Arc<AppState>) -> AppResult<()> {
        tracing::info!(
            module = "plugin",
            plugin = "hello-world-a3f9b2c1",
            "HelloWorld plugin initialized"
        );
        Ok(())
    }

    fn api_routes(&self) -> Router<Arc<AppState>> {
        async fn hello_handler(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
            Json(serde_json::json!({
                "plugin": "hello-world-a3f9b2c1",
                "status": "ok"
            }))
        }

        Router::new().route("/api/v1/plugins/hello", get(hello_handler))
    }

    fn extend_template_env(&self, env: &mut Environment<'_>) -> AppResult<()> {
        env.add_function(
            "hello_world",
            |name: Option<String>| -> Result<String, minijinja::Error> {
                let who = name.unwrap_or_else(|| "World".to_string());
                Ok(format!("Hello, {}!", who))
            },
        );
        Ok(())
    }

    fn frontend_assets(&self) -> Vec<PluginAsset> {
        vec![PluginAsset::css(self.name(), "hello.css", AssetPlacement::Head)]
    }
}
