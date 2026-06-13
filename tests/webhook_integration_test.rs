use colophon::serve;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};

const WEBHOOK_TEST_PORT: u16 = 2003;
const WEBHOOK_TEST_BASE: &str = "http://127.0.0.1:2003";
const SETUP_ADMIN_PASSWORD: &str = "admin123";

/// 缓存 Set-Cookie 头中 colophon_session cookie 的 token 值
#[derive(Debug)]
struct SessionCookie {
    token: String,
}

/// 启动测试服务器并等待健康检查通过
async fn start_server_and_wait_ready(port: u16) -> reqwest::Client {
    std::env::set_var("COLOPHON__DATABASE__URL", "sqlite::memory:");
    std::env::set_var("COLOPHON__SERVER__PORT", port.to_string());
    std::env::set_var(
        "COLOPHON__STORAGE__UPLOAD_DIR",
        "target_tmp_test_webhook_uploads",
    );
    std::env::set_var(
        "COLOPHON__THEME__THEME_DIR",
        "target_tmp_test_webhook_themes",
    );
    std::env::set_var("COLOPHON_TEST_MODE", "true");

    tokio::spawn(async {
        if let Err(e) = serve().await {
            eprintln!("Webhook test server crashed: {}", e);
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
    panic!("Webhook test server did not start in time on port {}", port);
}

/// 调用 setup API 创建管理员
async fn setup_admin(client: &reqwest::Client, base: &str) {
    let resp = client
        .post(format!("{}/api/v1/setup/initialize", base))
        .json(&serde_json::json!({
            "site_title": "Webhook Test Site",
            "site_description": "A test site for webhook tests",
            "site_url": "http://localhost:2003",
            "admin_url": "http://localhost:2003/admin",
            "allow_register": false,
            "username": "admin",
            "email": "admin@test.local",
            "password": SETUP_ADMIN_PASSWORD,
            "display_name": "Webhook Admin"
        }))
        .send()
        .await
        .expect("Webhook test setup initialization failed");

    assert!(
        resp.status().is_success(),
        "Setup returned {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
}

/// 登录管理员，返回 session cookie
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
        if let Some(token) = extract_token_from_cookie(cookie_str, "colophon_session=") {
            return SessionCookie { token };
        }
    }

    panic!("No colophon_session cookie found in: {:?}", all_cookies);
}

fn extract_token_from_cookie(cookie_str: &str, prefix: &str) -> Option<String> {
    cookie_str
        .strip_prefix(prefix)
        .and_then(|rest| rest.split(';').next())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
}

/// 向请求添加 session cookie
fn add_session_cookie(
    request: reqwest::RequestBuilder,
    cookie: &SessionCookie,
) -> reqwest::RequestBuilder {
    request.header("Cookie", format!("colophon_session={}", cookie.token))
}

/// 本地 HTTP 捕获服务器，用于接收 webhook 请求
///
/// 返回 (port, rx)，其中 rx 一旦收到请求即返回接收到的原始 HTTP 请求体。
/// 服务器会自动回复 HTTP 200 使 webhook 记录为成功。
async fn start_webhook_capture_server() -> (u16, tokio::sync::oneshot::Receiver<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind capture server");
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();

    tokio::spawn(async move {
        let (mut stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(_) => {
                let _ = tx.send(String::new());
                return;
            }
        };

        let mut buf = vec![0u8; 16384];
        let n = match stream.read(&mut buf).await {
            Ok(n) if n > 0 => n,
            _ => {
                let _ = tx.send(String::new());
                return;
            }
        };

        let raw_request = String::from_utf8_lossy(&buf[..n]).to_string();

        // 解析 HTTP 请求体：查找 "\r\n\r\n" 分隔符
        let body = if let Some(body_start) = raw_request.find("\r\n\r\n") {
            raw_request[body_start + 4..].to_string()
        } else {
            raw_request.clone()
        };

        // 返回 HTTP 200 使 webhook 记录成功
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;

        let _ = tx.send(body);
    });

    (port, rx)
}

#[tokio::test]
async fn test_webhook_trigger_on_publish() {
    let client = start_server_and_wait_ready(WEBHOOK_TEST_PORT).await;
    let base = WEBHOOK_TEST_BASE;

    // ── 1. 初始化安装 + 登录 ──
    setup_admin(&client, base).await;
    let session = login_as_admin(&client, base).await;

    // ── 2. 启动本地 HTTP 捕获服务器 ──
    let (capture_port, capture_rx) = start_webhook_capture_server().await;
    let webhook_target_url = format!("http://127.0.0.1:{}/hook", capture_port);

    // ── 3. POST /api/v1/admin/webhooks 创建 webhook ──
    let create_webhook_resp = add_session_cookie(
        client.post(format!("{}/api/v1/admin/webhooks", base)),
        &session,
    )
    .json(&serde_json::json!({
        "name": "test-webhook",
        "url": webhook_target_url,
        "events": "post.after_publish",
        "max_retries": 0
    }))
    .send()
    .await
    .expect("Create webhook request failed");

    assert_eq!(
        create_webhook_resp.status().as_u16(),
        200,
        "Create webhook should return 200, got {}",
        create_webhook_resp.status()
    );

    let webhook_body: serde_json::Value = create_webhook_resp.json().await.unwrap();
    let webhook_id = webhook_body["data"]["id"]
        .as_str()
        .expect("webhook id missing in response")
        .to_string();

    // ── 4. POST /api/v1/admin/posts 创建并发布文章，触发 webhook ──
    let create_post_resp = add_session_cookie(
        client.post(format!("{}/api/v1/admin/posts", base)),
        &session,
    )
    .json(&serde_json::json!({
        "title": "Webhook Test Post",
        "slug": "webhook-test-post",
        "content_md": "# Hello\nThis is a webhook test post.",
        "status": "published",
        "visibility": "public"
    }))
    .send()
    .await
    .expect("Create post request failed");

    assert_eq!(
        create_post_resp.status().as_u16(),
        200,
        "Create post should return 200, got {}",
        create_post_resp.status()
    );

    let post_body: serde_json::Value = create_post_resp.json().await.unwrap();
    let post_id = post_body["data"]["id"]
        .as_str()
        .expect("post id missing in response")
        .to_string();
    let post_title = post_body["data"]["title"]
        .as_str()
        .expect("post title missing in response")
        .to_string();

    // ── 5. 等待捕获服务器收到 webhook 请求（最多 15 秒）──
    let captured_body = timeout(Duration::from_secs(15), capture_rx)
        .await
        .expect("Timed out waiting for webhook delivery")
        .expect("Webhook capture server did not produce a result");

    assert!(
        !captured_body.is_empty(),
        "Captured webhook body should not be empty"
    );

    let captured_json: serde_json::Value =
        serde_json::from_str(&captured_body).expect("Captured body should be valid JSON");

    assert_eq!(
        captured_json["event"].as_str(),
        Some("post.after_publish"),
        "Webhook event mismatch"
    );
    assert_eq!(
        captured_json["data"]["post_id"].as_str(),
        Some(post_id.as_str()),
        "Webhook payload post_id mismatch"
    );
    assert_eq!(
        captured_json["data"]["title"].as_str(),
        Some(post_title.as_str()),
        "Webhook payload title mismatch"
    );

    // ── 6. GET /api/v1/admin/webhooks/:id/deliveries ──
    // 等待投递记录写入完成（异步），短暂轮询
    let mut delivery_found = false;
    for _ in 0..10 {
        let delivery_resp = add_session_cookie(
            client.get(format!(
                "{}/api/v1/admin/webhooks/{}/deliveries",
                base, webhook_id
            )),
            &session,
        )
        .send()
        .await
        .expect("List deliveries request failed");

        assert_eq!(
            delivery_resp.status().as_u16(),
            200,
            "List deliveries should return 200"
        );

        let delivery_body: serde_json::Value = delivery_resp.json().await.unwrap();
        let total = delivery_body["data"]["total"].as_i64().unwrap_or(0);
        if total > 0 {
            let items = delivery_body["data"]["items"]
                .as_array()
                .expect("items should be array");
            let first_item = &items[0];
            assert_eq!(
                first_item["success"].as_i64(),
                Some(1),
                "Delivery should have success=1"
            );
            delivery_found = true;
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }

    assert!(
        delivery_found,
        "Webhook delivery record was not found within 5 seconds"
    );

    // 清理临时目录
    let _ = std::fs::remove_dir_all("target_tmp_test_webhook_uploads");
    let _ = std::fs::remove_dir_all("target_tmp_test_webhook_themes");
}
