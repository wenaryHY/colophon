use std::sync::Arc;

use crate::{
    shared::{
        auth::{hash_password, verify_password, AuthUser},
        error::{AppError, AppResult},
    },
    state::AppState,
};

use super::{
    domain::CurrentUser,
    dto::{UpdatePasswordRequest, UpdateProfileRequest},
    repository,
};

pub async fn me(state: Arc<AppState>, auth: &AuthUser) -> AppResult<CurrentUser> {
    repository::find_current(&state.pool, &auth.id)
        .await?
        .ok_or(AppError::Unauthorized)
}

pub async fn update_profile(
    state: Arc<AppState>,
    auth: &AuthUser,
    body: UpdateProfileRequest,
) -> AppResult<CurrentUser> {
    let current = me(state.clone(), auth).await?;
    let display_name = merge_display_name(body.display_name, &current.display_name)?;
    let bio = merge_nullable_text(body.bio, current.bio);
    let avatar_media_id = merge_nullable_text(body.avatar_media_id, current.avatar_media_id);
    let theme_preference =
        merge_theme_preference(body.theme_preference, &current.theme_preference)?;
    let language = merge_language(body.language, &current.language)?;

    repository::update_profile(
        &state.pool,
        &auth.id,
        &display_name,
        bio.as_deref(),
        avatar_media_id.as_deref(),
        &theme_preference,
        &language,
    )
    .await?;

    me(state, auth).await
}

fn merge_display_name(value: Option<String>, current: &str) -> AppResult<String> {
    let next = value.unwrap_or_else(|| current.to_string());
    let trimmed = next.trim();
    if !trimmed.is_empty() {
        return Ok(trimmed.to_string());
    }
    Err(AppError::BadRequest("display_name is required".into()))
}

fn merge_nullable_text(value: Option<Option<String>>, current: Option<String>) -> Option<String> {
    match value {
        Some(Some(next)) => {
            let trimmed = next.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(None) => None,
        None => current,
    }
}

fn merge_theme_preference(value: Option<String>, current: &str) -> AppResult<String> {
    let next = value.unwrap_or_else(|| current.to_string());
    if matches!(next.as_str(), "system" | "light" | "dark") {
        return Ok(next);
    }
    Err(AppError::BadRequest("invalid theme preference".into()))
}

fn merge_language(value: Option<String>, current: &str) -> AppResult<String> {
    let next = value.unwrap_or_else(|| current.to_string());
    if matches!(next.as_str(), "zh" | "en") {
        return Ok(next);
    }
    Err(AppError::BadRequest("invalid language".into()))
}

pub async fn update_password(
    state: Arc<AppState>,
    auth: &AuthUser,
    body: UpdatePasswordRequest,
) -> AppResult<serde_json::Value> {
    if body.new_password.len() < 8 {
        return Err(AppError::BadRequest(
            "new password must be at least 8 characters".into(),
        ));
    }

    let current_hash = repository::find_password_hash(&state.pool, &auth.id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !verify_password(&body.current_password, &current_hash).await? {
        return Err(AppError::Unauthorized);
    }

    let new_hash = hash_password(&body.new_password).await?;
    repository::update_password(&state.pool, &auth.id, &new_hash).await?;
    repository::increment_token_version(&state.pool, &auth.id).await?;
    Ok(serde_json::json!({ "updated": true }))
}
