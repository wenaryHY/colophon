use std::sync::Arc;

use crate::{
    modules::{auth::dto::RegisterRequest, setting::repository as setting_repository},
    shared::{
        auth::{hash_password, issue_token, verify_password, generate_refresh_token, hash_token},
        error::{AppError, AppResult},
    },
    state::AppState,
};

use super::{
    dto::{AuthUserInfo, LoginRequest, LoginResponseData},
    repository,
};

pub async fn register(
    state: Arc<AppState>,
    body: RegisterRequest,
    token_lifetime_seconds: u64,
    refresh_expires_in_seconds: u64,
) -> AppResult<(LoginResponseData, String)> {
    ensure_public_registration_available(&state, &body).await?;
    validate_register_request(&body)?;
    ensure_identity_available(&state, &body).await?;

    let username = body.username.trim().to_string();
    let email = body.email.trim().to_string();
    let display_name = body
        .display_name
        .unwrap_or_else(|| username.clone())
        .trim()
        .to_string();
    let password_hash = hash_password(&body.password).await?;
    let role = "member";
    let user_id = repository::insert_user(
        &state.pool,
        &username,
        &email,
        &password_hash,
        &display_name,
        role,
    )
    .await?;

    let access_token = issue_token(
        &state.config.auth.secret,
        token_lifetime_seconds,
        user_id.clone(),
        username.clone(),
        role.to_string(),
    )?;

    let refresh_token = generate_refresh_token();
    let token_hash = hash_token(&refresh_token);
    let refresh_id = uuid::Uuid::new_v4().to_string();
    let family_id = uuid::Uuid::new_v4().to_string();
    // DB expires_at 对齐 cookie Max-Age
    let expires_at = (chrono::Utc::now() + chrono::Duration::seconds(refresh_expires_in_seconds as i64)).to_rfc3339();
    repository::save_refresh_token(&state.pool, &refresh_id, &user_id, &token_hash, &expires_at, &family_id)
        .await?;

    tracing::info!(
        module = "auth",
        event = "register_success",
        username = %username,
        role = %role,
        "registration succeeded"
    );
    Ok((
        LoginResponseData {
            user: AuthUserInfo {
                id: user_id,
                username,
                role: role.to_string(),
            },
            access_token,
        },
        refresh_token,
    ))
}

pub async fn login(
    state: Arc<AppState>,
    body: LoginRequest,
    token_lifetime_seconds: u64,
    refresh_expires_in_seconds: u64,
) -> AppResult<(LoginResponseData, String)> {
    ensure_setup_completed(&state).await?;
    tracing::debug!(
        module = "auth",
        event = "login_lookup",
        login = %body.login,
        "looking up login account"
    );
    let user = repository::find_by_login(&state.pool, body.login.trim())
        .await?
        .ok_or_else(|| {
            tracing::warn!(
                module = "auth",
                event = "login_user_not_found",
                login = %body.login,
                "login rejected"
            );
            AppError::Unauthorized
        })?;

    if user.status != "active" {
        tracing::warn!(
            module = "auth",
            event = "login_inactive_user",
            user_id = %user.id,
            username = %user.username,
            status = %user.status,
            "login rejected"
        );
        return Err(AppError::Forbidden);
    }

    if !verify_password(&body.password, &user.password_hash).await? {
        tracing::warn!(
            module = "auth",
            event = "login_bad_password",
            user_id = %user.id,
            username = %user.username,
            "login rejected"
        );
        return Err(AppError::Unauthorized);
    }

    repository::touch_last_login(&state.pool, &user.id).await?;
    tracing::info!(
        module = "auth",
        event = "login_success",
        user_id = %user.id,
        username = %user.username,
        role = %user.role,
        "login succeeded"
    );
    let access_token = issue_token(
        &state.config.auth.secret,
        token_lifetime_seconds,
        user.id.clone(),
        user.username.clone(),
        user.role.clone(),
    )?;

    let refresh_token = generate_refresh_token();
    let token_hash = hash_token(&refresh_token);
    let refresh_id = uuid::Uuid::new_v4().to_string();
    let family_id = uuid::Uuid::new_v4().to_string();
    // DB expires_at 对齐 cookie Max-Age
    let expires_at = (chrono::Utc::now() + chrono::Duration::seconds(refresh_expires_in_seconds as i64)).to_rfc3339();
    repository::save_refresh_token(&state.pool, &refresh_id, &user.id, &token_hash, &expires_at, &family_id)
        .await?;

    Ok((
        LoginResponseData {
            user: AuthUserInfo {
                id: user.id,
                username: user.username,
                role: user.role,
            },
            access_token,
        },
        refresh_token,
    ))
}

async fn ensure_public_registration_available(
    state: &Arc<AppState>,
    body: &RegisterRequest,
) -> AppResult<()> {
    ensure_setup_completed(state).await?;

    let allow_register = setting_repository::get_bool(&state.pool, "allow_register", true).await?;
    if !allow_register {
        tracing::warn!(
            module = "auth",
            event = "register_disabled",
            username = %body.username,
            email = %body.email,
            "registration rejected"
        );
        return Err(AppError::Conflict("public registration is disabled".into()));
    }

    if repository::user_count(&state.pool).await? > 0 {
        return Ok(());
    }

    tracing::warn!(
        module = "auth",
        event = "register_without_initialized_admin",
        username = %body.username,
        email = %body.email,
        "registration rejected"
    );
    Err(AppError::Conflict(
        "public registration is unavailable before administrator initialization".into(),
    ))
}

fn validate_register_request(body: &RegisterRequest) -> AppResult<()> {
    if body.username.trim().len() < 3 {
        tracing::warn!(
            module = "auth",
            event = "register_invalid_username",
            username = %body.username,
            "registration rejected"
        );
        return Err(AppError::BadRequest(
            "username must be at least 3 characters".into(),
        ));
    }

    if body.password.len() < 6 {
        tracing::warn!(
            module = "auth",
            event = "register_invalid_password",
            username = %body.username,
            "registration rejected"
        );
        return Err(AppError::BadRequest(
            "password must be at least 6 characters".into(),
        ));
    }

    Ok(())
}

async fn ensure_identity_available(state: &Arc<AppState>, body: &RegisterRequest) -> AppResult<()> {
    let exists =
        repository::exists_by_username_or_email(&state.pool, &body.username, &body.email).await?;
    if !exists {
        return Ok(());
    }

    tracing::warn!(
        module = "auth",
        event = "register_conflict",
        username = %body.username,
        email = %body.email,
        "registration rejected"
    );
    Err(AppError::Conflict(
        "username or email already exists".into(),
    ))
}

async fn ensure_setup_completed(state: &Arc<AppState>) -> AppResult<()> {
    if (*state.setup_stage.read().await).is_completed() {
        return Ok(());
    }
    Err(AppError::Conflict("setup not completed".into()))
}
