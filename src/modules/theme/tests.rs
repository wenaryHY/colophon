#[cfg(test)]
mod delete_theme_tests {
    use axum::body::Body;
    use axum::http::{header, Method, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::bootstrap::config::{
        AppConfig, AuthConfig, DatabaseConfig, PathsConfig, RuntimeConfig, ServerConfig,
        StorageConfig, ThemeConfig, WebhookConfig,
    };
    use crate::modules::setup::domain::SetupStage;
    use crate::shared::role::Role;
    use crate::state::AppState;
    use crate::ws::ServerEvent;

    const TEST_JWT_SECRET: &str = "test-secret-for-theme-integration-tests";

    /// 创建测试用内存数据库并运行所有 migrations
    async fn setup_test_db() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite for theme tests");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations for theme tests");
        pool
    }

    /// 签发一个 admin JWT，注入 session cookie 用
    fn issue_admin_jwt_token() -> String {
        crate::infra::jwt::issue_token(
            TEST_JWT_SECRET,
            3600,
            "test-admin-id".to_string(),
            "testadmin".to_string(),
            Role::Admin,
            1,
        )
        .expect("issue admin JWT token for theme test")
    }

    /// 创建只包含 delete_theme 路由的测试用 Router。
    ///
    /// 复用与 `auth/service_tests::setup_test_state()` 相同的 AppState 构造模式，
    /// 但仅注册删除主题这一个管理端点，避免引入整个 build_router 的副作用。
    async fn setup_test_router() -> axum::Router {
        let pool = setup_test_db().await;

        // 创建临时主题目录（空目录即可，handler 在 slug == "default" 时提前返回）
        let temp_theme_dir = std::env::temp_dir()
            .join(format!("colophon-theme-tests-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_theme_dir)
            .expect("create temp theme dir for tests");

        let config = AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                graceful_shutdown_timeout_seconds: 30,
            },
            database: DatabaseConfig {
                url: "sqlite::memory:".to_string(),
            },
            auth: AuthConfig {
                secret: TEST_JWT_SECRET.to_string(),
                expires_in_seconds: 3600,
                turnstile_secret: String::new(),
                turnstile_site_key: String::new(),
                cookie_secure: false,
            },
            storage: StorageConfig {
                upload_dir: "uploads".to_string(),
                max_upload_size_mb: 10,
                static_dir: "static".to_string(),
            },
            theme: ThemeConfig {
                theme_dir: temp_theme_dir.to_string_lossy().to_string(),
                active_theme_fallback: "default".to_string(),
                default_mode: "system".to_string(),
            },
            paths: PathsConfig {
                admin_dist_dir: "admin/dist".to_string(),
            },
            runtime: RuntimeConfig {
                mode: "test".to_string(),
            },
            webhook: WebhookConfig {
                max_concurrency: 5,
                timeout_seconds: 60,
            },
            site: crate::bootstrap::config::SiteConfig {
                site_timezone: "UTC".to_string(),
            },
            media: crate::bootstrap::config::MediaConfig {
                webp_enabled: false,
                webp_quality: 80,
                webp_max_edge: 2048,
                webp_max_concurrent: 1,
            },
        };

        let (event_tx, _) = tokio::sync::broadcast::channel::<ServerEvent>(256);
        let plugin_manager = Arc::new(tokio::sync::RwLock::new(
            crate::modules::plugin::manager::PluginManager::load().await,
        ));

        let state = Arc::new(
            AppState::new(
                config,
                pool,
                event_tx,
                "http://localhost:8080".to_string(),
                "http://localhost:8080/admin".to_string(),
                SetupStage::Completed,
                plugin_manager,
            )
            .unwrap(),
        );

        axum::Router::new()
            .route(
                "/api/v1/admin/themes/{slug}",
                axum::routing::delete(crate::modules::theme::handler::delete_theme),
            )
            .with_state(state)
    }

    /// 构造带 admin JWT session cookie 的 HTTP 请求
    fn build_admin_request(method: Method, uri: &str) -> axum::http::Request<Body> {
        let token = issue_admin_jwt_token();
        let cookie_value = format!(
            "{}={}; Path=/",
            crate::shared::auth_constants::SESSION_COOKIE_NAME_FOR_JWT_ACCESS_TOKEN,
            token
        );
        axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, cookie_value)
            .body(Body::empty())
            .expect("build test request")
    }

    #[tokio::test]
    async fn cannot_delete_default_theme() {
        let router = setup_test_router().await;
        let request = build_admin_request(Method::DELETE, "/api/v1/admin/themes/default");

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "default theme must not be deletable"
        );
    }

    #[tokio::test]
    async fn rejects_path_traversal_via_dot_dot_slug() {
        let router = setup_test_router().await;
        // axum 的 {slug} 只匹配单一路径段，`..` 作为一个段被捕获
        let request = build_admin_request(Method::DELETE, "/api/v1/admin/themes/..");

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "path traversal slug must be rejected by validate_theme_slug_is_safe"
        );
    }

    #[tokio::test]
    async fn returns_not_found_for_nonexistent_theme() {
        let router = setup_test_router().await;
        let request =
            build_admin_request(Method::DELETE, "/api/v1/admin/themes/nonexistent-xyz");

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "nonexistent theme must return 404"
        );
    }
}
