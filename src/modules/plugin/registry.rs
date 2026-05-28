use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use super::Plugin;

static PLUGIN_REGISTRY: Lazy<Mutex<Vec<Box<dyn Plugin>>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

pub async fn register(plugin: Box<dyn Plugin>) {
    PLUGIN_REGISTRY.lock().await.push(plugin);
}

pub async fn take_all() -> Vec<Box<dyn Plugin>> {
    std::mem::take(&mut *PLUGIN_REGISTRY.lock().await)
}
