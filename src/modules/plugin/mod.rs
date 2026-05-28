use async_trait::async_trait;
use axum::Router;
use minijinja::Environment;
use std::sync::Arc;

use crate::shared::error::AppResult;
use crate::state::AppState;

pub mod registry;
pub mod manager;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    async fn init(&self, _state: &Arc<AppState>) -> AppResult<()> {
        Ok(())
    }

    async fn shutdown(&self) -> AppResult<()> {
        Ok(())
    }

    fn api_routes(&self) -> Router<Arc<AppState>> {
        Router::new()
    }

    fn extend_template_env(&self, _env: &mut Environment<'_>) -> AppResult<()> {
        Ok(())
    }
}
