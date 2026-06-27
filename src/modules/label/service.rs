use std::sync::Arc;

use crate::{
    shared::{
        error::AppResult,
        response::deleted_json,
    },
    state::AppState,
};

use super::{
    domain::Label,
    dto::{CreateLabelRequest, UpdateLabelRequest},
    repository,
};

pub async fn list_labels(state: Arc<AppState>) -> AppResult<Vec<Label>> {
    Ok(repository::list_labels(&state.pool).await?)
}

pub async fn create_label(state: Arc<AppState>, body: CreateLabelRequest) -> AppResult<Label> {
    let id = repository::insert_label(&state.pool, &body.name, body.color.as_deref().unwrap_or("")).await?;

    repository::get_label(&state.pool, &id)
        .await?
        .ok_or(crate::shared::error::AppError::NotFound("Label not found".to_string()))
}

pub async fn update_label(state: Arc<AppState>, id: &str, body: UpdateLabelRequest) -> AppResult<Label> {
    repository::update_label(&state.pool, id, body.name.as_deref(), body.color.as_deref()).await?;

    repository::get_label(&state.pool, id)
        .await?
        .ok_or(crate::shared::error::AppError::NotFound(format!("Label '{}' not found", id)))
}

pub async fn delete_label(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    repository::delete_label(&state.pool, id).await?;
    deleted_json()
}
