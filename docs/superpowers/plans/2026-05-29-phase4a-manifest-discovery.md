# Phase 4a — 插件 Manifest 声明式发现 Implementation Plan

**状态:** ✅ 已完成

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有 Rust `Plugin` trait 架构上增加 P0 能力：`plugin.toml` 声明式 manifest、build.rs 自动扫描 `plugins/` 目录发现插件、DB 持久化启用/禁用状态、SemVer 版本兼容检查。不引入 wasmtime。

**Architecture:** build.rs 在编译时扫描 `plugins/` 目录，校验 `plugin.toml` 中 `id == 目录名`，生成 `$OUT_DIR/plugin_registry.rs`（通过 `include!` + `env!("CARGO_MANIFEST_DIR")` 内联各插件 `lib.rs`）。启动时 `PluginLoader` 扫描 manifest → semver 检查 → DB 状态过滤 → 返回 `DiscoveredPlugin` 列表。`PluginManager::load_with()` 仅实例化已发现的插件。`src/plugins/` 废弃，HelloWorld 迁移到 `plugins/hello-world-a3f9b2c1/`。

**Tech Stack:** Rust, serde, toml, semver, sha2, base64, sqlx, async-trait, once_cell, tokio

**Pre-requisites:** Phase 1–3 已完成，`cargo test -p inkforge` 全绿，migrations 最大编号 013。

**运行测试命令:** `cargo test -p inkforge`

---

## 文件结构

| 文件 | 职责 | 操作 |
|------|------|------|
| `Cargo.toml` | 添加 `semver`、`base64` 依赖 | 修改 |
| `build.rs` | 编译时扫描 `plugins/` 生成注册代码 | 新建 |
| `src/modules/plugin/manifest.rs` | `PluginManifest` 结构体 + toml 解析 | 新建 |
| `src/modules/plugin/id_strategy.rs` | `PluginIdStrategy` trait + `ShortHashIdStrategy` | 新建 |
| `src/modules/plugin/loader.rs` | `PluginLoader` 扫描、版本检查、状态过滤 | 新建 |
| `src/modules/plugin/status.rs` | `PluginStatusStore` DB 操作 | 新建 |
| `src/modules/plugin/registry.rs` | 保持不变（`register`/`take_all` 对外 API 不动） | 不动 |
| `src/modules/plugin/mod.rs` | 注册新模块 | 修改 |
| `src/modules/plugin/manager.rs` | 新增 `load_with()` 按 discovered 过滤 | 修改 |
| `src/lib.rs` | 移除硬编码注册，改用 `register_all()` + loader | 修改 |
| `migrations/014_plugin_status.sql` | `plugins` 表 | 新建 |
| `plugins/hello-world-a3f9b2c1/plugin.toml` | HelloWorld manifest | 新建 |
| `plugins/hello-world-a3f9b2c1/lib.rs` | HelloWorld 源码（从 `src/plugins/` 移过来） | 新建 |
| `plugins/hello-world-a3f9b2c1/static/hello.css` | 从旧位置 copy | 移动 |
| `src/plugins/hello_world.rs` | 废弃 | 删除 |
| `src/plugins/mod.rs` | 废弃 | 删除 |
| `src/tests/plugin_hello_world_tests.rs` | 重构为 manifest 测试 | 修改 |
| `src/tests/plugin_manifest_tests.rs` | manifest/loader/id_strategy 单元测试 | 新建 |
| `src/tests.rs` | 更新测试模块注册 | 修改 |

---

## Task 1: 添加 semver + base64 依赖

**Files:**
- Modify: `Cargo.toml`

**目的:** 为 manifest 版本检查和插件 ID 哈希计算添加依赖。

- [ ] **Step 1: 在 Cargo.toml 中添加依赖**

在 `sha2 = "=0.10.9"` 之后添加：

```toml
semver = "=1.0.26"
base64 = "=0.22.1"
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过，无错误。

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: 添加 semver 和 base64 依赖（Phase 4a 插件 manifest）"
```

---

## Task 2: 创建 PluginManifest + PluginIdStrategy

**Files:**
- Create: `src/modules/plugin/manifest.rs`
- Create: `src/modules/plugin/id_strategy.rs`
- Modify: `src/modules/plugin/mod.rs`

