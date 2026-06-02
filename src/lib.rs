pub mod admin;
pub mod bootstrap;
pub mod infra;
pub mod modules;
pub mod shared;
pub mod state;
pub mod ws;

include!(concat!(env!("OUT_DIR"), "/plugin_registry.rs"));

#[cfg(test)]
pub mod tests;

use std::{net::SocketAddr, sync::Arc};

use bootstrap::{config::AppConfig, router::build_router};
use sqlx::sqlite::SqlitePoolOptions;
use state::AppState;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use std::path::PathBuf;
use crate::modules::plugin::loader::PluginLoader;
use crate::modules::plugin::manager::PluginManager;

pub async fn serve() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "inkforge=info,axum=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::load()?;
    config.validate()?;
    std::fs::create_dir_all(&config.storage.upload_dir)?;
    std::fs::create_dir_all(&config.theme.theme_dir)?;

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous=NORMAL")
        .execute(&pool)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    // 创建 WebSocket broadcast channel，容量 256
    let (event_tx, _rx) = broadcast::channel::<ws::ServerEvent>(256);

    let setup_runtime = modules::setup::service::bootstrap_runtime(&pool).await?;

    register_all().await;

    let loader = PluginLoader::new(
        PathBuf::from("plugins"),
        env!("CARGO_PKG_VERSION"),
    );
    let discovered = loader.discover(&pool).await?;
    let plugin_manager = Arc::new(tokio::sync::RwLock::new(PluginManager::load_with(discovered).await));

    let state = Arc::new(AppState::new(
        config.clone(),
        pool,
        event_tx,
        setup_runtime.site_url,
        setup_runtime.admin_url,
        setup_runtime.stage,
        plugin_manager.clone(),
    )?);

    state.plugin_manager.write().await.init_all(&state).await?;
    // 初始化 Webhook 分发器，注册到全局 HookRegistry
    {
        let dispatcher = modules::webhook::service::WebhookDispatcher::new(state.pool.clone());
        let hooks = dispatcher.into_hooks();
        state.plugin_manager.read().await.hook_registry().register("webhook", hooks).await;
    }
    modules::backup::scheduler::start_backup_scheduler(state.clone()).await?;
    modules::trash::scheduler::start_trash_scheduler(state.clone()).await?;
    let app = build_router(state).await;

    let addr = SocketAddr::new(config.server.host.parse()?, config.server.port);
    tracing::info!("InkForge listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
