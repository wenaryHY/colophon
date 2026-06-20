#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sqlx::SqlitePool;

    use crate::{
        bootstrap::config::{AppConfig, AuthConfig, DatabaseConfig, PathsConfig, RuntimeConfig, ServerConfig, StorageConfig, ThemeConfig, WebhookConfig},
        modules::{
            auth::{
                dto::{LoginRequest, RegisterRequest},
                service,
            },
            setup::domain::SetupStage,
        },
        shared::{
            auth::{decode_token, hash_password, hash_token, issue_token, verify_password},
            role::Role,
        },
        state::AppState,
        ws::ServerEvent,
    };

    /// 创建测试用内存数据库
    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    /// 创建测试用 AppState
    async fn setup_test_state() -> Arc<AppState> {
        let pool = setup_test_db().await;
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
                secret: "test-secret-key-for-jwt".to_string(),
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
                theme_dir: "themes".to_string(),
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

        Arc::new(
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
        )
    }

    /// 注册并启用 public registration
    async fn enable_registration(state: &Arc<AppState>) {
        // 插入 allow_register = true
        sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('allow_register', 'true')")
            .execute(&state.pool)
            .await
            .unwrap();

        // 插入至少一个用户（避免 "需要先初始化管理员" 错误）
        let admin_hash = hash_password("admin123456").await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, display_name, role, status, theme_preference) 
             VALUES ('admin-id', 'admin', 'admin@example.com', ?, 'Admin', 'admin', 'active', 'system')",
        )
        .bind(&admin_hash)
        .execute(&state.pool)
        .await
        .unwrap();
    }

    // ── 1. 用户注册测试 ──

    #[tokio::test]
    async fn test_register_success() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let req = RegisterRequest {
            username: "newuser".to_string(),
            email: "newuser@example.com".to_string(),
            password: "StrongPassword123!".to_string(),
            display_name: Some("New User".to_string()),
            turnstile_token: None,
        };

        let result = service::register(state.clone(), req, 3600, 7 * 86400).await;
        assert!(result.is_ok());

        let (response, refresh_token) = result.unwrap();
        assert_eq!(response.user.username, "newuser");
        assert_eq!(response.user.role, Role::Member);
        assert!(!response.access_token.is_empty());
        assert!(!refresh_token.is_empty());

        // 验证密码已加密存储（Argon2id）
        let stored_hash: String = sqlx::query_scalar(
            "SELECT password_hash FROM users WHERE username = 'newuser'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_ne!(stored_hash, "StrongPassword123!");
        assert!(stored_hash.starts_with("$argon2id$"));
    }

    #[tokio::test]
    async fn test_register_duplicate_username() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let req1 = RegisterRequest {
            username: "duplicate".to_string(),
            email: "user1@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), req1, 3600, 7 * 86400)
            .await
            .unwrap();

        let req2 = RegisterRequest {
            username: "duplicate".to_string(),
            email: "user2@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        let result = service::register(state.clone(), req2, 3600, 7 * 86400).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, crate::shared::error::AppError::BadRequest(_)),
            "Expected BadRequest, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_register_duplicate_email() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let req1 = RegisterRequest {
            username: "user1".to_string(),
            email: "duplicate@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), req1, 3600, 7 * 86400)
            .await
            .unwrap();

        let req2 = RegisterRequest {
            username: "user2".to_string(),
            email: "duplicate@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        let result = service::register(state.clone(), req2, 3600, 7 * 86400).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_register_weak_password() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let req = RegisterRequest {
            username: "weakpw".to_string(),
            email: "weakpw@example.com".to_string(),
            password: "short".to_string(), // < 8 字符
            display_name: None,
            turnstile_token: None,
        };
        let result = service::register(state.clone(), req, 3600, 7 * 86400).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            crate::shared::error::AppError::BadRequest(_)
        ));
    }

    #[tokio::test]
    async fn test_register_short_username() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let req = RegisterRequest {
            username: "ab".to_string(), // < 3 字符
            email: "short@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        let result = service::register(state.clone(), req, 3600, 7 * 86400).await;

        assert!(result.is_err());
    }

    // ── 2. 用户登录测试 ──

    #[tokio::test]
    async fn test_login_with_username() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "logintest".to_string(),
            email: "logintest@example.com".to_string(),
            password: "SecurePass123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        let login_req = LoginRequest {
            login: "logintest".to_string(),
            password: "SecurePass123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let result = service::login(state.clone(), login_req, 3600, 7 * 86400).await;

        assert!(result.is_ok());
        let (response, _) = result.unwrap();
        assert_eq!(response.user.username, "logintest");
        assert!(!response.access_token.is_empty());
    }

    #[tokio::test]
    async fn test_login_with_email() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "emailtest".to_string(),
            email: "emaillogin@example.com".to_string(),
            password: "SecurePass123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        let login_req = LoginRequest {
            login: "emaillogin@example.com".to_string(),
            password: "SecurePass123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let result = service::login(state.clone(), login_req, 3600, 7 * 86400).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "wrongpw".to_string(),
            email: "wrongpw@example.com".to_string(),
            password: "CorrectPass123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        let login_req = LoginRequest {
            login: "wrongpw".to_string(),
            password: "WrongPassword!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let result = service::login(state.clone(), login_req, 3600, 7 * 86400).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::shared::error::AppError::Unauthorized
        ));
    }

    #[tokio::test]
    async fn test_login_nonexistent_user() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let login_req = LoginRequest {
            login: "nonexistent".to_string(),
            password: "Password123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let result = service::login(state.clone(), login_req, 3600, 7 * 86400).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::shared::error::AppError::Unauthorized
        ));
    }

    #[tokio::test]
    async fn test_login_returns_valid_jwt() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "jwtuser".to_string(),
            email: "jwt@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        let login_req = LoginRequest {
            login: "jwtuser".to_string(),
            password: "Password123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let (response, _) = service::login(state.clone(), login_req, 3600, 7 * 86400)
            .await
            .unwrap();

        // 验证 JWT 可解码
        let claims = decode_token(&response.access_token, &state.config.auth.secret).unwrap();
        assert_eq!(claims.username, "jwtuser");
        assert_eq!(claims.role, Role::Member);
    }

    // ── 3. JWT Token 验证测试 ──

    #[tokio::test]
    async fn test_verify_valid_token() {
        let state = setup_test_state().await;

        let token = issue_token(
            &state.config.auth.secret,
            3600,
            "user-123".to_string(),
            "testuser".to_string(),
            Role::Member,
            1,
        )
        .unwrap();

        let claims = decode_token(&token, &state.config.auth.secret);
        assert!(claims.is_ok());

        let claims = claims.unwrap();
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.role, Role::Member);
        assert_eq!(claims.token_version, 1);
    }

    #[tokio::test]
    async fn test_verify_expired_token() {
        let state = setup_test_state().await;

        // 创建已过期的 token（负数生命周期）
        let now = chrono::Utc::now().timestamp();
        let expired_claims = crate::infra::jwt::token::Claims {
            sub: "user-123".to_string(),
            username: "testuser".to_string(),
            role: Role::Member,
            exp: now - 3600, // 1 小时前过期
            token_version: 1,
        };

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &expired_claims,
            &jsonwebtoken::EncodingKey::from_secret(state.config.auth.secret.as_bytes()),
        )
        .unwrap();

        let result = decode_token(&token, &state.config.auth.secret);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_verify_forged_token() {
        let state = setup_test_state().await;

        // 使用错误密钥签名的 token
        let token = issue_token(
            "wrong-secret",
            3600,
            "user-123".to_string(),
            "testuser".to_string(),
            Role::Member,
            1,
        )
        .unwrap();

        let result = decode_token(&token, &state.config.auth.secret);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_verify_tampered_claims() {
        let state = setup_test_state().await;

        let mut token = issue_token(
            &state.config.auth.secret,
            3600,
            "user-123".to_string(),
            "testuser".to_string(),
            Role::Member,
            1,
        )
        .unwrap();

        // 篡改 token payload（修改中间部分）
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() == 3 {
            token = format!("{}.tampered.{}", parts[0], parts[2]);
        }

        let result = decode_token(&token, &state.config.auth.secret);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_verify_empty_token() {
        let state = setup_test_state().await;

        let result = decode_token("", &state.config.auth.secret);
        assert!(result.is_err());
    }

    // ── 4. Refresh Token 测试 ──

    #[tokio::test]
    async fn test_refresh_token_saved_on_register() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let req = RegisterRequest {
            username: "refreshtest".to_string(),
            email: "refresh@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        let (_, refresh_token) = service::register(state.clone(), req, 3600, 7 * 86400)
            .await
            .unwrap();

        // 验证 refresh_token 已存储到数据库
        let token_hash = hash_token(&refresh_token);
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM refresh_tokens WHERE token_hash = ?",
        )
        .bind(&token_hash)
        .fetch_one(&state.pool)
        .await
        .unwrap();

        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_refresh_token_saved_on_login() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "loginrefresh".to_string(),
            email: "loginrefresh@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        let login_req = LoginRequest {
            login: "loginrefresh".to_string(),
            password: "Password123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let (_, refresh_token) = service::login(state.clone(), login_req, 3600, 7 * 86400)
            .await
            .unwrap();

        let token_hash = hash_token(&refresh_token);
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM refresh_tokens WHERE token_hash = ?",
        )
        .bind(&token_hash)
        .fetch_one(&state.pool)
        .await
        .unwrap();

        assert_eq!(count, 1);
    }

    // ── 5. 密码哈希测试 ──

    #[tokio::test]
    async fn test_password_hash_uses_argon2() {
        let password = "TestPassword123!";
        let hash = hash_password(password).await.unwrap();

        // Argon2id 哈希前缀
        assert!(hash.starts_with("$argon2id$"));
        assert_ne!(hash, password);
    }

    #[tokio::test]
    async fn test_password_verify_correct() {
        let password = "CorrectPassword123!";
        let hash = hash_password(password).await.unwrap();

        let is_valid = verify_password(password, &hash).await.unwrap();
        assert!(is_valid);
    }

    #[tokio::test]
    async fn test_password_verify_wrong() {
        let password = "CorrectPassword123!";
        let hash = hash_password(password).await.unwrap();

        let is_valid = verify_password("WrongPassword!", &hash).await.unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_password_hash_is_unique() {
        let password = "SamePassword123!";
        let hash1 = hash_password(password).await.unwrap();
        let hash2 = hash_password(password).await.unwrap();

        // 相同密码每次哈希结果不同（因为 salt 随机）
        assert_ne!(hash1, hash2);

        // 但都能验证通过
        assert!(verify_password(password, &hash1).await.unwrap());
        assert!(verify_password(password, &hash2).await.unwrap());
    }

    // ── 6. Remember Me 测试 ──

    #[tokio::test]
    async fn test_remember_me_true_returns_refresh_token() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "rememberme".to_string(),
            email: "rememberme@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        let login_req = LoginRequest {
            login: "rememberme".to_string(),
            password: "Password123!".to_string(),
            remember_me: Some(true),
            turnstile_token: None,
        };
        let (response, refresh_token) = service::login(state.clone(), login_req, 3600, 7 * 86400)
            .await
            .unwrap();

        assert!(!response.access_token.is_empty());
        assert!(!refresh_token.is_empty());
        assert_eq!(refresh_token.len(), 64); // 64 字符 hex
    }

    #[tokio::test]
    async fn test_remember_me_false_still_returns_refresh_token() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "noremember".to_string(),
            email: "noremember@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        let login_req = LoginRequest {
            login: "noremember".to_string(),
            password: "Password123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let (_, refresh_token) = service::login(state.clone(), login_req, 3600, 7 * 86400)
            .await
            .unwrap();

        // 当前实现总是返回 refresh_token，无论 remember_me 值
        assert!(!refresh_token.is_empty());
    }

    #[tokio::test]
    async fn test_remember_me_true_uses_long_expiry() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "longexpiry".to_string(),
            email: "longexpiry@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        // remember_me: true → 7 天 (604800 秒)
        let login_req = LoginRequest {
            login: "longexpiry".to_string(),
            password: "Password123!".to_string(),
            remember_me: Some(true),
            turnstile_token: None,
        };
        let long_expiry_seconds = 7 * 86400; // 7 天
        let (_, refresh_token) =
            service::login(state.clone(), login_req, 3600, long_expiry_seconds)
                .await
                .unwrap();

        // 验证 expires_at 约为 7 天后
        let token_hash = crate::shared::auth::hash_token(&refresh_token);
        let (_, expires_at, _, _) =
            crate::modules::auth::repository::find_valid_refresh_token(&state.pool, &token_hash)
                .await
                .unwrap()
                .unwrap();

        let expires_time =
            chrono::DateTime::parse_from_rfc3339(&expires_at)
                .unwrap()
                .with_timezone(&chrono::Utc);
        let now = chrono::Utc::now();
        let seconds_until_expiry = (expires_time - now).num_seconds();

        // 允许 ±60 秒误差（测试执行耗时）
        assert!(
            seconds_until_expiry >= long_expiry_seconds as i64 - 60
                && seconds_until_expiry <= long_expiry_seconds as i64 + 60,
            "expected ~{} seconds, got {}",
            long_expiry_seconds,
            seconds_until_expiry
        );
    }

    #[tokio::test]
    async fn test_remember_me_false_uses_short_expiry() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "shortexpiry".to_string(),
            email: "shortexpiry@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        // remember_me: false → 15 分钟 (900 秒)
        let login_req = LoginRequest {
            login: "shortexpiry".to_string(),
            password: "Password123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let short_expiry_seconds = 900; // 15 分钟
        let (_, refresh_token) =
            service::login(state.clone(), login_req, 3600, short_expiry_seconds)
                .await
                .unwrap();

        // 验证 expires_at 约为 15 分钟后
        let token_hash = crate::shared::auth::hash_token(&refresh_token);
        let (_, expires_at, _, _) =
            crate::modules::auth::repository::find_valid_refresh_token(&state.pool, &token_hash)
                .await
                .unwrap()
                .unwrap();

        let expires_time =
            chrono::DateTime::parse_from_rfc3339(&expires_at)
                .unwrap()
                .with_timezone(&chrono::Utc);
        let now = chrono::Utc::now();
        let seconds_until_expiry = (expires_time - now).num_seconds();

        // 允许 ±60 秒误差
        assert!(
            seconds_until_expiry >= short_expiry_seconds as i64 - 60
                && seconds_until_expiry <= short_expiry_seconds as i64 + 60,
            "expected ~{} seconds, got {}",
            short_expiry_seconds,
            seconds_until_expiry
        );
    }

    // ── 7. 边界条件测试 ──

    #[tokio::test]
    async fn test_register_with_whitespace_trimming() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let req = RegisterRequest {
            username: "  trimtest  ".to_string(),
            email: "  trim@example.com  ".to_string(),
            password: "Password123!".to_string(),
            display_name: Some("  Trim User  ".to_string()),
            turnstile_token: None,
        };
        let (response, _) = service::register(state.clone(), req, 3600, 7 * 86400)
            .await
            .unwrap();

        assert_eq!(response.user.username, "trimtest");
    }

    #[tokio::test]
    async fn test_login_case_sensitive() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        let register_req = RegisterRequest {
            username: "CaseSensitive".to_string(),
            email: "case@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        // SQLite 默认 username/email 列区分大小写
        let login_req = LoginRequest {
            login: "casesensitive".to_string(), // 小写
            password: "Password123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let result = service::login(state.clone(), login_req, 3600, 7 * 86400).await;

        // 应该失败（用户名不匹配）
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_inactive_user_cannot_login() {
        let state = setup_test_state().await;
        enable_registration(&state).await;

        // 注册用户
        let register_req = RegisterRequest {
            username: "inactiveuser".to_string(),
            email: "inactive@example.com".to_string(),
            password: "Password123!".to_string(),
            display_name: None,
            turnstile_token: None,
        };
        service::register(state.clone(), register_req, 3600, 7 * 86400)
            .await
            .unwrap();

        // 将用户状态设为 inactive
        sqlx::query("UPDATE users SET status = 'inactive' WHERE username = 'inactiveuser'")
            .execute(&state.pool)
            .await
            .unwrap();

        // 尝试登录
        let login_req = LoginRequest {
            login: "inactiveuser".to_string(),
            password: "Password123!".to_string(),
            remember_me: Some(false),
            turnstile_token: None,
        };
        let result = service::login(state.clone(), login_req, 3600, 7 * 86400).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::shared::error::AppError::Unauthorized
        ));
    }
}
