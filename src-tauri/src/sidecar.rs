// sidecar 生命周期管理 (重构后仅保留状态查询，保持历史命名或后续再重命名)
// 负责与 In-Process Axum 联调健康检查

use std::time::Duration;

/// 等待 colophon 服务就绪（最多 10 秒）
/// 每 500ms 轮询 /api/v1/health，超时返回 Err
pub async fn wait_ready(port: u16) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/api/v1/health", port);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    let max_attempts = 20; // 20 * 500ms = 10s
    for i in 0..max_attempts {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("colophon service ready after {} attempts", i + 1);
                return Ok(());
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Err(format!(
        "colophon service did not become ready within 10 seconds on port {}",
        port
    ))
}

/// 查询安装状态，返回 setup_stage 字符串
/// 可能的值: not_started | admin_created | configured | completed
pub async fn get_setup_stage(port: u16) -> Result<String, String> {
    let url = format!("http://127.0.0.1:{}/api/v1/setup/status", port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let stage = resp
        .get("data")
        .and_then(|d| d.get("stage"))
        .and_then(|s| s.as_str())
        .unwrap_or("not_started")
        .to_string();

    Ok(stage)
}
