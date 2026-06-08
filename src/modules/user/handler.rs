use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};

use crate::{
    shared::{auth::AuthUser, error::AppResult, json::AppJson, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{UpdatePasswordRequest, UpdateProfileRequest},
    service,
};

pub async fn me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: AuthUser,
) -> AppResult<Json<ApiResponse<super::domain::CurrentUser>>> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "user",
        event = "me_request",
        client_request_id = %client_request_id,
        user_id = %auth.id,
        "loading current user profile"
    );
    Ok(Json(ApiResponse::success(service::me(state, &auth).await?)))
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: AuthUser,
    AppJson(body): AppJson<UpdateProfileRequest>,
) -> AppResult<impl IntoResponse> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "user",
        event = "update_profile_request",
        client_request_id = %client_request_id,
        user_id = %auth.id,
        "updating current user profile"
    );
    
    let updated_lang = body.language.clone();
    let updated_user = service::update_profile(state, &auth, body).await?;
    
    // 如果更新了语言偏好，同步设置 cookie（与前端、admin 互通）
    let mut response_headers = axum::http::HeaderMap::new();
    if let Some(new_lang) = updated_lang {
        let cookie_value = crate::infra::i18n_middleware::build_lang_cookie(&new_lang);
        response_headers.insert(
            axum::http::header::SET_COOKIE,
            cookie_value.parse().expect("cookie value must be valid HeaderValue"),
        );
    }
    
    Ok((response_headers, Json(ApiResponse::success(updated_user))))
}

pub async fn update_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: AuthUser,
    AppJson(body): AppJson<UpdatePasswordRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "user",
        event = "update_password_request",
        client_request_id = %client_request_id,
        user_id = %auth.id,
        "updating current user password"
    );
    Ok(Json(ApiResponse::success(
        service::update_password(state, &auth, body).await?,
    )))
}
