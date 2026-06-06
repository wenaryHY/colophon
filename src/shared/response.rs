use serde::Serialize;

use crate::shared::{error::AppResult, request_id::current_or_generate_request_id};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
    pub request_id: String,
}

#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub pagination: PaginationMeta,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self::success_with_request_id(data, current_or_generate_request_id())
    }

    pub fn success_with_request_id(data: T, request_id: String) -> Self {
        Self {
            code: 0,
            message: "ok".into(),
            data: Some(data),
            request_id,
        }
    }

    pub fn error(code: i32, message: impl Into<String>) -> Self {
        Self::error_with_request_id(code, message, current_or_generate_request_id())
    }

    pub fn error_with_request_id(code: i32, message: impl Into<String>, request_id: String) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
            request_id,
        }
    }
}

impl<T: Serialize> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, page: i64, page_size: i64, total: i64) -> Self {
        Self {
            items,
            pagination: PaginationMeta {
                page,
                page_size,
                total,
            },
        }
    }
}

pub fn deleted_json() -> AppResult<serde_json::Value> {
    Ok(serde_json::json!({ "deleted": true }))
}

pub fn action_json(key: &str, value: bool) -> AppResult<serde_json::Value> {
    Ok(serde_json::json!({ key: value }))
}