- [ ] **Step 1: 创建 manifest.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub engine: Option<EngineMeta>,
    pub hooks: Option<HooksMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub id: String,
    pub title: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineMeta {
    pub inkforge: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksMeta {
    pub template: Option<bool>,
    pub routes: Option<bool>,
    pub assets: Option<Vec<String>>,
}

impl PluginManifest {
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = toml::from_str(&content)?;
        Ok(manifest)
    }
}
```

- [ ] **Step 2: 创建 id_strategy.rs**

```rust
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait PluginIdStrategy: Send + Sync {
    fn generate(name: &str) -> String;
    fn validate(id: &str) -> bool;
}

pub struct ShortHashIdStrategy;

impl PluginIdStrategy for ShortHashIdStrategy {
    fn generate(name: &str) -> String {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let hash = Sha256::digest(format!("{}-{}", name, ts));
        use base64::Engine;
        let suffix = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash[..6]);
        format!("{}-{}", name, &suffix[..8])
    }

    fn validate(id: &str) -> bool {
        id.len() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !id.starts_with('-')
            && !id.ends_with('-')
    }
}
```

- [ ] **Step 3: 在 mod.rs 中注册新模块**

在 `src/modules/plugin/mod.rs` 的 `pub mod asset;` 之后添加：

```rust
pub mod manifest;
pub mod id_strategy;
```

- [ ] **Step 4: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。

- [ ] **Step 5: Commit**

```bash
git add src/modules/plugin/manifest.rs src/modules/plugin/id_strategy.rs src/modules/plugin/mod.rs
git commit -m "feat: 创建 PluginManifest 和 PluginIdStrategy 数据结构"
```

---

## Task 3: 创建 PluginStatusStore（DB 迁移 + 操作）

**Files:**
- Create: `migrations/014_plugin_status.sql`
- Create: `src/modules/plugin/status.rs`
- Modify: `src/modules/plugin/mod.rs`

- [ ] **Step 1: 创建数据库迁移文件**

```sql
CREATE TABLE IF NOT EXISTS plugins (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    version TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    installed_at TEXT NOT NULL,
    error_message TEXT
);
```

- [ ] **Step 2: 创建 status.rs**

```rust
use sqlx::SqlitePool;

use crate::shared::error::AppResult;

pub struct PluginStatusStore;

