use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::header,
    Json,
};
use multer::Multipart;

use crate::{
    shared::{
        auth::AdminUser,
        error::{AppError, AppResult},
        response::{ApiResponse, PaginatedResponse},
    },
    state::AppState,
};

use super::{
    category,
    domain::MediaItem,
    dto::{
        CreateMediaCategoryRequest, MediaQuery, RenameMediaRequest, UpdateCategoryRequest,
        UpdateMediaCategoryCrudRequest,
    },
    service, MediaCategory,
};

type UploadResponse = ApiResponse<MediaItem>;

pub async fn list_media(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Query(query): Query<MediaQuery>,
) -> AppResult<Json<ApiResponse<PaginatedResponse<MediaItem>>>> {
    Ok(Json(ApiResponse::success(
        service::list_media(state, query).await?,
    )))
}

/// GET /api/v1/admin/media/{id} — 获取单个媒体项详情（含 conversion_status）
pub async fn get_media(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MediaItem>>> {
    let item = service::get_media_item(&state, &id).await?;
    Ok(Json(ApiResponse::success(item)))
}

/// 流式 multipart 上传：用 multer 边收边解析，不等整个 body 完成
pub async fn upload_media(
    State(state): State<Arc<AppState>>,
    admin: AdminUser,
    req: axum::http::Request<Body>,
) -> AppResult<Json<UploadResponse>> {
    let (parts, body) = req.into_parts();

    let content_type = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing Content-Type header".into()))?;

    let boundary = extract_multipart_boundary(content_type).ok_or_else(|| {
        AppError::BadRequest("Invalid multipart Content-Type, missing boundary".into())
    })?;

    // 流式 multipart 解析，边收边处理
    let stream = body.into_data_stream();
    let mut multipart = Multipart::new(stream, boundary);

    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut file_content_type: Option<String> = None;
    let mut category: Option<String> = None;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart 解析失败: {}", e)))?
    {
        match field.name() {
            Some("file") => {
                // 与旧行为一致：取第一个 file 字段，跳过后续的
                if file_data.is_some() {
                    continue;
                }
                filename = Some(field.file_name().unwrap_or("untitled").to_string());
                file_content_type = field.content_type().map(|ct| ct.to_string());

                // 流式读取文件内容 chunk，累加并检查大小上限
                let max_bytes = (state.config.storage.max_upload_size_mb * 1024 * 1024) as usize;
                let mut data = Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("文件读取失败: {}", e)))?
                {
                    data.extend_from_slice(&chunk);
                    if data.len() > max_bytes {
                        return Err(AppError::BadRequest(format!(
                            "file exceeds max size of {} MB",
                            state.config.storage.max_upload_size_mb
                        )));
                    }
                }
                file_data = Some(data);
            }
            Some("category") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("category 读取失败: {}", e)))?;
                category = String::from_utf8(bytes.to_vec()).ok();
            }
            _ => {
                // 忽略未知字段
            }
        }
    }

    let file_data =
        file_data.ok_or_else(|| AppError::BadRequest("file field is required".into()))?;
    let filename = filename.unwrap_or_else(|| "untitled".to_string());

    let result = service::upload_media_raw(
        state,
        &admin.0,
        filename,
        file_content_type,
        file_data,
        category,
    )
    .await?;

    Ok(Json(ApiResponse::success(result)))
}

fn extract_multipart_boundary(content_type: &str) -> Option<String> {
    for segment in content_type.split(';') {
        let segment = segment.trim();
        if segment.starts_with("boundary=") {
            return Some(segment[9..].trim_matches('"').to_string());
        }
    }
    None
}

pub async fn delete_media(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    Ok(Json(ApiResponse::success(
        service::delete_media(state, &id).await?,
    )))
}

pub async fn rename_media(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(body): Json<RenameMediaRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    Ok(Json(ApiResponse::success(
        service::rename_media(state, &id, &body.name).await?,
    )))
}

pub async fn update_media_category(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateCategoryRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    Ok(Json(ApiResponse::success(
        service::update_category(state, &id, body.category.as_deref()).await?,
    )))
}

pub async fn list_media_categories(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> AppResult<Json<ApiResponse<Vec<MediaCategory>>>> {
    Ok(Json(ApiResponse::success(
        category::list_categories(state).await?,
    )))
}

pub async fn create_media_category(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Json(body): Json<CreateMediaCategoryRequest>,
) -> AppResult<Json<ApiResponse<MediaCategory>>> {
    Ok(Json(ApiResponse::success(
        category::create_category(state, body).await?,
    )))
}

pub async fn update_media_category_crud(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateMediaCategoryCrudRequest>,
) -> AppResult<Json<ApiResponse<MediaCategory>>> {
    Ok(Json(ApiResponse::success(
        category::update_category(state, &id, body).await?,
    )))
}

pub async fn delete_media_category(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    Ok(Json(ApiResponse::success(
        category::delete_category(state, &id).await?,
    )))
}
