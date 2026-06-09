/// SSRF 防护集成测试
/// 
/// 验证 webhook 模块正确拒绝指向内网地址的 URL
use inkforge::serve;
use std::time::Duration;
use tokio::time::sleep;

const SSRF_TEST_PORT: u16 = 2004;
const SSRF_TEST_BASE: &str = "http://127.0.0.1:2004";
const SETUP_ADMIN_PASSWORD: &str = "admin123";

#[derive(Debug)]
struct SessionCookie {
    token: String,
}

async fn start_server_and_wait_ready(port: u16) -> reqwest::Client {
    std::env::set_var("INKFORGE__DATABASE__URL", "sqlite::memory:");
    std::env::set_var("INKFORGE__SERVER__PORT", port.to_string());
    std::env::set_var("INKFORGE__STORAGE__UPLOAD_DIR", "target_tmp_test_ssrf_uploads");
    std::env::set_var("INKFORGE__THEME__THEME_DIR", "target_tmp_test_ssrf_themes");

    tokio::spawn(async {
        if let Err(e) = serve().await {
            eprintln!("SSRF test server crashed: {}", e);
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
    panic!("SSRF test server did not start in time on port {}", port);
}

async fn setup_admin(client: &reqwest::Client, base: &str) {
    let resp = client
        .post(format!("{}/api/v1/setup/initialize", base))
        .json(&serde_json::json!({
            "site_title": "SSRF Test Site",
            "site_description": "A test site for SSRF protection",
            "site_url": "http://localhost:2004",
            "admin_url": "http://localhost:2004/admin",
            "allow_register": false,
            "username": "admin",
            "email": "admin@test.local",
            "password": SETUP_ADMIN_PASSWORD,
            "display_name": "SSRF Admin"
        }))
        .send()
        .await
        .expect("SSRF test setup initialization failed");

    assert!(
        resp.status().is_success(),
        "Setup returned {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
}

async fn login_as_admin(client: &reqwest::Client, base: &str) -> SessionCookie {
    let resp = client
        .post(format!("{}/api/v1/auth/login", base))
        .json(&serde_json::json!({
            "login": "admin",
            "password": SETUP_ADMIN_PASSWORD
        }))
        .send()
        .await
        .expect("Login request failed");

    assert!(resp.status().is_success());

    let headers = resp.headers();
    let all_cookies: Vec<String> = headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect();

    for cookie_str in &all_cookies {
        if let Some(token) = extract_token_from_cookie(cookie_str, "inkforge_session=") {
            return SessionCookie { token };
        }
    }

    panic!("No inkforge_session cookie found in: {:?}", all_cookies);
}

fn extract_token_from_cookie(cookie_str: &str, prefix: &str) -> Option<String> {
    cookie_str
        .strip_prefix(prefix)
        .and_then(|rest| rest.split(';').next())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
}

fn add_session_cookie(
    request: reqwest::RequestBuilder,
    cookie: &SessionCookie,
) -> reqwest::RequestBuilder {
    request.header("Cookie", format!("inkforge_session={}", cookie.token))
}

/// 测试：拒绝指向 localhost 的 webhook
#[tokio::test]
async fn test_webhook_rejects_localhost() {
    let client = start_server_and_wait_ready(SSRF_TEST_PORT).await;
    let base = SSRF_TEST_BASE;

    setup_admin(&client, base).await;
    let session = login_as_admin(&client, base).await;

    let resp = add_session_cookie(
        client.post(format!("{}/api/v1/admin/webhooks", base)),
        &session,
    )
    .json(&serde_json::json!({
        "name": "malicious-localhost",
        "url": "http://127.0.0.1:6379/",
        "events": "post.after_publish"
    }))
    .send()
    .await
    .expect("Request should complete");

    assert_eq!(
        resp.status().as_u16(),
        400,
        "Should reject localhost with 400, got {}",
        resp.status()
    );

    let body_text = resp.text().await.unwrap();
    let body: serde_json::Value = serde_json::from_str(&body_text).unwrap();
    let message = body["message"].as_str().unwrap_or("");
    assert!(
        message.contains("内网") || message.contains("localhost"),
        "Error message should mention internal network or localhost, got: {}",
        message
    );

    // 清理
    let _ = std::fs::remove_dir_all("target_tmp_test_ssrf_uploads");
    let _ = std::fs::remove_dir_all("target_tmp_test_ssrf_themes");
}

/// 测试：拒绝指向私有 IP 的 webhook，允许公网 URL
#[tokio::test]
async fn test_webhook_ssrf_protection() {
    let client = start_server_and_wait_ready(SSRF_TEST_PORT + 50).await;
    let base = format!("http://127.0.0.1:{}", SSRF_TEST_PORT + 50);

    setup_admin(&client, &base).await;
    let session = login_as_admin(&client, &base).await;

    // 拒绝 192.168.x.x
    let resp = add_session_cookie(
        client.post(format!("{}/api/v1/admin/webhooks", &base)),
        &session,
    )
    .json(&serde_json::json!({
        "name": "malicious-192",
        "url": "http://192.168.1.1/api",
        "events": "post.after_publish"
    }))
    .send()
    .await
    .expect("Request should complete");
    assert_eq!(resp.status().as_u16(), 400, "Should reject 192.168.x.x");

    // 拒绝 10.x.x.x
    let resp2 = add_session_cookie(
        client.post(format!("{}/api/v1/admin/webhooks", &base)),
        &session,
    )
    .json(&serde_json::json!({
        "name": "malicious-10",
        "url": "http://10.0.0.1/",
        "events": "post.after_publish"
    }))
    .send()
    .await
    .expect("Request should complete");
    assert_eq!(resp2.status().as_u16(), 400, "Should reject 10.x.x.x");

    // 允许公网 URL
    let resp3 = add_session_cookie(
        client.post(format!("{}/api/v1/admin/webhooks", &base)),
        &session,
    )
    .json(&serde_json::json!({
        "name": "legitimate-webhook",
        "url": "https://webhook.site/test",
        "events": "post.after_publish"
    }))
    .send()
    .await
    .expect("Request should complete");
    
    assert_eq!(
        resp3.status().as_u16(),
        200,
        "Should allow public URL, got {}",
        resp3.status()
    );

    // 清理
    let _ = std::fs::remove_dir_all("target_tmp_test_ssrf_uploads");
    let _ = std::fs::remove_dir_all("target_tmp_test_ssrf_themes");
}