impl PluginStatusStore {
    pub async fn get_enabled_ids(pool: &SqlitePool) -> AppResult<Vec<String>> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT id FROM plugins WHERE enabled = 1"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn ensure_installed(
        pool: &SqlitePool,
        id: &str,
        title: &str,
        version: &str,
    ) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR IGNORE INTO plugins (id, title, version, enabled, installed_at) VALUES (?, ?, ?, 1, ?)"
        )
        .bind(id)
        .bind(title)
        .bind(version)
        .bind(&now)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<()> {
        sqlx::query("UPDATE plugins SET enabled = ? WHERE id = ?")
            .bind(enabled as i32)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
```

- [ ] **Step 3: 在 mod.rs 中注册模块**

在 `pub mod id_strategy;` 之后添加：

```rust
pub mod status;
```

- [ ] **Step 4: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。

- [ ] **Step 5: Commit**

```bash
git add migrations/014_plugin_status.sql src/modules/plugin/status.rs src/modules/plugin/mod.rs
git commit -m "feat: 创建 PluginStatusStore 和 plugins 数据库迁移"
```

---

## Task 4: 创建 PluginLoader（扫描 + 版本检查 + 状态过滤）

**Files:**
- Create: `src/modules/plugin/loader.rs`
- Modify: `src/modules/plugin/mod.rs`

- [ ] **Step 1: 创建 loader.rs**

```rust
use std::path::{Path, PathBuf};

use semver::VersionReq;

use crate::shared::error::AppResult;

use super::manifest::PluginManifest;
use super::status::PluginStatusStore;

pub struct DiscoveredPlugin {
    pub manifest: PluginManifest,
    pub dir_path: PathBuf,
}

pub struct PluginLoader {
    plugin_dir: PathBuf,
    host_version: String,
}

impl PluginLoader {
    pub fn new(plugin_dir: &Path, host_version: &str) -> Self {
        Self {
            plugin_dir: plugin_dir.to_path_buf(),
            host_version: host_version.to_string(),
        }
    }

    pub fn scan_manifests(&self) -> AppResult<Vec<PluginManifest>> {
        let mut manifests = Vec::new();
        let dir = match std::fs::read_dir(&self.plugin_dir) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(module = "plugin", "plugin directory not found, creating");
                std::fs::create_dir_all(&self.plugin_dir)?;
                return Ok(manifests);
            }
            Err(e) => return Err(e.into()),
        };

        for entry in dir {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let manifest_path = entry.path().join("plugin.toml");
            if !manifest_path.exists() {
                tracing::warn!(module = "plugin", dir = %dir_name, "missing plugin.toml");
                continue;
            }
            let manifest = PluginManifest::from_file(&manifest_path)?;
            if manifest.plugin.id != dir_name {
                tracing::error!(
                    module = "plugin",
                    expected = %dir_name,
                    found = %manifest.plugin.id,
                    "plugin id mismatch with directory name"
                );
                continue;
            }
            manifests.push(manifest);
        }
        Ok(manifests)
    }

    pub fn check_version(&self, manifest: &PluginManifest) -> Result<bool, semver::Error> {
        let req_str = manifest
            .engine
            .as_ref()
            .and_then(|e| e.inkforge.as_deref())
            .unwrap_or("*");
        let req = VersionReq::parse(req_str)?;
        let host_ver = semver::Version::parse(&self.host_version)?;
        Ok(req.matches(&host_ver))
    }

    pub async fn discover(&self, pool: &sqlx::SqlitePool) -> AppResult<Vec<DiscoveredPlugin>> {
        let manifests = self.scan_manifests()?;
        let enabled_ids = PluginStatusStore::get_enabled_ids(pool).await?;
        let mut discovered = Vec::new();

        for manifest in manifests {
            match self.check_version(&manifest) {
                Ok(false) => {
                    tracing::warn!(
                        module = "plugin",
                        id = %manifest.plugin.id,
                        version = %manifest.plugin.version,
                        "plugin requires newer host version"
                    );
                    continue;
                }
                Err(e) => {
                    tracing::error!(module = "plugin", id = %manifest.plugin.id, error = %e, "version check error");
                    continue;
                }
                Ok(true) => {}
            }

            if !enabled_ids.contains(&manifest.plugin.id) {
                tracing::info!(module = "plugin", id = %manifest.plugin.id, "plugin disabled, skipping");
                continue;
            }

            PluginStatusStore::ensure_installed(
                pool,
                &manifest.plugin.id,
                &manifest.plugin.title,
                &manifest.plugin.version,
            ).await?;

            let dir_path = self.plugin_dir.join(&manifest.plugin.id);
            discovered.push(DiscoveredPlugin { manifest, dir_path });
        }

        Ok(discovered)
    }
}
```

- [ ] **Step 2: 在 mod.rs 中注册模块**

在 `pub mod status;` 之后添加：

```rust
pub mod loader;
```

- [ ] **Step 3: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。

- [ ] **Step 4: Commit**

```bash
git add src/modules/plugin/loader.rs src/modules/plugin/mod.rs
git commit -m "feat: 创建 PluginLoader（扫描 + SemVer 检查 + DB 状态过滤）"
```

---

## Task 5: 创建 build.rs 自动扫描插件

**Files:**
- Create: `build.rs`（项目根目录，与 Cargo.toml 同级）

**设计说明:** build.rs 在编译时遍历 `plugins/` 子目录，读取 `plugin.toml` 校验 `id == 目录名`，通过 `include!` + `env!("CARGO_MANIFEST_DIR")` 将各插件 `lib.rs` 嵌入为子模块，生成异步 `register_all()` 函数调用 `registry::register()`。

- [ ] **Step 1: 创建 build.rs**

```rust
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("plugin_registry.rs");

    let plugins_dir = Path::new("plugins");
    let mut entries = String::new();

    if plugins_dir.exists() {
        if let Ok(dir) = fs::read_dir(plugins_dir) {
            for entry in dir.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
                let manifest_path = path.join("plugin.toml");
                let lib_path = path.join("lib.rs");

                if !manifest_path.exists() || !lib_path.exists() {
                    continue;
                }

                let content = fs::read_to_string(&manifest_path).unwrap_or_default();
                let id = content
                    .lines()
                    .find(|l| l.trim().starts_with("id"))
                    .and_then(|l| {
                        l.splitn(2, '=')
                            .nth(1)
                            .map(|v| v.trim().trim_matches(|c: char| c == '"' || c == '\''))
                    })
                    .unwrap_or("");

                if id != dir_name {
                    println!(
                        "cargo:warning=plugin id '{}' != directory '{}', skipping",
                        id, dir_name
                    );
                    continue;
                }

                let module_name = format!(
                    "_plugin_{}",
                    dir_name.replace('-', "_").replace('.', "_")
                );

                entries.push_str(&format!(
                    r#"mod {module_name} {{
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/plugins/{dir_name}/lib.rs"));
}}
crate::modules::plugin::registry::register(Box::new({module_name}::HelloWorldPlugin::new())).await;
"#,
                    module_name = module_name,
                    dir_name = dir_name,
                ));

                println!("cargo:warning=registering plugin: {}", dir_name);
            }
        }
    }

    let code = format!(
        r#"pub async fn register_all() {{
    {}
}}"#,
        entries
    );

    fs::write(&dest, code).unwrap();
    println!("cargo:rerun-if-changed=plugins/");
}
```

- [ ] **Step 2: 验证 build.rs 生成**

```bash
cargo check -p inkforge
```

Expected: 编译通过。注意 build stage 输出中应出现 `registering plugin: hello-world-a3f9b2c1`（如果该目录已创建）。若尚未创建，会有 `warning` 但不会阻止编译（代码生成一个空 `register_all()`）。

- [ ] **Step 3: Commit**

```bash
git add build.rs
git commit -m "feat: 创建 build.rs 自动扫描 plugins/ 目录并生成注册代码"
```

---

## Task 6: 重构 lib.rs + PluginManager 集成

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/modules/plugin/manager.rs`

