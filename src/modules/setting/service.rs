use std::sync::Arc;

use crate::{
    shared::error::{AppError, AppResult},
    state::AppState,
};

use super::{
    dto::{SettingItem, UpdateSettingRequest},
    repository,
    validator::{
        canonical_admin_url_from_site_url, normalize_admin_url, normalize_bool_string,
        normalize_site_url,
    },
};

const ALLOWED_SETTINGS: &[&str] = &[
    "site_title",
    "site_description",
    "site_url",
    "admin_url",
    "allow_register",
    "allow_comment",
    "comment_require_login",
    "comment_moderation_mode",
    "comment_trusted_after_approved",
    "comment_max_length",
    "active_theme",
    "theme_default_mode",
];

pub async fn list_settings(state: Arc<AppState>) -> AppResult<Vec<SettingItem>> {
    Ok(repository::list(&state.pool).await?)
}

pub async fn update_setting(
    state: Arc<AppState>,
    body: UpdateSettingRequest,
) -> AppResult<serde_json::Value> {
    if !ALLOWED_SETTINGS.contains(&body.key.as_str()) {
        return Err(AppError::BadRequest("setting key is not writable".into()));
    }

    match body.key.as_str() {
        "site_url" => {
            let site_url = normalize_site_url(&body.value)?;
            let admin_url = canonical_admin_url_from_site_url(&site_url)?;
            repository::upsert(&state.pool, "site_url", &site_url).await?;
            repository::upsert(&state.pool, "admin_url", &admin_url).await?;
            *state.site_url.write().await = site_url;
            *state.admin_url.write().await = admin_url;
        }
        "admin_url" => {
            return Err(AppError::BadRequest(
                "admin_url is derived from site_url. Update site_url instead.".into(),
            ));
        }
        _ => {
            let value = normalize_setting_value(&body.key, &body.value)?;
            repository::upsert(&state.pool, &body.key, &value).await?;
        }
    }

    state.invalidate_all_caches().await;
    Ok(serde_json::json!({ "updated": true }))
}

fn normalize_setting_value(key: &str, value: &str) -> AppResult<String> {
    match key {
        "site_url" => normalize_site_url(value),
        "admin_url" => normalize_admin_url(value),
        "allow_register" | "allow_comment" | "comment_require_login" => {
            normalize_bool_string(value, key)
        }
        "trash_retention_days" => normalize_i64_range(value, key, 1, 90),
        "trash_cleanup_hour" => normalize_i64_range(value, key, 0, 23),
        "trash_cleanup_minute" => normalize_i64_range(value, key, 0, 59),
        _ => Ok(value.trim().to_string()),
    }
}

fn normalize_i64_range(value: &str, key: &str, min: i64, max: i64) -> AppResult<String> {
    let parsed = value
        .trim()
        .parse::<i64>()
        .map_err(|_| AppError::BadRequest(format!("{key} must be an integer")))?;
    if (min..=max).contains(&parsed) {
        return Ok(parsed.to_string());
    }
    Err(AppError::BadRequest(format!(
        "{key} must be between {min} and {max}"
    )))
}
