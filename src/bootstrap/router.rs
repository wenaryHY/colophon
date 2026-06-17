use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{header, request::Parts as RequestParts, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, patch, post},
    Router,
};
use axum_governor::{extractor::PeerIp, nz, GovernorConfigBuilder, GovernorLayer, Quota};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

use crate::{admin, modules, state::AppState, ws};

/// GET /api/v1/health — 健康检查端点
///
/// 用于监控系统检查服务存活状态。无需认证。
/// 始终返回 200，DB 故障通过 body 表达，避免触发 5xx 告警。
///
/// # Response
/// ```json
/// {
///   "status": "ok",      // "ok" | "degraded"
///   "db": "ok"           // "ok" | "error"
/// }
/// ```
///
/// # Example
/// ```bash
/// curl http://localhost:2000/api/v1/health
/// ```
async fn health_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db_ok = sqlx::query_scalar::<_, String>("SELECT 'ok'")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    let status = if db_ok { "ok" } else { "degraded" };
    axum::Json(serde_json::json!({
        "status": status,
        "db": if db_ok { "ok" } else { "error" }
    }))
}

/// GET /api/v1/version — 版本信息
///
/// 返回当前运行的 Colophon 版本号，用于客户端兼容性检查。无需认证。
///
/// # Response
/// ```json
/// {
///   "name": "colophon",
///   "version": "1.0.0"
/// }
/// ```
///
/// # Example
/// ```bash
/// curl http://localhost:2000/api/v1/version
/// ```
async fn version_info() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

pub async fn serve_admin_index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    admin::admin_static(Path("index.html".to_string()), State(state)).await
}

async fn render_home_entry(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
) -> crate::shared::error::AppResult<Response> {
    if !(*state.setup_stage.read().await).is_completed() {
        return Ok(Redirect::temporary("/admin").into_response());
    }
    modules::theme::handler::render_home(State(state), headers, auth).await
}

async fn serve_setup_entry() -> impl IntoResponse {
    Redirect::temporary("/admin").into_response()
}

async fn serve_admin_entry(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    serve_admin_index(State(state)).await.into_response()
}

async fn redirect_admin_with_trailing_slash() -> impl IntoResponse {
    Redirect::permanent("/admin")
}

async fn serve_admin_path(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if is_admin_asset_path(&path) {
        return admin::admin_static(Path(path), State(state))
            .await
            .into_response();
    }
    serve_admin_index(State(state)).await.into_response()
}

fn is_admin_asset_path(path: &str) -> bool {
    path.contains('.')
}