- [ ] **Step 1: 修改 lib.rs — 移除硬编码注册，改用自动发现**

在 `pub mod ws;` 之后、`#[cfg(test)]` 之前添加：

```rust
include!(concat!(env!("OUT_DIR"), "/plugin_registry.rs"));
```

修改 `serve()` 函数，将：

```rust
crate::modules::plugin::registry::register(Box::new(
    plugins::hello_world::HelloWorldPlugin::new(),
))
.await;

let plugin_manager = Arc::new(PluginManager::load().await);
```

替换为：

```rust
register_all().await;

let loader = crate::modules::plugin::loader::PluginLoader::new(
    std::path::Path::new("plugins"),
    env!("CARGO_PKG_VERSION"),
);
let discovered = loader.discover(&pool).await?;
let plugin_manager = Arc::new(PluginManager::load_with(discovered).await);
```

同时删除 `pub mod plugins;` 行。

修改后的 `src/lib.rs` 前 15 行变为：

```rust
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
```

- [ ] **Step 2: 修改 manager.rs — 新增 load_with() 方法**

在 `impl PluginManager` 块中，保留 `load()` 方法不变，新增 `load_with()`：

```rust
use super::loader::DiscoveredPlugin;

impl PluginManager {
    pub async fn load() -> Self {
        let plugins = registry::take_all().await;
        tracing::info!(
            module = "plugin",
            count = plugins.len(),
            "PluginManager loaded {} plugin(s)",
            plugins.len()
        );
        Self { plugins }
    }

    pub async fn load_with(discovered: Vec<DiscoveredPlugin>) -> Self {
        let all_plugins = registry::take_all().await;
        let discovered_ids: std::collections::HashSet<String> = discovered
            .iter()
            .map(|d| d.manifest.plugin.id.clone())
            .collect();
        let plugins: Vec<Box<dyn super::Plugin>> = all_plugins
            .into_iter()
            .filter(|p| discovered_ids.contains(p.name()))
            .collect();
        tracing::info!(
            module = "plugin",
            count = plugins.len(),
            "PluginManager loaded {} discovered plugin(s)",
            plugins.len()
        );
        Self { plugins }
    }
```

在 `manager.rs` 文件顶部 `use super::asset::PluginAsset;` 之后添加 import：

```rust
use std::collections::HashSet;
```

