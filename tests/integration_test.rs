use colophon::serve;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_setup_flow_integration() {
    // 独立测试环境隔离
    std::env::set_var("COLOPHON__DATABASE__URL", "sqlite::memory:");
    std::env::set_var("COLOPHON__SERVER__PORT", "2001");
    // 给测试分配特定的目录，避免污染本地开发文件
    std::env::set_var("COLOPHON__STORAGE__UPLOAD_DIR", "target_tmp_test_uploads");
    std::env::set_var("COLOPHON__THEME__THEME_DIR", "target_tmp_test_themes");

    // 后台启动服务器
    tokio::spawn(async {
        if let Err(e) = serve().await {
            eprintln!("Test server crashed: {}", e);
        }
    });

    // 轮询等待端口就绪
    let client = reqwest::Client::new();
    let health_url = "http://127.0.0.1:2001/api/v1/health";

    let mut ready = false;
    for _ in 0..20 {
        if let Ok(resp) = client.get(health_url).send().await {
            if resp.status().is_success() {
                ready = true;
                break;
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Test server did not start in time");

    // 测试 1: 验证初始状态为 not_started
    let status_url = "http://127.0.0.1:2001/api/v1/setup/status";
    let resp: serde_json::Value = client
        .get(status_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let stage = resp["data"]["stage"].as_str().unwrap();
    assert_eq!(stage, "not_started");

    // 清理创建的临时目录
    let _ = std::fs::remove_dir_all("target_tmp_test_uploads");
    let _ = std::fs::remove_dir_all("target_tmp_test_themes");
}
