use std::sync::Arc;

use crate::{
    shared::{
        error::{AppError, AppResult},
        response::deleted_json,
        slug::generate_slug,
        http::require_non_empty,
    },
    state::AppState,
};

use super::{
    domain::Category,
    dto::{CreateCategoryRequest, UpdateCategoryRequest},
    repository,
};

pub async fn list_categories(state: Arc<AppState>) -> AppResult<Vec<Category>> {
    Ok(repository::list_categories(&state.pool).await?)
}

pub async fn create_category(
    state: Arc<AppState>,
    body: CreateCategoryRequest,
) -> AppResult<Category> {
    require_non_empty(&body.name, "category name")?;
    let slug = generate_slug(&body.name, body.slug.as_deref());
    if repository::category_slug_or_name_exists(&state.pool, &slug, body.name.trim(), None).await? {
        return Err(AppError::Conflict(
            "category slug or name already exists".into(),
        ));
    }
    let id = repository::insert_category(
        &state.pool,
        body.name.trim(),
        &slug,
        body.description.as_deref(),
        body.parent_id.as_deref(),
        body.sort_order.unwrap_or(0),
    )
    .await?;
    repository::get_category(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("分类未找到".to_string()))
}

pub async fn update_category(
    state: Arc<AppState>,
    id: &str,
    body: UpdateCategoryRequest,
) -> AppResult<Category> {
    let current = repository::get_category(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound(format!("分类 '{}' 未找到", id)))?;
    let name = body.name.unwrap_or(current.name.clone());
    let slug = body.slug.unwrap_or(current.slug.clone());
    if repository::category_slug_or_name_exists(&state.pool, &slug, &name, Some(id)).await? {
        return Err(AppError::Conflict(
            "category slug or name already exists".into(),
        ));
    }
    repository::update_category(
        &state.pool,
        id,
        &name,
        &slug,
        body.description
            .as_deref()
            .or(current.description.as_deref()),
        body.parent_id.as_deref().or(current.parent_id.as_deref()),
        body.sort_order.unwrap_or(current.sort_order),
    )
    .await?;
    repository::get_category(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound(format!("分类 '{}' 未找到", id)))
}

pub async fn delete_category(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    repository::delete_category(&state.pool, id).await?;
    deleted_json()
}
