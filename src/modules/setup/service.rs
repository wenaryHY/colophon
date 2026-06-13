use std::sync::Arc;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    infra::jwt,
    modules::{
        setting::{
            repository as setting_repository,
            validator::{
                canonical_admin_url_from_site_url, normalize_admin_url, normalize_site_url,
            },
        },
        setup::{
            domain::SetupStage,
            dto::{SetupInitializeRequest, SetupInitializeResponse, SetupStatusResponse},
            repository::{self, SetupSnapshot, SetupWriteModel},
        },
    },
    shared::{
        auth::{hash_password, issue_token},
        error::{AppError, AppResult},
        role::Role,
    },
    state::AppState,
};

pub struct SetupRuntimeBootstrap {
    pub site_url: String,
    pub admin_url: String,
    pub stage: SetupStage,
}

pub async fn bootstrap_runtime(pool: &SqlitePool) -> AppResult<SetupRuntimeBootstrap> {
    let snapshot = load_and_reconcile_snapshot(pool).await?;
    Ok(SetupRuntimeBootstrap {
        site_url: snapshot.site_url,
        admin_url: snapshot.admin_url,
        stage: snapshot.stage,
    })
}

pub async fn get_status(state: Arc<AppState>) -> AppResult<SetupStatusResponse> {
    let snapshot = load_and_reconcile_snapshot(&state.pool).await?;
    refresh_runtime_from_snapshot(&state, &snapshot).await;
    Ok(SetupStatusResponse {
        installed: snapshot.stage.is_completed(),
        stage: snapshot.stage,
        site_title: snapshot.site_title,
        site_description: snapshot.site_description,
        site_url: snapshot.site_url,
        admin_url: snapshot.admin_url,
        allow_register: snapshot.allow_register,
    })
}

pub async fn initialize(
    state: Arc<AppState>,
    body: SetupInitializeRequest,
) -> AppResult<(SetupInitializeResponse, String)> {
    let snapshot = load_and_reconcile_snapshot(&state.pool).await?;
    ensure_not_installed(snapshot.stage)?;
    ensure_setup_can_run(&snapshot)?;

    let model = build_write_model(body).await?;
    let user_id = repository::create_installation(&state.pool, &model).await?;
    refresh_runtime_cache(&state, &model).await;

    // 签发 refresh token，确保初始化时管理员自动获得刷新能力
    let refresh_token = jwt::generate_refresh_token();
    let refresh_token_hash = jwt::hash_token(&refresh_token);
    let refresh_token_id = Uuid::new_v4().to_string();
    let refresh_expires_at = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    crate::modules::auth::repository::save_refresh_token(
        &state.pool,
        &refresh_token_id,
        &user_id,
        &refresh_token_hash,
        &refresh_expires_at,
        "setup",
    )
    .await?;

    let token = issue_token(
        &state.config.auth.secret,
        state.config.auth.expires_in_seconds,
        user_id,
        model.username.clone(),
        Role::Admin,
    )?;

    Ok((
        SetupInitializeResponse {
            token,
            redirect_to: model.admin_url,
        },
        refresh_token,
    ))
}

async fn build_write_model(body: SetupInitializeRequest) -> AppResult<SetupWriteModel> {
    let site_title = require_text(&body.site_title, "site_title")?;
    let username = require_text(&body.username, "username")?;
    let email = require_text(&body.email, "email")?;
    let display_name = body
        .display_name
        .as_deref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(&username)
        .to_string();

    ensure_username(&username)?;
    ensure_password(&body.password)?;

    let site_url = normalize_site_url(&body.site_url)?;
    let _ = normalize_admin_url(&body.admin_url)?;
    let admin_url = canonical_admin_url_from_site_url(&site_url)?;

    Ok(SetupWriteModel {
        site_title,
        site_description: body.site_description.trim().to_string(),
        site_url,
        admin_url,
        allow_register: body.allow_register,
        username,
        email,
        display_name,
        password_hash: hash_password(&body.password).await?,
    })
}

fn require_text(value: &str, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(format!("{field} is required")));
    }
    Ok(trimmed.to_string())
}

fn ensure_username(value: &str) -> AppResult<()> {
    if value.len() >= 3 {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "username must be at least 3 characters".into(),
    ))
}

fn ensure_password(value: &str) -> AppResult<()> {
    if value.len() >= 8 {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "password must be at least 8 characters".into(),
    ))
}

fn ensure_setup_can_run(snapshot: &SetupSnapshot) -> AppResult<()> {
    if snapshot.user_count == 0 {
        return Ok(());
    }
    Err(AppError::Conflict(
        "setup cannot continue because users already exist".into(),
    ))
}

pub fn ensure_not_installed(stage: SetupStage) -> AppResult<()> {
    if !stage.is_completed() {
        return Ok(());
    }
    Err(AppError::Conflict("setup already completed".into()))
}

async fn load_and_reconcile_snapshot(pool: &SqlitePool) -> AppResult<SetupSnapshot> {
    let mut snapshot = repository::load_snapshot(pool).await?;

    let mut changed = false;
    if !snapshot.site_url.trim().is_empty() {
        let normalized_site = normalize_site_url(&snapshot.site_url)?;
        if normalized_site != snapshot.site_url {
            snapshot.site_url = normalized_site;
            changed = true;
        }

        let canonical_admin = canonical_admin_url_from_site_url(&snapshot.site_url)?;
        if canonical_admin != snapshot.admin_url {
            snapshot.admin_url = canonical_admin;
            changed = true;
        }
    }

    if snapshot.needs_state_backfill() {
        repository::persist_stage(pool, snapshot.stage).await?;
    }

    if changed {
        setting_repository::upsert(pool, "site_url", &snapshot.site_url).await?;
        setting_repository::upsert(pool, "admin_url", &snapshot.admin_url).await?;
    }

    Ok(snapshot)
}

async fn refresh_runtime_from_snapshot(state: &Arc<AppState>, snapshot: &SetupSnapshot) {
    *state.site_url.write().await = snapshot.site_url.clone();
    *state.admin_url.write().await = snapshot.admin_url.clone();
    *state.setup_stage.write().await = snapshot.stage;
}

async fn refresh_runtime_cache(state: &Arc<AppState>, model: &SetupWriteModel) {
    *state.site_url.write().await = model.site_url.clone();
    *state.admin_url.write().await = model.admin_url.clone();
    *state.setup_stage.write().await = SetupStage::Completed;
}