- [ ] **Step 3: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过（`src/plugins/` 仍存在，但 `pub mod plugins;` 已删除，不再引用）。

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/modules/plugin/manager.rs
git commit -m "feat: 集成 PluginLoader 和 load_with 到启动流程"
```

---

## Task 7: 迁移 HelloWorld 到 plugins/ 目录 + 删除 src/plugins/

**Files:**
- Create: `plugins/hello-world-a3f9b2c1/plugin.toml`
- Create: `plugins/hello-world-a3f9b2c1/lib.rs`
- Create: `plugins/hello-world-a3f9b2c1/static/hello.css`（从旧位置复制）
- Delete: `src/plugins/hello_world.rs`
- Delete: `src/plugins/mod.rs`
- Delete: `src/plugins/` 目录

- [ ] **Step 1: 创建目录 + plugin.toml**

```bash
mkdir -p plugins/hello-world-a3f9b2c1/static
```

然后创建 `plugins/hello-world-a3f9b2c1/plugin.toml`：

```toml
[plugin]
id = "hello-world-a3f9b2c1"
title = "Hello World"
version = "0.1.0"
description = "A demo plugin that demonstrates the InkForge plugin system"
author = "InkForge Team"

[engine]
inkforge = ">=0.3.0"

[hooks]
template = true
routes = true
assets = ["css"]
```

- [ ] **Step 2: 创建 lib.rs（基于原 HelloWorld，修改 name() 返回值）**

```rust
use async_trait::async_trait;
use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use minijinja::Environment;
use std::sync::Arc;

use crate::modules::plugin::asset::{AssetPlacement, PluginAsset};
use crate::modules::plugin::Plugin;
use crate::shared::error::AppResult;
use crate::state::AppState;

#[derive(Default)]
pub struct HelloWorldPlugin;

