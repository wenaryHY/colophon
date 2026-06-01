use std::sync::Arc;

use slug::slugify;
use uuid::Uuid;

use crate::{
    shared::error::{AppError, AppResult},
    state::AppState,
};

use super::{
    category_domain::MediaCategory,
    dto::{CreateMediaCategoryRequest, UpdateMediaCategoryCrudRequest},
};

pub async fn list_categories(state: Arc<AppState>) -> AppResult<Vec<MediaCategory>> {
    Ok(list_categories_repo(&state.pool).await?)
}

pub async fn create_category(
    state: Arc<AppState>,
    body: CreateMediaCategoryRequest,
) -> AppResult<MediaCategory> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "media category name is required".into(),
        ));
    }

    let slug = body
        .slug
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| slugify(name));

    if media_category_slug_or_name_exists(&state.pool, &slug, name, None).await? {
        return Err(AppError::Conflict(
            "media category slug or name already exists".into(),
        ));
    }

    let id = insert_category(
        &state.pool,
        name,
        &slug,
        body.description.as_deref(),
        body.icon.as_deref(),
        body.color.as_deref(),
        body.sort_order.unwrap_or(0),
    )
    .await?;

    get_category(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn update_category(
    state: Arc<AppState>,
    id: &str,
    body: UpdateMediaCategoryCrudRequest,
) -> AppResult<MediaCategory> {
    let current = get_category(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    let name = body.name.unwrap_or(current.name.clone());
    let trimmed_name = name.trim().to_string();
    if trimmed_name.is_empty() {
        return Err(AppError::BadRequest(
            "media category name is required".into(),
        ));
    }

    let slug = body.slug.unwrap_or(current.slug.clone()).trim().to_string();
    if slug.is_empty() {
        return Err(AppError::BadRequest(
            "media category slug is required".into(),
        ));
    }

    if media_category_slug_or_name_exists(&state.pool, &slug, &trimmed_name, Some(id)).await? {
        return Err(AppError::Conflict(
            "media category slug or name already exists".into(),
        ));
    }

    update_category_repo(
        &state.pool,
        id,
        &trimmed_name,
        &slug,
        body.description
            .as_deref()
            .or(current.description.as_deref()),
        body.icon.as_deref().or(current.icon.as_deref()),
        body.color.as_deref().or(current.color.as_deref()),
        body.sort_order.unwrap_or(current.sort_order),
    )
    .await?;

    get_category(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn delete_category(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    let category = get_category(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if ["image", "audio", "video", "document", "archive", "other"].contains(&category.slug.as_str())
    {
        return Err(AppError::BadRequest(
            "default media categories cannot be deleted".into(),
        ));
    }

    sqlx::query("UPDATE media SET category = 'other' WHERE category = ?")
        .bind(&category.slug)
        .execute(&state.pool)
        .await?;

    sqlx::query(
        "UPDATE media_categories
         SET deleted_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ?",
    )
        .bind(id)
        .execute(&state.pool)
        .await?;

    Ok(serde_json::json!({ "deleted": true }))
}

pub async fn ensure_category_exists_or_resolve(
    state: &AppState,
    category: Option<&str>,
    ext: &str,
) -> AppResult<String> {
    if let Some(category) = category.map(|v| v.trim()).filter(|v| !v.is_empty()) {
        let exists: i64 =
            sqlx::query_scalar(
                "SELECT COUNT(1) FROM media_categories WHERE (id = ? OR slug = ?) AND deleted_at IS NULL",
            )
                .bind(category)
                .bind(category)
                .fetch_one(&state.pool)
                .await?;
        if exists == 0 {
            return Err(AppError::BadRequest("media category does not exist".into()));
        }
        return Ok(resolve_category_slug(&state.pool, category).await?);
    }

    Ok(infer_category_by_extension(ext))
}

fn infer_category_by_extension(ext: &str) -> String {
    match ext {
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "svg" | "bmp" => "image",
        "mp3" | "ogg" | "wav" | "m4a" | "flac" => "audio",
        "mp4" | "mov" | "avi" | "mkv" | "webm" => "video",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" => "document",
        "zip" | "rar" | "7z" | "tar" | "gz" => "archive",
        _ => "other",
    }
    .to_string()
}

async fn list_categories_repo<'e, E>(executor: E) -> Result<Vec<MediaCategory>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, MediaCategory>(
        "SELECT * FROM media_categories WHERE deleted_at IS NULL ORDER BY sort_order ASC, name ASC",
    )
    .fetch_all(executor)
    .await
}

async fn get_category<'e, E>(executor: E, id: &str) -> Result<Option<MediaCategory>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, MediaCategory>(
        "SELECT * FROM media_categories WHERE (id = ? OR slug = ?) AND deleted_at IS NULL LIMIT 1",
    )
    .bind(id)
    .bind(id)
    .fetch_optional(executor)
    .await
}

async fn resolve_category_slug<'e, E>(executor: E, id_or_slug: &str) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar::<_, String>(
        "SELECT slug FROM media_categories WHERE (id = ? OR slug = ?) AND deleted_at IS NULL LIMIT 1",
    )
    .bind(id_or_slug)
    .bind(id_or_slug)
    .fetch_one(executor)
    .await
}

async fn media_category_slug_or_name_exists<'e, E>(
    executor: E,
    slug: &str,
    name: &str,
    exclude_id: Option<&str>,
) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(exclude_id) = exclude_id {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM media_categories WHERE (slug = ? OR name = ?) AND id != ? AND deleted_at IS NULL)",
        )
        .bind(slug)
        .bind(name)
        .bind(exclude_id)
        .fetch_one(executor)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM media_categories WHERE (slug = ? OR name = ?) AND deleted_at IS NULL)",
        )
        .bind(slug)
        .bind(name)
        .fetch_one(executor)
        .await
    }
}

async fn insert_category<'e, E>(
    executor: E,
    name: &str,
    slug: &str,
    description: Option<&str>,
    icon: Option<&str>,
    color: Option<&str>,
    sort_order: i64,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO media_categories (id, name, slug, description, icon, color, sort_order)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(icon)
    .bind(color)
    .bind(sort_order)
    .execute(executor)
    .await?;
    Ok(id)
}

async fn update_category_repo<'e, E>(
    executor: E,
    id: &str,
    name: &str,
    slug: &str,
    description: Option<&str>,
    icon: Option<&str>,
    color: Option<&str>,
    sort_order: i64,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE media_categories
         SET name = ?, slug = ?, description = ?, icon = ?, color = ?, sort_order = ?, updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(icon)
    .bind(color)
    .bind(sort_order)
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}
