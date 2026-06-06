use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
    Json,
};

use crate::{
    shared::{auth::AdminUser, error::AppError, response::ApiResponse},
    state::AppState,
};

use super::{
    domain::BackupProvider,
    dto::{BackupScheduleRequest, CreateBackupRequest},
    merge, service,
};

pub async fn create_backup(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Json(body): Json<CreateBackupRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let provider = BackupProvider::from_str(&body.provider)
        .ok_or_else(|| AppError::BadRequest("provider 仅支持 local / s3".into()))?;
    let data = service::create_backup(state, provider).await?;
    Ok(Json(ApiResponse::success(data)))
}

pub async fn list_backups(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> Result<Json<ApiResponse<Vec<super::dto::BackupListResponse>>>, AppError> {
    let data = service::list_backups(state).await?;
    Ok(Json(ApiResponse::success(data)))
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    admin: AdminUser,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<ApiResponse<Vec<super::dto::RestoreProgressResponse>>>, AppError> {
    let _ = admin;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;

    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            file_name = field.file_name().map(str::to_string);
            file_bytes = Some(field.bytes().await?.to_vec());
            break;
        }
    }

    let bytes = file_bytes.ok_or_else(|| {
        tracing::warn!(module = "backup", event = "restore_upload_missing_file_field");
        AppError::BadRequest("missing backup file field".into())
    })?;
    tracing::info!(
        module = "backup",
        event = "restore_upload_received",
        file_name = file_name.as_deref().unwrap_or("-"),
        size = bytes.len()
    );

    let data = service::restore_backup_from_bytes(state, bytes).await?;
    Ok(Json(ApiResponse::success(data)))
}

pub async fn get_schedule(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> Result<Json<ApiResponse<super::dto::BackupScheduleResponse>>, AppError> {
    let data = service::get_schedule(state).await?;
    Ok(Json(ApiResponse::success(data)))
}

pub async fn update_schedule(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Json(body): Json<BackupScheduleRequest>,
) -> Result<Json<ApiResponse<super::dto::BackupScheduleResponse>>, AppError> {
    let data = service::update_schedule(state, body).await?;
    Ok(Json(ApiResponse::success(data)))
}

pub async fn delete_backup(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    reject_path_traversal(&id)?;
    let data = service::delete_backup(state, id).await?;
    Ok(Json(ApiResponse::success(data)))
}

pub async fn download_backup(
    State(_state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    reject_path_traversal(&id)?;
    let backup_path = AppState::backup_root_dir()?.join(&id).join("backup.zip");

    if !backup_path.exists() {
        return Err(AppError::NotFound);
    }

    let file: Vec<u8> = tokio::fs::read(&backup_path).await?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"backup.zip\"",
            ),
        ],
        file,
    ))
}

pub async fn merge_restore_backup(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<super::dto::RestoreProgressResponse>>>, AppError> {
    reject_path_traversal(&id)?;
    let data = merge::merge_restore_backup(state, id).await?;
    Ok(Json(ApiResponse::success(data)))
}

fn reject_path_traversal(id: &str) -> Result<(), AppError> {
    if id.contains("..")
        || id.contains('/')
        || id.contains('\\')
        || id.contains('\0')
    {
        return Err(AppError::BadRequest(
            "invalid backup id".into(),
        ));
    }
    Ok(())
}