impl HelloWorldPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for HelloWorldPlugin {
    fn name(&self) -> &str {
        "hello-world-a3f9b2c1"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    async fn init(&self, _state: &Arc<AppState>) -> AppResult<()> {
        tracing::info!(
            module = "plugin",
            plugin = "hello-world-a3f9b2c1",
            "HelloWorld plugin initialized"
        );
        Ok(())
    }

    fn api_routes(&self) -> Router<Arc<AppState>> {
        async fn hello_handler(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
            Json(serde_json::json!({
                "plugin": "hello-world-a3f9b2c1",
                "status": "ok"
            }))
        }

        Router::new().route("/api/v1/plugins/hello", get(hello_handler))
    }

    fn extend_template_env(&self, env: &mut Environment<'_>) -> AppResult<()> {
        env.add_function(
            "hello_world",
            |name: Option<String>| -> Result<String, minijinja::Error> {
                let who = name.unwrap_or_else(|| "World".to_string());
                Ok(format!("Hello, {}!", who))
            },
        );
        Ok(())
    }

    fn frontend_assets(&self) -> Vec<PluginAsset> {
        vec![PluginAsset::css(self.name(), "hello.css", AssetPlacement::Head)]
    }
}
```

- [ ] **Step 3: 复制静态资源**

```bash
cp plugins/hello-world/static/hello.css plugins/hello-world-a3f9b2c1/static/hello.css
```

- [ ] **Step 4: 删除 src/plugins/ 目录**

```bash
rm -rf src/plugins/
```

或者删除以下文件：`src/plugins/hello_world.rs`、`src/plugins/mod.rs`，再删除 `src/plugins/` 目录。

- [ ] **Step 5: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。build.rs 输出 `registering plugin: hello-world-a3f9b2c1`。

- [ ] **Step 6: Commit**

```bash
git add plugins/hello-world-a3f9b2c1/
git rm src/plugins/hello_world.rs src/plugins/mod.rs
git rm -r src/plugins/
git commit -m "refactor: 迁移 HelloWorld 插件到 plugins/ 声明式目录结构"
```

---

## Task 8: 更新测试（manifest/loader/id_strategy 测试 + 删除旧测试引用）

**Files:**
- Delete: `src/tests/plugin_hello_world_tests.rs`
- Create: `src/tests/plugin_manifest_tests.rs`
- Modify: `src/tests.rs`

- [ ] **Step 1: 删除旧测试文件**

```bash
rm src/tests/plugin_hello_world_tests.rs
```

- [ ] **Step 2: 创建 manifest + loader + id_strategy 测试**

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::modules::plugin::id_strategy::{PluginIdStrategy, ShortHashIdStrategy};
    use crate::modules::plugin::manifest::PluginManifest;
    use crate::modules::plugin::loader::PluginLoader;

    #[test]
    fn parse_valid_plugin_toml() {
        let toml_str = r#"
[plugin]
id = "test-plugin"
title = "Test Plugin"
version = "1.0.0"
description = "A test plugin"

[engine]
inkforge = ">=0.3.0"

[hooks]
template = true
routes = true
assets = ["css", "js"]
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.id, "test-plugin");
        assert_eq!(manifest.plugin.title, "Test Plugin");
        assert_eq!(manifest.plugin.version, "1.0.0");
        assert_eq!(
            manifest.engine.as_ref().and_then(|e| e.inkforge.as_deref()),
            Some(">=0.3.0")
        );
        let hooks = manifest.hooks.unwrap();
        assert_eq!(hooks.template, Some(true));
        assert_eq!(hooks.routes, Some(true));
        assert_eq!(hooks.assets, Some(vec!["css".to_string(), "js".to_string()]));
    }

    #[test]
    fn parse_minimal_plugin_toml() {
        let toml_str = r#"
[plugin]
id = "minimal"
title = "Minimal"
version = "0.1.0"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.id, "minimal");
        assert!(manifest.engine.is_none());
        assert!(manifest.hooks.is_none());
    }

    #[test]
    fn manifest_from_file_parses_correctly() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugins")
            .join("hello-world-a3f9b2c1")
            .join("plugin.toml");
        if path.exists() {
            let manifest = PluginManifest::from_file(&path).unwrap();
            assert_eq!(manifest.plugin.id, "hello-world-a3f9b2c1");
            assert_eq!(manifest.plugin.title, "Hello World");
            assert!(manifest.engine.unwrap().inkforge.is_some());
        }
    }

    #[test]
    fn version_check_passes_with_compatible_version() {
        let manifest = make_manifest(">=0.3.0");
        let loader = PluginLoader::new(Path::new("plugins"), "0.3.0");
        assert!(loader.check_version(&manifest).unwrap());
    }

    #[test]
    fn version_check_passes_with_exact_match() {
        let manifest = make_manifest(">=0.2.0");
        let loader = PluginLoader::new(Path::new("plugins"), "0.3.0");
        assert!(loader.check_version(&manifest).unwrap());
    }

    #[test]
    fn version_check_fails_with_incompatible_version() {
        let manifest = make_manifest(">=1.0.0");
        let loader = PluginLoader::new(Path::new("plugins"), "0.3.0");
        assert!(!loader.check_version(&manifest).unwrap());
    }

    #[test]
    fn version_check_passes_with_no_engine_constraint() {
        let toml_str = r#"
[plugin]
id = "test"
title = "Test"
version = "1.0.0"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let loader = PluginLoader::new(Path::new("plugins"), "0.3.0");
        assert!(loader.check_version(&manifest).unwrap());
    }

    fn make_manifest(inkforge_req: &str) -> PluginManifest {
        let toml_str = format!(
            r#"
[plugin]
id = "test"
title = "Test"
version = "1.0.0"

[engine]
inkforge = "{}"
"#,
            inkforge_req
        );
        toml::from_str(&toml_str).unwrap()
    }

    #[test]
    fn generated_id_is_valid() {
        let id = ShortHashIdStrategy::generate("hello-world");
        assert!(ShortHashIdStrategy::validate(&id));
    }

    #[test]
    fn same_name_different_time_produces_different_ids() {
        let id1 = ShortHashIdStrategy::generate("hello-world");
        let id2 = ShortHashIdStrategy::generate("hello-world");
        assert_ne!(id1, id2, "two calls should produce different IDs due to nanosecond timestamps");
    }

    #[test]
    fn invalid_chars_rejected() {
        assert!(!ShortHashIdStrategy::validate("hello world"));
        assert!(!ShortHashIdStrategy::validate("hello/world"));
        assert!(!ShortHashIdStrategy::validate("hello@world"));
    }

    #[test]
    fn empty_id_rejected() {
        assert!(!ShortHashIdStrategy::validate(""));
    }

    #[test]
    fn leading_trailing_separator_rejected() {
        assert!(!ShortHashIdStrategy::validate("-hello"));
        assert!(!ShortHashIdStrategy::validate("hello-"));
    }

    #[test]
    fn long_id_rejected() {
        let long = "a".repeat(65);
        assert!(!ShortHashIdStrategy::validate(&long));
    }

    #[test]
    fn valid_ids_accepted() {
        assert!(ShortHashIdStrategy::validate("hello-world"));
        assert!(ShortHashIdStrategy::validate("hello-world-a3f9b2c1"));
        assert!(ShortHashIdStrategy::validate("a"));
        assert!(ShortHashIdStrategy::validate("abc123-xyz"));
    }
}
```

- [ ] **Step 3: 更新 src/tests.rs**

移除第 162 行的 `mod plugin_hello_world_tests;`，添加新模块：

```rust
mod plugin_manifest_tests;
```

修改后的 `src/tests.rs` 末尾：

```rust
mod plugin_manager_tests;
mod plugin_manifest_tests;
```

- [ ] **Step 4: 运行测试**

```bash
cargo test -p inkforge plugin_manifest_tests -- --nocapture
```

Expected: 所有新测试通过。

- [ ] **Step 5: 运行全量测试确认无回归**

```bash
cargo test -p inkforge
```

Expected: 除 `plugin_hello_world_tests` 已移除外，所有原有测试继续通过。`plugin_manager_tests` 不受影响（它们使用 `registry::register` + `PluginManager::load()`）。

- [ ] **Step 6: Commit**

```bash
git rm src/tests/plugin_hello_world_tests.rs
git add src/tests/plugin_manifest_tests.rs src/tests.rs
git commit -m "test: 替换 plugin_hello_world_tests 为 manifest/loader/id_strategy 测试"
```

---

## Task 9: 全量验证 + 文档更新

**Files:**
- 运行 `cargo test -p inkforge`
- 运行 `cargo clippy -p inkforge`
- 更新 `memories/PROJECT_STATUS.md` 记录 Phase 4a 完成

- [ ] **Step 1: 运行完整测试套件**

```bash
cargo test -p inkforge
```

Expected: 所有测试通过（`plugin_manager_tests` 使用 `#[serial]`，确认仍通过）。

- [ ] **Step 2: 运行 clippy**

```bash
cargo clippy -p inkforge -- -D warnings
```

Expected: 无 warning。

- [ ] **Step 3: 清理旧的 plugins/hello-world/ 目录（如无其他用途）**

```bash
rm -rf plugins/hello-world/
```

> 注意：如果 `plugins/hello-world/` 有其他内容被依赖，此步骤可跳过或改为 Task 7 一起处理。

- [ ] **Step 4: 更新 PROJECT_STATUS.md**

在 `memories/PROJECT_STATUS.md` 的近期优先级或插件章节中标记 Phase 4a 完成，添加简要说明：manifest 声明式发现、build.rs 自动扫描、DB 持久化启用/禁用、SemVer 版本兼容检查已就绪。

- [ ] **Step 5: Commit**

```bash
git add memories/PROJECT_STATUS.md
git commit -m "docs: 标记 Phase 4a 插件 manifest 发现完成"
```

---

## 验证清单

| 检查项 | 命令 | 期望 |
|--------|------|------|
| 编译通过 | `cargo check -p inkforge` | 0 errors，build.rs 输出 registering plugin |
| 测试全绿 | `cargo test -p inkforge` | All passed（plugin_manager_tests 仍通过） |
| Clippy 无警告 | `cargo clippy -p inkforge -- -D warnings` | 0 warnings |
| `src/plugins/` 已删除 | `Test-Path src/plugins/` | False |
| `plugins/hello-world-a3f9b2c1/` 存在 | `Test-Path plugins/hello-world-a3f9b2c1/plugin.toml` | True |
| build.rs 生成注册代码 | 检查 build output | 出现 `registering plugin: hello-world-a3f9b2c1` |
| migrations 有 014 | `Test-Path migrations/014_plugin_status.sql` | True |
| 依赖完整 | `rg 'semver\|base64' Cargo.toml` | 两个依赖在 [dependencies] 中 |
