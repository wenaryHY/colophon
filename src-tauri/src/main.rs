// Tauri 桌面应用入口
// 负责启动 Tauri 应用，不包含业务逻辑

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    colophon_desktop_lib::run();
}
