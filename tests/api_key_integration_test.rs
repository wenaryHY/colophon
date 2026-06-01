use inkforge::serve;
use std::time::Duration;
use tokio::time::sleep;

const API_KEY_TEST_PORT: u16 = 2002;
const API_KEY_TEST_BASE: &str = "http://127.0.0.1:2002";
const SETUP_ADMIN_PASSWORD: &str = "admin123";

/// 启动测试服务器并等待健康检查通过
async fn start_server_and_wait_ready(port: u16) -> reqwest::Client {
    std::env::set_var("INKFORGE__DATABASE__URL", "sqlite::memory:");
    std::env::set_var("INKFORGE__SERVER__PORT", port.to_string());
    std::env::set_var("INKFORGE__STORAGE__UPLOAD_DIR", "target_tmp_test_api_key_uploads");
    std::env::set_var("INKFORGE__THEME__THEME_DIR", "target_tmp_test_api_key_themes");

    tokio::spawn(async {
        if let Err(e) = serve().await {
            eprintln!("Test server crashed: {}", e);
        }
    });

    let client = reqwest::Client::new();
    let health_url = format!("http://127.0.0.1:{}/api/v1/health", port);

    for _ in 0..30 {
        if let Ok(resp) = client.get(&health_url).send().await {
            if resp.status().is_success() {
                return client;
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
    panic!("Test server did not start in time on port {}", port);
}

/// 调用 setup API 创建管理员
async fn setup_admin(client: &reqwest::Client, base: &str) {
    let resp = client
        .post(format!("{}/api/v1/setup/initialize", base))
        .json(&serde_json::json!({
            "site_title": "Test Site",
            "site_description": "A test site for API key tests",
            "site_url": "http://localhost:2002",
            "admin_url": "http://localhost:2002/admin",
            "allow_register": false,
            "username": "admin",
            "email": "admin@test.local",
            "password": SETUP_ADMIN_PASSWORD,
            "display_name": "Test Admin"
        }))
        .send()
        .await
        .expect("Setup initialization request failed");

    assert!(
        resp.status().is_success(),
        "Setup initialization returned {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
}

/// 登录管理员，从 Set-Cookie 头提取 session cookie（包含多个 Set-Cookie）
async fn login_as_admin(client: &reqwest::Client, base: &str) -> String {
    let resp = client
        .post(format!("{}/api/v1/auth/login", base))
        .json(&serde_json::json!({
            "login": "admin",
            "password": SETUP_ADMIN_PASSWORD
        }))
        .send()
        .await
        .expect("Login request failed");

    assert!(
        resp.status().is_success(),
        "Login returned {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    extract_session_cookie_from_response(&resp)
}

/// 从响应的 Set-Cookie 头中提取 inkforge_session cookie 值
fn extract_session_cookie_from_response(resp: &reqwest::Response) -> String {
    let headers = resp.headers();
    let all_cookies: Vec<String> = headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect();

    for cookie_str in &all_cookies {
        if cookie_str.starts_with("inkforge_session=") {
            // 提取值部分 (inkforge_session=TOKEN; ...)
            let value_part = cookie_str
                .strip_prefix("inkforge_session=")
                .unwrap();
            let token = value_part
                .split(';')
                .next()
                .unwrap_or("")
                .to_string();
            if !token.is_empty() {
                return token;
            }
        }
    }

    panic!(
        "No inkforge_session cookie found in Set-Cookie headers: {:?}",
        all_cookies
    );
}

/// 创建带 session cookie 的请求构建器
fn with_session(client: &reqwest::Client, method: reqwest::Method, url: &str, cookie: &str) -> reqwest::RequestBuilder {
    client
        .request(method, url)
        .header("Cookie", format!("inkforge_session={}", cookie))
}

#[tokio::test]
async fn test_api_key_lifecycle() {
    let client = start_server_and_wait_ready(API_KEY_TEST_PORT).await;
    let base = API_KEY_TEST_BASE;

    // ── 1. 初始化安装 + 登录 ──
    setup_admin(&client, base).await;
    let session_cookie = login_as_admin(&client, base).await;

    // ── 2. POST /api/v1/admin/api-keys 创建 API Key ──
    let create_resp = with_session(
        &client,
        reqwest::Method::POST,
        &format!("{}/api/v1/admin/api-keys", base),
        &session_cookie,
    )
    .json(&serde_json::json!({"name": "my-test-key"}))
    .send()
    .await
    .expect("Create API key request failed");

    assert_eq!(
        create_resp.status().as_u16(),
        200,
        "Create API key should return 200, got {}",
        create_resp.status()
    );

    let create_body: serde_json::Value = create_resp.json().await.unwrap();
    let api_key_data = &create_body["data"];
    let full_key = api_key_data["api_key"].as_str().expect("api_key field missing in response");
    let key_id = api_key_data["id"].as_str().expect("id field missing in response");

    // 断言返回完整 key（ink_ 前缀）
    assert!(
        full_key.starts_with("ink_"),
        "API key should start with 'ink_', got: {}",
        full_key
    );
    assert!(
        full_key.len() > 12,
        "API key should be longer than prefix, got length {}",
        full_key.len()
    );
    assert_eq!(
        api_key_data["name"].as_str().unwrap(),
        "my-test-key",
        "API key name mismatch"
    );

    // ── 3. GET /api/v1/admin/api-keys 列表，断言包含刚创建的 key ──
    let list_resp = with_session(
        &client,
        reqwest::Method::GET,
        &format!("{}/api/v1/admin/api-keys", base),
        &session_cookie,
    )
    .send()
    .await
    .expect("List API keys request failed");

    assert_eq!(
        list_resp.status().as_u16(),
        200,
        "List API keys should return 200, got {}",
        list_resp.status()
    );

    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let items = list_body["data"].as_array().expect("data should be an array");
    assert!(!items.is_empty(), "API key list should not be empty");

    let found = items.iter().any(|item| item["id"].as_str() == Some(key_id));
    assert!(found, "Created API key should appear in list");

    // ── 4. 用 API Key 访问需认证的公开 API → GET /api/v1/me → 断言 200 ──
    let me_resp = client
        .get(format!("{}/api/v1/me", base))
        .header("X-API-Key", full_key)
        .send()
        .await
        .expect("GET /api/v1/me with API key failed");

    assert_eq!(
        me_resp.status().as_u16(),
        200,
        "GET /api/v1/me with API key should return 200, got {}",
        me_resp.status()
    );

    // ── 5. 用 API Key 访问管理 API → GET /api/v1/admin/api-keys → 断言 403 ──
    let admin_resp = client
        .get(format!("{}/api/v1/admin/api-keys", base))
        .header("X-API-Key", full_key)
        .send()
        .await
        .expect("Admin API request with API key failed");

    assert_eq!(
        admin_resp.status().as_u16(),
        403,
        "GET /api/v1/admin/api-keys with API key should return 403 Forbidden, got {}",
        admin_resp.status()
    );

    let admin_error_body: serde_json::Value = admin_resp.json().await.unwrap();
    assert_eq!(
        admin_error_body["code"].as_i64(),
        Some(40300),
        "Forbidden response should have code 40300"
    );

    // ── 6. DELETE /api/v1/admin/api-keys/:id 撤销 ──
    let revoke_resp = with_session(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/api/v1/admin/api-keys/{}", base, key_id),
        &session_cookie,
    )
    .send()
    .await
    .expect("Revoke API key request failed");

    assert_eq!(
        revoke_resp.status().as_u16(),
        200,
        "Revoke API key should return 200, got {}",
        revoke_resp.status()
    );

    let revoke_body: serde_json::Value = revoke_resp.json().await.unwrap();
    assert_eq!(
        revoke_body["data"]["revoked"].as_bool(),
        Some(true),
        "Revoke response should indicate revoked"
    );

    // ── 7. 用已撤销的 API Key 访问需认证接口 → 断言 401 ──
    let revoked_resp = client
        .get(format!("{}/api/v1/me", base))
        .header("X-API-Key", full_key)
        .send()
        .await
        .expect("Revoked API key request failed");

    assert_eq!(
        revoked_resp.status().as_u16(),
        401,
        "GET /api/v1/me with revoked API key should return 401 Unauthorized, got {}",
        revoked_resp.status()
    );

    let revoked_error_body: serde_json::Value = revoked_resp.json().await.unwrap();
    assert_eq!(
        revoked_error_body["code"].as_i64(),
        Some(40100),
        "Unauthorized response should have code 40100"
    );

    // 清理临时目录
    let _ = std::fs::remove_dir_all("target_tmp_test_api_key_uploads");
    let _ = std::fs::remove_dir_all("target_tmp_test_api_key_themes");
}
