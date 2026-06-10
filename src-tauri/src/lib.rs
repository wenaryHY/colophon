// Colophon 桌面应用核心库
// 管理 Tauri 应用生命周期：In-Process 启动 Axum、健康检查、窗口路由

mod commands;
mod sidecar;
mod window;



#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // 在独立 tokio 任务中完成启动流程，避免阻塞 Tauri 主线程
            tauri::async_runtime::spawn(async move {
                // 1. 启动 In-Process Axum 服务
                tauri::async_runtime::spawn(async {
                    if let Err(e) = colophon::serve().await {
                        tracing::error!("Axum server exited with error: {}", e);
                    }
                });

                // 2. 等待服务就绪（健康检查）
                if let Err(e) = sidecar::wait_ready(2000).await {
                    tracing::error!("{}", e);
                    // 超时后仍打开窗口，服务可能需要更长时间
                }

                // 3. 查询安装状态，决定打开哪个窗口
                let stage = sidecar::get_setup_stage(2000)
                    .await
                    .unwrap_or_else(|_| "not_started".to_string());

                tracing::info!("setup stage: {}", stage);

                if stage == "completed" {
                    if let Err(e) = window::open_admin_window(&app_handle) {
                        tracing::error!("failed to open admin window: {}", e);
                    }
                } else {
                    if let Err(e) = window::open_setup_window(&app_handle) {
                        tracing::error!("failed to open setup window: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::switch_to_admin,
            commands::get_server_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            // In-Process 模式下，当窗口全部关闭导致 Tauri 退出时，整个进程会自动结束，不需要手动 kill
        });
}
