use std::sync::Arc;

use crate::{
    shared::{
        error::{AppError, AppResult},
        response::deleted_json,
        slug::generate_slug,
    },
    state::AppState,
};

use super::{
    domain::Tag,
    dto::{CreateTagRequest, UpdateTagRequest},
    repository,
};

pub async fn list_tags(state: Arc<AppState>) -> AppResult<Vec<Tag>> {
    Ok(repository::list_tags(&state.pool).await?)
}

pub async fn create_tag(state: Arc<AppState>, body: CreateTagRequest) -> AppResult<Tag> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("tag name is required".into()));
    }

    let slug = generate_slug(&body.name, body.slug.as_deref());

    if repository::tag_slug_or_name_exists(&state.pool, &slug, body.name.trim(), None).await? {
        return Err(AppError::Conflict("tag slug or name already exists".into()));
    }

    let id = repository::insert_tag(&state.pool, body.name.trim(), &slug).await?;

    repository::get_tag(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn update_tag(state: Arc<AppState>, id: &str, body: UpdateTagRequest) -> AppResult<Tag> {
    let existing = repository::get_tag(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    let name = body
        .name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let slug = body
        .slug
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if name.is_none() && slug.is_none() {
        return Ok(existing);
    }

    let final_name = name.unwrap_or(&existing.name);
    let generated_slug;
    let final_slug = if let Some(s) = slug {
        s
    } else if name.is_some() {
        generated_slug = generate_slug(final_name, None);
        &generated_slug
    } else {
        &existing.slug
    };

    if repository::tag_slug_or_name_exists(&state.pool, final_slug, final_name, Some(id)).await? {
        return Err(AppError::Conflict("tag slug or name already exists".into()));
    }

    repository::update_tag(&state.pool, id, name, Some(final_slug)).await?;

    repository::get_tag(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn delete_tag(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    repository::delete_tag(&state.pool, id).await?;
    deleted_json()
}
