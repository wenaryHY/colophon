// 双窗口管理
// setup_window: 安装引导窗口（小尺寸）
// admin_window: 后台管理窗口（大尺寸）

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const SETUP_WINDOW_LABEL: &str = "setup";
const ADMIN_WINDOW_LABEL: &str = "admin";
const SERVER_PORT: u16 = 2000;

/// 打开安装引导窗口，加载 /setup 页面
pub fn open_setup_window(app: &AppHandle) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/setup", SERVER_PORT);

    WebviewWindowBuilder::new(
        app,
        SETUP_WINDOW_LABEL,
        WebviewUrl::External(url.parse().unwrap()),
    )
    .title("Colophon — 首次安装")
    .inner_size(860.0, 680.0)
    .min_inner_size(640.0, 540.0)
    .center()
    .resizable(true)
    .build()
    .map_err(|e| format!("failed to create setup window: {}", e))?;

    tracing::info!("setup window opened");
    Ok(())
}

/// 打开后台管理窗口，加载 /admin 页面
pub fn open_admin_window(app: &AppHandle) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/admin", SERVER_PORT);

    WebviewWindowBuilder::new(
        app,
        ADMIN_WINDOW_LABEL,
        WebviewUrl::External(url.parse().unwrap()),
    )
    .title("Colophon")
    .inner_size(1280.0, 820.0)
    .min_inner_size(800.0, 600.0)
    .center()
    .resizable(true)
    .build()
    .map_err(|e| format!("failed to create admin window: {}", e))?;

    tracing::info!("admin window opened");
    Ok(())
}

/// 关闭安装窗口，打开后台窗口（供 IPC 命令调用）
pub fn switch_to_admin(app: &AppHandle) -> Result<(), String> {
    // 关闭安装窗口
    if let Some(setup_win) = app.get_webview_window(SETUP_WINDOW_LABEL) {
        setup_win
            .close()
            .map_err(|e| format!("failed to close setup window: {}", e))?;
    }

    // 打开后台窗口（若已存在则聚焦）
    if let Some(admin_win) = app.get_webview_window(ADMIN_WINDOW_LABEL) {
        admin_win
            .set_focus()
            .map_err(|e| format!("failed to focus admin window: {}", e))?;
    } else {
        open_admin_window(app)?;
    }

    tracing::info!("switched from setup window to admin window");
    Ok(())
}
