use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::error::Error as StdError;
use thiserror::Error;

use super::response::ApiResponse;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("资源未找到: {0}")]
    NotFound(String),
    #[error("未授权访问")]
    Unauthorized,
    #[error("禁止访问")]
    Forbidden,
    #[error("请求参数错误: {0}")]
    BadRequest(String),
    #[error("资源已存在: {0}")]
    Conflict(String),
    #[error("请求过于频繁: {0}")]
    TooManyRequests(String),
    #[error("文件上传错误: {0}")]
    Multipart(String),
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                crate::shared::error_codes::NOT_FOUND,
                msg,
            ),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                crate::shared::error_codes::UNAUTHORIZED,
                "未授权访问".to_string(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                crate::shared::error_codes::FORBIDDEN,
                "禁止访问".to_string(),
            ),
            Self::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                crate::shared::error_codes::BAD_REQUEST,
                msg,
            ),
            Self::Conflict(msg) => (
                StatusCode::CONFLICT,
                crate::shared::error_codes::CONFLICT,
                msg,
            ),
            Self::TooManyRequests(msg) => (
                StatusCode::TOO_MANY_REQUESTS,
                crate::shared::error_codes::TOO_MANY_REQUESTS,
                msg,
            ),
            Self::Multipart(msg) => (
                StatusCode::BAD_REQUEST,
                crate::shared::error_codes::BAD_REQUEST,
                msg,
            ),
            Self::Sqlx(err) => {
                tracing::error!(
                    module = "shared_error",
                    event = "sqlx_error",
                    error = ?err,
                    error_source = ?err.source(),
                    "数据库错误"
                );

                // 细分数据库错误类型
                if let Some(db_err) = err.as_database_error() {
                    if db_err.is_unique_violation() {
                        return (
                            StatusCode::CONFLICT,
                            Json(ApiResponse::<()>::error(
                                crate::shared::error_codes::RESOURCE_ALREADY_EXISTS,
                                "资源已存在".to_string(),
                            )),
                        )
                        .into_response();
                    }
                    if db_err.is_foreign_key_violation() {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ApiResponse::<()>::error(
                                crate::shared::error_codes::BAD_REQUEST,
                                "关联资源不存在".to_string(),
                            )),
                        )
                        .into_response();
                    }
                    if db_err.is_check_violation() {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ApiResponse::<()>::error(
                                crate::shared::error_codes::BAD_REQUEST,
                                "数据校验失败".to_string(),
                            )),
                        )
                        .into_response();
                    }
                }

                // RowNotFound 特殊处理
                if matches!(err, sqlx::Error::RowNotFound) {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(ApiResponse::<()>::error(
                            crate::shared::error_codes::NOT_FOUND,
                            "资源未找到".to_string(),
                        )),
                    )
                    .into_response();
                }

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    crate::shared::error_codes::DATABASE_ERROR,
                    "数据库错误".to_string(),
                )
            }
            Self::Config(err) => {
                tracing::error!(
                    module = "shared_error",
                    event = "config_error",
                    error = ?err,
                    "配置错误"
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    crate::shared::error_codes::CONFIG_ERROR,
                    "配置错误".to_string(),
                )
            }
            Self::Io(err) => {
                tracing::error!(
                    module = "shared_error",
                    event = "io_error",
                    error = ?err,
                    "IO错误"
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    crate::shared::error_codes::INTERNAL_ERROR,
                    "内部服务器错误".to_string(),
                )
            }
            Self::Anyhow(err) => {
                tracing::error!(
                    module = "shared_error",
                    event = "anyhow_error",
                    error = ?err,
                    "应用程序错误"
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    crate::shared::error_codes::INTERNAL_ERROR,
                    "内部服务器错误".to_string(),
                )
            }
            Self::SerdeJson(err) => {
                tracing::error!(
                    module = "shared_error",
                    event = "serde_json_error",
                    error = ?err,
                    "JSON序列化错误"
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    crate::shared::error_codes::INTERNAL_ERROR,
                    "内部服务器错误".to_string(),
                )
            }
        };

        (status, Json(ApiResponse::<()>::error(code, message))).into_response()
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::Multipart(err.to_string())
    }
}
