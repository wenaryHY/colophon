use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, State},
    Json,
};

use crate::{
    shared::{
        auth::AdminUser,
        error::{AppError, AppResult},
        response::ApiResponse,
    },
    state::AppState,
};

use crate::modules::theme::{
    domain::ThemeSummary,
    dto::{SaveThemeConfigRequest, ThemeDetailResponse, ThemeUploadResponse},
    service::ThemeService,
    ThemeManifest,
};

pub async fn active_theme(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    let slug = service.list_themes(&state.pool).await?.1;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "slug": slug }),
    )))
}

pub async fn list_themes(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> AppResult<Json<ApiResponse<Vec<ThemeSummary>>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    let (manifests, active_slug) = service.list_themes(&state.pool).await?;
    let summaries = manifests
        .into_iter()
        .map(|manifest| ThemeSummary {
            active: manifest.slug == active_slug,
            manifest,
        })
        .collect();
    Ok(Json(ApiResponse::success(summaries)))
}

pub async fn activate_theme(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(slug): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    service.activate_theme(&state.pool, &slug).await?;
    state.invalidate_all_caches().await;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "activated": slug }),
    )))
}

pub async fn get_theme_detail(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(slug): Path<String>,
) -> AppResult<Json<ApiResponse<ThemeDetailResponse>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    let (manifest, config) = service.get_theme_detail(&state.pool, &slug).await?;
    let schema = manifest.config.clone();
    Ok(Json(ApiResponse::success(ThemeDetailResponse {
        manifest,
        config,
        schema,
    })))
}

pub async fn save_theme_config(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(slug): Path<String>,
    Json(req): Json<SaveThemeConfigRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    service
        .save_theme_config(&state.pool, &slug, &req.config)
        .await?;
    state.invalidate_all_caches().await;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "saved": slug }),
    )))
}

pub async fn upload_theme_archive(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    mut multipart: Multipart,
) -> AppResult<Json<ApiResponse<ThemeUploadResponse>>> {
    let mut theme_data: Option<Vec<u8>> = None;

    // 提取上传的文件
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            theme_data = Some(field.bytes().await?.to_vec());
            break;
        }
    }

    let theme_data = theme_data.ok_or(AppError::BadRequest("No file uploaded".to_string()))?;

    // 解析 zip 包
    let cursor = std::io::Cursor::new(theme_data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|_| AppError::BadRequest("Invalid zip file".to_string()))?;

    // 查找 theme.toml
    let mut manifest_content = String::new();
    {
        let mut theme_toml = archive
            .by_name("theme.toml")
            .map_err(|_| AppError::BadRequest("theme.toml not found in archive".to_string()))?;
        std::io::Read::read_to_string(&mut theme_toml, &mut manifest_content)
            .map_err(|e| AppError::Io(e))?;
    }

    // 解析 manifest
    let manifest: ThemeManifest = toml::from_str(&manifest_content)
        .map_err(|e| AppError::BadRequest(format!("Failed to parse theme.toml: {}", e)))?;

    // 提取主题到 themes 目录
    let theme_dir = state.theme_dir.join(&manifest.slug);
    if theme_dir.exists() {
        std::fs::remove_dir_all(&theme_dir).map_err(|e| AppError::Io(e))?;
    }
    std::fs::create_dir_all(&theme_dir).map_err(|e| AppError::Io(e))?;

    let extract_result = (|| -> AppResult<()> {
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Failed to read archive: {}", e)))?;
            let entry_path = file
                .enclosed_name()
                .ok_or_else(|| AppError::BadRequest("ZIP contains invalid path entry".to_string()))?
                .to_path_buf();
            let outpath = theme_dir.join(entry_path);

            if file.is_dir() {
                std::fs::create_dir_all(&outpath).map_err(AppError::Io)?;
                continue;
            }
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(AppError::Io)?;
            }
            let mut outfile = std::fs::File::create(&outpath).map_err(AppError::Io)?;
            std::io::copy(&mut file, &mut outfile).map_err(AppError::Io)?;
        }
        Ok(())
    })();
    if let Err(err) = extract_result {
        if let Err(e) = std::fs::remove_dir_all(&theme_dir) {
            tracing::warn!(
                module = "theme",
                path = %theme_dir.display(),
                error = %e,
                "failed to clean up theme directory after extraction error"
            );
        }
        return Err(err);
    }

    // 校验必要模板文件
    let templates_dir = theme_dir.join("templates");
    if !templates_dir.join("index.html").exists() {
        let _ = std::fs::remove_dir_all(&theme_dir);
        return Err(AppError::BadRequest(
            "主题缺少必要文件: templates/index.html 不存在".into(),
        ));
    }
    if !templates_dir.join("post.html").exists() {
        let _ = std::fs::remove_dir_all(&theme_dir);
        return Err(AppError::BadRequest(
            "主题缺少必要文件: templates/post.html 不存在".into(),
        ));
    }

    Ok(Json(ApiResponse::success(ThemeUploadResponse {
        slug: manifest.slug.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        message: "主题已上传".to_string(),
    })))
}

/// 验证主题 slug 是否安全（不含路径穿越字符）
fn validate_theme_slug_is_safe(slug: &str) -> AppResult<()> {
    if slug.contains("..") || slug.contains('/') || slug.contains('\\') {
        return Err(AppError::BadRequest("非法的主题标识".into()));
    }
    Ok(())
}

/// DELETE /api/v1/admin/themes/{slug}
pub async fn delete_theme(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(slug): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    validate_theme_slug_is_safe(&slug)?;

    if slug == "default" {
        return Err(AppError::BadRequest("不能删除 default 主题".into()));
    }

    let active_slug = crate::modules::theme::repository::get_active_theme(&state.pool).await?;
    if slug == active_slug {
        return Err(AppError::BadRequest(
            "不能删除当前激活的主题，请先切换到其他主题".into(),
        ));
    }

    let theme_path = state.theme_dir.join(&slug);
    if !theme_path.exists() {
        return Err(AppError::NotFound(format!("主题 '{}' 不存在", slug)));
    }

    std::fs::remove_dir_all(&theme_path).map_err(|e| {
        tracing::error!(
            module = "theme",
            event = "delete_theme_io_error",
            slug = %slug,
            error = %e,
            "failed to remove theme directory"
        );
        AppError::Io(e)
    })?;

    crate::modules::theme::repository::delete_config(&state.pool, &slug).await?;
    state.invalidate_all_caches().await;

    tracing::info!(
        module = "theme",
        event = "theme_deleted",
        slug = %slug,
        "theme and config deleted successfully"
    );

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "deleted": slug }),
    )))
}

#[cfg(test)]
mod slug_tests {
    use super::validate_theme_slug_is_safe;

    #[test]
    fn valid_slug_passes() {
        assert!(validate_theme_slug_is_safe("my-theme").is_ok());
    }

    #[test]
    fn dot_dot_rejected() {
        assert!(validate_theme_slug_is_safe("../escape").is_err());
    }

    #[test]
    fn slash_rejected() {
        assert!(validate_theme_slug_is_safe("bad/slug").is_err());
    }

    #[test]
    fn backslash_rejected() {
        assert!(validate_theme_slug_is_safe("bad\\slug").is_err());
    }
}