/// 后端路由层 auth guard — 检查 session cookie 中的 JWT，
/// 解码得到用户角色。Admin 放行，非 admin 返回 401 HTML 页面。
///
/// 注意：不是 `AdminUser` 提取器——`AdminUser` 返回 JSON 错误，对 SPA 入口 HTML
/// 请求不友好。这里返回一个内联 HTML 页面，包含中英文 i18n 提示 + 返回首页链接。
async fn admin_page_auth_guard(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Response {
    let auth_result = crate::shared::auth::session_token_from_headers(&headers)
        .and_then(|token| crate::infra::jwt::decode_token(&token, &state.config.auth.secret).ok())
        .map(|claims| claims.role);

    match auth_result {
        // 无 cookie 或无效 token → 放行。SPA 的 AdminGate 会显示 Login 组件
        None => next.run(req).await,
        // 已登录 + Admin → 放行
        Some(role) if role.can_access_admin() => next.run(req).await,
        // 已登录 + 非 Admin → 拒绝
        Some(_) => {
            let html = format!(
                r#"<!DOCTYPE html>
<html lang="zh-CN">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>权限不足 — Colophon</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'PingFang SC','Microsoft YaHei',sans-serif;
display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0;
background:#111318;color:#e2e3e8;text-align:center}}
h1{{font-size:24px;font-weight:700;margin-bottom:8px}}
p{{font-size:14px;color:#8b8d98;margin-bottom:24px}}
a{{color:#ff8c52;text-decoration:none;font-weight:500}}
a:hover{{text-decoration:underline}}
</style></head>
<body><div><h1>401 · 权限不足</h1>
<p>当前账号无权访问管理后台。<br>Permission denied. You do not have access to the admin panel.</p>
<a href="/">← 返回首页</a>&nbsp;&nbsp;<a href="/profile">个人中心</a></div>
</body></html>"#
            );
            axum::response::Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Body::from(html))
                .expect("response builder with valid status and headers cannot fail")
        }
    }
}

fn matches_cached_origin(cache: &Arc<tokio::sync::RwLock<String>>, origin: &HeaderValue) -> bool {
    if let Ok(cached) = cache.try_read() {
        if cached.is_empty() {
            return false;
        }
        if let Ok(parsed) = url::Url::parse(&cached) {
            let origin_str = parsed.origin().unicode_serialization();
            if let Ok(expected) = origin_str.parse::<HeaderValue>() {
                return &expected == origin;
            }
        }
    }
    false
}

pub async fn build_router(state: Arc<AppState>) -> Router {
    let port = state.config.server.port;

    let is_production = state.config.is_production();

    let base_origins: Vec<HeaderValue> = {
        let mut v = vec![
            format!("http://localhost:{port}")
                .parse::<HeaderValue>()
                .expect("hardcoded CORS origin must be valid HeaderValue"),
            format!("http://127.0.0.1:{port}")
                .parse::<HeaderValue>()
                .expect("hardcoded CORS origin must be valid HeaderValue"),
        ];
        if !is_production && port != 5173 {
            v.push(
                "http://localhost:5173"
                    .parse::<HeaderValue>()
                    .expect("hardcoded CORS origin must be valid HeaderValue"),
            );
            v.push(
                "http://127.0.0.1:5173"
                    .parse::<HeaderValue>()
                    .expect("hardcoded CORS origin must be valid HeaderValue"),
            );
        }
        v
    };

    let site_url_cache = state.site_url.clone();
    let admin_url_cache = state.admin_url.clone();
    let allow_origin =
        AllowOrigin::predicate(move |origin: &HeaderValue, _parts: &RequestParts| {
            if base_origins.iter().any(|item| item == origin) {
                return true;
            }
            if matches_cached_origin(&site_url_cache, origin) {
                return true;
            }
            if matches_cached_origin(&admin_url_cache, origin) {
                return true;
            }
            false
        });

    let cors_layer = CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::ORIGIN,
            "x-client-request-id"
                .parse()
                .expect("hardcoded header name must be valid"),
            "x-api-key"
                .parse()
                .expect("hardcoded header name must be valid"),
        ])
        .expose_headers([
            "x-client-request-id"
                .parse()
                .expect("hardcoded header name must be valid"),
            "x-request-id"
                .parse()
                .expect("hardcoded header name must be valid"),
        ]);

    let register_governor_config = GovernorConfigBuilder::default()
        .with_extractor(PeerIp::default())
        .expect_connect_info()
        .quota_default(Quota::requests_per_second(nz!(1u32)).burst(nz!(3u32)))
        .finish()
        .expect("governor config with valid quota must succeed");

    let auth_v1 = Router::new()
        .route(
            "/api/v1/auth/register",
            post(modules::auth::handler::register)
                .layer(GovernorLayer::new(register_governor_config)),
        )
        .route("/api/v1/auth/logout", post(modules::auth::handler::logout))
        .route(
            "/api/v1/auth/refresh",
            post(modules::auth::handler::refresh_token),
        )
        .merge(
            Router::new()
                .route("/api/v1/auth/login", post(modules::auth::handler::login))
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::shared::security::login_rate_limit,
                )),
        )
        .layer(cors_layer.clone());

    let v1 = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/version", get(version_info))
        .route(
            "/api/v1/turnstile-config",
            get(modules::setup::turnstile_config::get_turnstile_config),
        )
        .route("/api/v1/setup/status", get(modules::setup::handler::status))
        .route(
            "/api/v1/setup/initialize",
            post(modules::setup::handler::initialize),
        )
        .route("/api/v1/me", get(modules::user::handler::me))
        .route(
            "/api/v1/me/profile",
            patch(modules::user::handler::update_profile),
        )
        .route(
            "/api/v1/me/password",
            patch(modules::user::handler::update_password),
        )
        .route(
            "/api/v1/me/comments",
            get(modules::comment::handler::my_comments),
        )
        .route(
            "/api/v1/me/comments/{id}",
            delete(modules::comment::handler::delete_own_comment),
        )
        .route(
            "/api/v1/posts",
            get(modules::post::handler::list_public_posts),
        )
        .route("/api/v1/search", get(modules::post::handler::search_posts))
        .route(
            "/api/v1/posts/{slug}",
            get(modules::post::handler::get_public_post),
        )
        .route(
            "/api/v1/categories",
            get(modules::category::handler::list_categories),
        )
        .route("/api/v1/tags", get(modules::tag::handler::list_tags))
        .route(
            "/api/v1/admin/categories",
            post(modules::category::handler::create_category),
        )
        .route(
            "/api/v1/admin/categories/{id}",
            patch(modules::category::handler::update_category)
                .delete(modules::category::handler::delete_category),
        )
        .route(
            "/api/v1/admin/tags",
            post(modules::tag::handler::create_tag),
        )
        .route(
            "/api/v1/admin/tags/{id}",
            patch(modules::tag::handler::update_tag).delete(modules::tag::handler::delete_tag),
        )
        .route(
            "/api/v1/posts/{slug}/comments",
            get(modules::comment::handler::list_post_comments)
                .post(modules::comment::handler::create_comment),
        )
        .route(
            "/api/v1/themes/active",
            get(modules::theme::handler::active_theme),
        )
        .route(
            "/api/v1/admin/posts",
            get(modules::post::handler::list_admin_posts).post(modules::post::handler::create_post),
        )
        .route(
            "/api/v1/admin/posts/{id}",
            get(modules::post::handler::get_admin_post)
                .patch(modules::post::handler::update_post)
                .delete(modules::post::handler::delete_post),
        )
        .route(
            "/api/v1/admin/pages/upload",
            post(modules::post::handler::upload_custom_page),
        )
        .route(
            "/api/v1/admin/comments",
            get(modules::comment::handler::list_admin_comments),
        )
        .route(
            "/api/v1/admin/comments/{id}/approve",
            post(modules::comment::handler::approve_comment),
        )
        .route(
            "/api/v1/admin/comments/{id}/reject",
            post(modules::comment::handler::reject_comment),
        )
        .route(
            "/api/v1/admin/comments/{id}",
            delete(modules::comment::handler::delete_comment),
        )
        .route(
            "/api/v1/admin/comments/{id}/restore",
            post(modules::comment::handler::restore_comment),
        )
        .route(
            "/api/v1/admin/comments/{id}/purge",
            delete(modules::comment::handler::purge_comment),
        )
        .route(
            "/api/v1/admin/media",
            get(modules::media::handler::list_media).post(modules::media::handler::upload_media),
        )
        .route(
            "/api/v1/admin/media/{id}",
            delete(modules::media::handler::delete_media)
                .patch(modules::media::handler::rename_media),
        )
        .route(
            "/api/v1/admin/media/{id}/category",
            patch(modules::media::handler::update_media_category),
        )
        .route(
            "/api/v1/admin/media/categories",
            get(modules::media::handler::list_media_categories)
                .post(modules::media::handler::create_media_category),
        )
        .route(
            "/api/v1/admin/media/categories/{id}",
            patch(modules::media::handler::update_media_category_crud)
                .delete(modules::media::handler::delete_media_category),
        )
        .route(
            "/api/v1/admin/themes",
            get(modules::theme::handler::list_themes),
        )
        .route(
            "/api/v1/admin/themes/{slug}/detail",
            get(modules::theme::handler::get_theme_detail),
        )
        .route(
            "/api/v1/admin/themes/{slug}/config",
            patch(modules::theme::handler::save_theme_config),
        )
        .route(
            "/api/v1/admin/themes/upload",
            post(modules::theme::handler::upload_theme_archive),
        )
        .route(
            "/api/v1/admin/themes/{slug}/activate",
            post(modules::theme::handler::activate_theme),
        )
        .route(
            "/api/v1/admin/themes/{slug}",
            delete(modules::theme::handler::delete_theme),
        )
        .route(
            "/api/v1/preview/content",
            post(modules::theme::handler::preview_content),
        )
        .route(
            "/api/v1/preview/theme",
            post(modules::theme::handler::preview_theme),
        )
        .route(
            "/api/v1/admin/settings",
            get(modules::setting::handler::list_settings)
                .patch(modules::setting::handler::update_setting),
        )
        .route(
            "/api/v1/admin/settings/batch",
            patch(modules::setting::handler::update_settings_batch),
        )
        .route(
            "/api/v1/admin/backup",
            post(modules::backup::handler::create_backup),
        )
        .route(
            "/api/v1/admin/backup/list",
            get(modules::backup::handler::list_backups),
        )
        .route(
            "/api/v1/admin/backup/restore",
            post(modules::backup::handler::restore_backup),
        )
        .route(
            "/api/v1/admin/backup/schedule",
            get(modules::backup::handler::get_schedule)
                .patch(modules::backup::handler::update_schedule),
        )
        .route(
            "/api/v1/admin/backup/{id}",
            delete(modules::backup::handler::delete_backup)
                .get(modules::backup::handler::download_backup),
        )
        .route(
            "/api/v1/admin/backups/{id}/merge-restore",
            post(modules::backup::handler::merge_restore_backup),
        )
        .route(
            "/api/v1/admin/trash",
            get(modules::trash::handler::list_trash),
        )
        .route(
            "/api/v1/admin/trash/purge-expired",
            post(modules::trash::handler::purge_expired),
        )
        .route(
            "/api/v1/admin/trash/{item_type}/{id}/restore",
            post(modules::trash::handler::restore_item),
        )
        .route(
            "/api/v1/admin/trash/{item_type}/{id}",
            delete(modules::trash::handler::purge_item),
        )
        .route(
            "/api/v1/admin/plugins/{name}/settings",
            get(crate::modules::plugin::handler::get_settings)
                .put(crate::modules::plugin::handler::update_settings),
        )
        .route(
            "/api/v1/admin/plugins/slots",
            get(crate::modules::plugin::handler::list_slots),
        )
        .route(
            "/api/v1/admin/plugins",
            get(crate::modules::plugin::handler::list_plugins),
        )
        .route(
            "/api/v1/admin/plugins/{name}/toggle",
            post(crate::modules::plugin::handler::toggle_plugin),
        )
        .route(
            "/api/v1/admin/webhooks",
            get(crate::modules::webhook::handler::list_webhooks)
                .post(crate::modules::webhook::handler::create_webhook),
        )
        .route(
            "/api/v1/admin/webhooks/{id}",
            get(crate::modules::webhook::handler::get_webhook)
                .patch(crate::modules::webhook::handler::update_webhook)
                .delete(crate::modules::webhook::handler::delete_webhook),
        )
        .route(
            "/api/v1/admin/webhooks/{id}/deliveries",
            get(crate::modules::webhook::handler::list_deliveries),
        )
        .route(
            "/api/v1/admin/api-keys",
            get(crate::modules::api_key::handler::list_api_keys)
                .post(crate::modules::api_key::handler::create_api_key),
        )
        .route(
            "/api/v1/admin/api-keys/{id}",
            patch(crate::modules::api_key::handler::update_api_key)
                .delete(crate::modules::api_key::handler::revoke_api_key),
        )
        .merge(state.plugin_manager.read().await.collect_routes(&state))
        .layer(cors_layer.clone());

    Router::new()
        .route("/", get(render_home_entry))
        .route("/preview", get(modules::theme::handler::preview_page))
        .route("/posts/{slug}", get(modules::theme::handler::render_post))
        .route("/author/{username}", get(modules::theme::handler::render_author_archive))
        .route("/tags", get(modules::theme::handler::render_tags_list))
        .route("/categories", get(modules::theme::handler::render_categories_list))
        .route("/tags/{slug}", get(modules::theme::handler::render_tag_archive))
        .route("/search", get(modules::theme::handler::render_search))
        .route(
            "/categories/{slug}",
            get(modules::theme::handler::render_category_archive),
        )
        .route(
            "/pages/{slug}",
            get(modules::post::handler::render_custom_page),
        )
        .route(
            "/profile",
            get(modules::user::theme_handler::render_profile_page),
        )
        .route(
            "/login",
            get(|| async { axum::response::Redirect::permanent("/admin") }),
        )
        .route(
            "/register",
            get(modules::user::theme_handler::render_register_page),
        )
        .route(
            "/static/themes/{theme_slug}/{*file_path}",
            get(modules::theme::handler::serve_active_static),
        )
        .route(
            "/uploads/{*file_path}",
            get(modules::theme::handler::serve_upload_static),
        )
        .route(
            "/static/plugins/{plugin_slug}/{*file_path}",
            get(modules::theme::handler::serve_plugin_static),
        )
        .route(
            "/static/{*file_path}",
            get(modules::theme::handler::serve_global_static),
        )
        .route("/setup", get(serve_setup_entry))
        .nest("/admin", {
            Router::new()
                .route("/", get(serve_admin_entry))
                .route("/{*path}", get(serve_admin_path))
                .route_layer(middleware::from_fn_with_state(
                    state.clone(),
                    admin_page_auth_guard,
                ))
        })
        .route("/admin/", get(redirect_admin_with_trailing_slash))
        .route("/sitemap.xml", get(modules::seo::sitemap::serve_sitemap))
        .route("/rss.xml", get(modules::post::feed::render_atom_feed))
        .route("/feed", get(modules::post::feed::redirect_feed_to_rss))
        .route("/robots.txt", get(modules::seo::robots::serve_robots))
        .route(
            "/favicon.ico",
            get(|| async { axum::http::StatusCode::NO_CONTENT }),
        )
        .route("/ws/admin", get(ws::ws_admin_handler))
        .route("/ws/public", get(ws::ws_public_handler))
        .merge(auth_v1)
        .merge(v1)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new()),
        )
        .layer(axum::middleware::from_fn(
            crate::shared::security::security_headers,
        ))
        .layer(axum::middleware::from_fn(
            crate::shared::request_id::request_id_context,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::infra::i18n_middleware::inject_language,
        ))
        .fallback(get(modules::theme::handler::fallback_404))
        .with_state(state)
}
