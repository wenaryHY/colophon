use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::{
    modules::theme::context::TemplateContext,
    shared::error::{AppError, AppResult},
    state::AppState,
};

#[derive(Debug, Default, Deserialize)]
pub struct AuthPageQuery {
    pub redirect: Option<String>,
}

fn sanitize_redirect(target: Option<&str>, fallback: &str) -> String {
    match target {
        Some(path) if path.starts_with('/') && !path.starts_with("//") => path.to_string(),
        _ => fallback.to_string(),
    }
}

pub async fn render_profile_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    let auth_user = match auth {
        Some(user) => user,
        None => {
            tracing::info!(
                module = "user_theme",
                event = "profile_redirect_to_login",
                client_request_id = %client_request_id,
                path = "/profile",
                redirect_to = "/login?redirect=/profile",
                "redirecting unauthenticated profile request"
            );
            return Ok(Redirect::to("/login?redirect=/profile").into_response());
        }
    };

    tracing::info!(
        module = "user_theme",
        event = "profile_render",
        client_request_id = %client_request_id,
        user_id = %auth_user.id,
        username = %auth_user.username,
        "rendering profile page"
    );

    let current_user = crate::modules::user::repository::find_current(&state.pool, &auth_user.id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let ctx = TemplateContext::load(&state).await?;
    let plugin_guard = state.plugin_manager.read().await;
    let env = crate::modules::theme::engine::build_template_engine(&ctx, &state.theme_dir, &*plugin_guard, &state.template_env_cache)?;
    let tmpl = env
        .get_template("profile.html")
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {
            current_user => current_user
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

pub async fn render_login_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
    Query(query): Query<AuthPageQuery>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    let redirect_to = sanitize_redirect(query.redirect.as_deref(), "/profile");

    if let Some(user) = auth {
        tracing::info!(
            module = "user_theme",
            event = "login_page_redirect",
            client_request_id = %client_request_id,
            user_id = %user.id,
            username = %user.username,
            redirect_to = %redirect_to,
            "redirecting authenticated login-page request"
        );
        return Ok(Redirect::to(&redirect_to).into_response());
    }

    tracing::info!(
        module = "user_theme",
        event = "login_page_render",
        client_request_id = %client_request_id,
        redirect_to = %redirect_to,
        "rendering login page"
    );

    let ctx = TemplateContext::load(&state).await?;
    let plugin_guard = state.plugin_manager.read().await;
    let env = crate::modules::theme::engine::build_template_engine(&ctx, &state.theme_dir, &*plugin_guard, &state.template_env_cache)?;
    let tmpl = env
        .get_template("login.html")
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {
            redirect_to => redirect_to
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

pub async fn render_register_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
    Query(query): Query<AuthPageQuery>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    let redirect_to = sanitize_redirect(query.redirect.as_deref(), "/profile");

    if let Some(user) = auth {
        tracing::info!(
            module = "user_theme",
            event = "register_page_redirect",
            client_request_id = %client_request_id,
            user_id = %user.id,
            username = %user.username,
            redirect_to = %redirect_to,
            "redirecting authenticated register-page request"
        );
        return Ok(Redirect::to(&redirect_to).into_response());
    }

    tracing::info!(
        module = "user_theme",
        event = "register_page_render",
        client_request_id = %client_request_id,
        redirect_to = %redirect_to,
        "rendering registration page"
    );

    let ctx = TemplateContext::load(&state).await?;
    let plugin_guard = state.plugin_manager.read().await;
    let env = crate::modules::theme::engine::build_template_engine(&ctx, &state.theme_dir, &*plugin_guard, &state.template_env_cache)?;
    let tmpl = env
        .get_template("register.html")
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {
            redirect_to => redirect_to
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}
