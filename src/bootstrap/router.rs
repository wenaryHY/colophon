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
use axum_governor::{nz, Quota};
use axum_governor::GovernorConfigBuilder as AxumGovernorConfigBuilder;
use axum_governor::GovernorLayer as AxumGovernorLayer;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use utoipa::OpenApi;

use crate::{admin, modules, shared::security::ForwardedIpExtractor, state::AppState, ws};

/// 健康检查响应体。
///
/// 始终返回 200，DB 故障通过 body 中的 `status`/`db` 字段表达，
/// 避免触发监控系统的 5xx 告警。
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// 服务整体状态：`"ok"` | `"degraded"`
    pub status: String,
    /// 数据库连通性：`"ok"` | `"error"`
    pub db: String,
}

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
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "system",
    responses(
        (status = 200, description = "服务健康状态", body = crate::shared::response::ApiResponse<HealthResponse>),
    )
)]
async fn health_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db_ok = sqlx::query_scalar::<_, String>("SELECT 'ok'")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    let status = if db_ok { "ok" } else { "degraded" };
    axum::Json(HealthResponse {
        status: status.to_string(),
        db: if db_ok { "ok" } else { "error" }.to_string(),
    })
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

// ---------------------------------------------------------------------------
// 子函数：CORS 配置
// ---------------------------------------------------------------------------

/// 构建 CORS 层 — 允许 localhost / 127.0.0.1 / 配置中的 site_url / admin_url
fn configure_cors(state: &AppState) -> CorsLayer {
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

    CorsLayer::new()
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
        ])
}

// ---------------------------------------------------------------------------
// 子函数：限流配置
// ---------------------------------------------------------------------------

/// 三种限流级别的 Governor layer。
struct RateLimitLayers {
    /// 注册接口：每秒 1 次，突发 3 次
    register: AxumGovernorLayer<std::net::IpAddr>,
    /// 登录接口：极严苛（Argon2 极度耗费 CPU）— 每 10 秒 1 次，突发 3 次
    login: AxumGovernorLayer<std::net::IpAddr>,
    /// 普通 API 接口：防爬虫 — 每秒 10 次，突发 50 次
    api: AxumGovernorLayer<std::net::IpAddr>,
}

fn configure_rate_limits(trusted_proxies: Vec<std::net::IpAddr>) -> RateLimitLayers {
    let register = AxumGovernorLayer::new(
        AxumGovernorConfigBuilder::default()
            .with_extractor(ForwardedIpExtractor { trusted_proxies: trusted_proxies.clone() })
            .quota_default(Quota::requests_per_second(nz!(1u32)).burst(nz!(3u32)))
            .finish()
            .expect("governor config with valid quota must succeed"),
    );

    let login = AxumGovernorLayer::new(
        AxumGovernorConfigBuilder::default()
            .with_extractor(ForwardedIpExtractor { trusted_proxies: trusted_proxies.clone() })
            .quota_default(Quota::seconds_per_request(nz!(10u32)).burst(nz!(3u32)))
            .finish()
            .unwrap(),
    );

    let api = AxumGovernorLayer::new(
        AxumGovernorConfigBuilder::default()
            .with_extractor(ForwardedIpExtractor { trusted_proxies })
            .quota_default(Quota::requests_per_second(nz!(10u32)).burst(nz!(50u32)))
            .finish()
            .unwrap(),
    );

    RateLimitLayers { register, login, api }
}

// ---------------------------------------------------------------------------
// 子函数：认证路由
// ---------------------------------------------------------------------------

/// 认证相关路由 — register / login / logout / refresh
fn auth_routes(
    cors: CorsLayer,
    state: Arc<AppState>,
    register_limit: AxumGovernorLayer<std::net::IpAddr>,
    login_limit: AxumGovernorLayer<std::net::IpAddr>,
) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/auth/register",
            post(modules::auth::handler::register)
                .route_layer(register_limit),
        )
        .route(
            "/api/v1/auth/login",
            post(modules::auth::handler::login)
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::shared::security::login_rate_limit,
                ))
                .route_layer(login_limit),
        )
        .route("/api/v1/auth/logout", post(modules::auth::handler::logout))
        .route(
            "/api/v1/auth/refresh",
            post(modules::auth::handler::refresh_token),
        )
        .layer(cors)
}

// ---------------------------------------------------------------------------
// 子函数：公共 API 路由
// ---------------------------------------------------------------------------

/// 公共 API — 用户信息、文章、分类、标签
fn public_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

// ---------------------------------------------------------------------------
// 子函数：管理后台路由（按领域拆分，每个子函数 ≤ 40 行）
// ---------------------------------------------------------------------------

/// 分类与标签管理
fn admin_category_routes() -> Router<Arc<AppState>> {
    Router::new()
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
            patch(modules::tag::handler::update_tag)
                .delete(modules::tag::handler::delete_tag),
        )
}

/// 文章与评论（帖子级别）
fn admin_post_routes() -> Router<Arc<AppState>> {
    Router::new()
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
            get(modules::post::handler::list_admin_posts)
                .post(modules::post::handler::create_post),
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
}

/// 评论管理（审核、删除、恢复、彻底删除）
fn admin_comment_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

/// 媒体资源管理
fn admin_media_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/admin/media",
            get(modules::media::handler::list_media)
                .post(modules::media::handler::upload_media),
        )
        .route(
            "/api/v1/admin/media/{id}",
            get(modules::media::handler::get_media)
                .delete(modules::media::handler::delete_media)
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
}

/// 主题管理与预览
fn admin_theme_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

/// 站点设置
fn admin_settings_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/admin/settings",
            get(modules::setting::handler::list_settings)
                .patch(modules::setting::handler::update_setting),
        )
        .route(
            "/api/v1/admin/settings/batch",
            patch(modules::setting::handler::update_settings_batch),
        )
}

/// 备份管理
fn admin_backup_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

/// 回收站
fn admin_trash_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

/// 插件管理
fn admin_plugin_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

/// Webhook 管理
fn admin_webhook_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

/// API Key 管理
fn admin_apikey_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

/// 系统信息
fn admin_sysinfo_routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/v1/admin/sysinfo",
        get(crate::modules::setting::sysinfo::sysinfo),
    )
}

/// 合并所有管理后台路由（不含插件动态路由，插件路由在 build_router 中合并）
fn admin_routes() -> Router<Arc<AppState>> {
    Router::new()
        .merge(admin_category_routes())
        .merge(admin_post_routes())
        .merge(admin_comment_routes())
        .merge(admin_media_routes())
        .merge(admin_theme_routes())
        .merge(admin_settings_routes())
        .merge(admin_backup_routes())
        .merge(admin_trash_routes())
        .merge(admin_plugin_routes())
        .merge(admin_webhook_routes())
        .merge(admin_apikey_routes())
        .merge(admin_sysinfo_routes())
}

// ---------------------------------------------------------------------------
// 子函数：绕过限流的公共端点
// ---------------------------------------------------------------------------

/// 无需限流的公共端点：health、version、turnstile-config、setup
/// 这些端点直连 localhost 时无 X-Forwarded-For，ForwardedIpExtractor 不应拦截
fn unguarded_routes() -> Router<Arc<AppState>> {
    Router::new()
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
}

// ---------------------------------------------------------------------------
// 子函数：主题渲染路由
// ---------------------------------------------------------------------------

/// 前台页面渲染、静态资源、管理后台 SPA 入口
fn theme_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(render_home_entry))
        .route("/preview", get(modules::theme::handler::preview_page))
        .route("/posts/{slug}", get(modules::theme::handler::render_post))
        .route(
            "/author/{username}",
            get(modules::theme::handler::render_author_archive),
        )
        .route("/tags", get(modules::theme::handler::render_tags_list))
        .route(
            "/categories",
            get(modules::theme::handler::render_categories_list),
        )
        .route(
            "/tags/{slug}",
            get(modules::theme::handler::render_tag_archive),
        )
        .route("/search", get(modules::theme::handler::render_search))
        .route(
            "/cookie-policy",
            get(modules::theme::handler::render_cookie_policy),
        )
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
        .route(
            "/sitemap.xml",
            get(modules::seo::sitemap::serve_sitemap),
        )
        .route(
            "/rss.xml",
            get(modules::post::feed::render_atom_feed),
        )
        .route(
            "/feed",
            get(modules::post::feed::redirect_feed_to_rss),
        )
        .route(
            "/robots.txt",
            get(modules::seo::robots::serve_robots),
        )
        .route(
            "/favicon.ico",
            get(|| async { axum::http::StatusCode::NO_CONTENT }),
        )
        .route("/ws/admin", get(ws::ws_admin_handler))
        .route("/ws/public", get(ws::ws_public_handler))
}

// ---------------------------------------------------------------------------
// 组装函数
// ---------------------------------------------------------------------------

pub async fn build_router(state: Arc<AppState>) -> Router {
    let cors = configure_cors(&state);

    // M-1: 解析可信代理 IP 列表
    let trusted_proxies: Vec<std::net::IpAddr> = state
        .config
        .server
        .trusted_proxies
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();

    let RateLimitLayers {
        register,
        login,
        api,
    } = configure_rate_limits(trusted_proxies);

    let auth = auth_routes(cors.clone(), state.clone(), register, login);
    let unguarded = unguarded_routes();
    let theme = theme_routes(state.clone());

    // HTTP tracing 层 — 闭包类型无法具名，内联于此
    let trace_layer = {
        use crate::shared::request_id::current_request_id;

        TraceLayer::new_for_http()
            .make_span_with(|request: &axum::http::Request<axum::body::Body>| {
                let request_id = current_request_id().unwrap_or_else(|| "unknown".into());
                tracing::info_span!(
                    "http_request",
                    client_request_id = %request_id,
                    method = %request.method(),
                    uri = %request.uri(),
                    latency_ms = tracing::field::Empty,
                )
            })
            .on_response(
                |response: &axum::http::Response<_>,
                 latency: std::time::Duration,
                 span: &tracing::Span| {
                    let status = response.status().as_u16();
                    span.record("latency_ms", latency.as_millis() as u64);
                    tracing::info!(
                        target: "colophon::http",
                        status = status,
                        latency_ms = latency.as_millis(),
                        "response completed"
                    );
                },
            )
    };

    // v1 API：公共路由 + 管理路由 + 插件动态路由，共享同一套限流和 CORS
    // 插件路由在此处合并（而非 admin_routes 内），因为 collect_routes 返回
    // Router<Arc<AppState>>，需要与子函数的返回类型保持一致。
    let v1 = public_routes()
        .merge(admin_routes())
        .merge(state.plugin_manager.read().await.collect_routes(&state))
        .layer(api)
        .layer(axum::middleware::from_fn(
            crate::shared::security::log_rate_limited,
        ))
        .layer(cors.clone());

    let router = theme
        .merge(unguarded)
        .merge(auth)
        .merge(v1)
        .layer(
            ServiceBuilder::new()
                .layer(trace_layer)
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
        .with_state(state.clone());

    // M-2: Swagger UI 仅在非生产环境启用，防止泄露 API 端点
    if state.config.is_production() {
        router
    } else {
        router.merge(
            utoipa_swagger_ui::SwaggerUi::new("/api/docs")
                .url("/api-docs/openapi.json", crate::bootstrap::openapi::ApiDoc::openapi()),
        )
    }
}
