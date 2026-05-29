# P1a — Hooks 系统 Implementation Plan

**状态:** 🔲 待实施

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有 Plugin 架构上实现 Directus 式 Hooks 系统：5 个钩子（`post.before_save`、`post.after_save`、`post.after_publish`、`post.before_render`、`comment.before_create`）、Filter/Action 二分模型、优先排序行管道、并行 Action + 5s 超时、Plugin trait 扩展、HookRegistry 注册/分发/卸载。

**Architecture:**

```
Plugin::hooks() → Vec<Hook>
    ↓ (PluginManager::init_all 时注册)
HookRegistry::register(plugin_name, hooks)
    ↓ (运行时分发)
post::service::create_post:
    hook_registry.dispatch_filter("post.before_save", ctx).await?
    → 写 DB
    hook_registry.dispatch_action("post.after_save", ctx).await
    (if status changed to published)
    hook_registry.dispatch_action("post.after_publish", ctx).await

theme::handler::render_post:
    hook_registry.dispatch_filter_best_effort("post.before_render", ctx).await
    → MiniJinja 渲染

comment::service::create_comment:
    hook_registry.dispatch_filter("comment.before_create", ctx).await?
    → 写 DB
```

**Tech Stack:** Rust, async_trait, tokio, Arc, RwLock, minijinja

**Pre-requisites:** Phase 1–4a 已完成，`cargo test -p inkforge` 全绿。

**运行测试命令:** `cargo test -p inkforge plugin_hook`

---

## 文件结构

| 文件 | 职责 | 操作 |
|------|------|------|
| `src/modules/plugin/hook.rs` | Hook/HookContext/HookHandler/HookData 类型定义 | 新建 |
| `src/modules/plugin/hook_registry.rs` | HookRegistry 注册/分发/卸载 | 新建 |
| `src/modules/plugin/mod.rs` | Plugin trait 添加 hooks() 方法 + 注册新模块 | 修改 |
| `src/modules/plugin/manager.rs` | PluginManager 集成 HookRegistry | 修改 |
| `src/modules/post/service.rs` | 插入 before_save/after_save/after_publish 调用点 | 修改 |
| `src/modules/comment/service.rs` | 插入 before_create 调用点 | 修改 |
| `src/modules/theme/handler.rs` | 插入 before_render 调用点 | 修改 |
| `src/tests/plugin_hook_tests.rs` | HookRegistry + 分发单元测试 | 新建 |
| `src/tests.rs` | 注册测试模块 | 修改 |
| `plugins/hello-world-a3f9b2c1/lib.rs` | HelloWorld 注册 after_publish 示例钩子 | 修改 |

---

## Task 1: 定义 Hook 类型体系

**Files:**
- Create: `src/modules/plugin/hook.rs`

**目的:** 定义所有 Hook 相关数据结构 — Hook 注册描述、HookHandler trait、HookContext 和各钩子的 HookData。

- [ ] **Step 1: 创建 hook.rs**

```rust
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::shared::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    Filter,
    Action,
}

pub struct Hook {
    pub name: String,
    pub priority: i32,
    pub plugin_name: String,
    pub hook_type: HookType,
    pub handler: Arc<dyn HookHandler>,
}

#[async_trait]
pub trait HookHandler: Send + Sync {
    async fn run(&self, ctx: &mut HookContext) -> AppResult<()>;
}

impl Hook {
    pub fn new_filter(
        name: &str,
        priority: i32,
        plugin_name: &str,
        handler: Arc<dyn HookHandler>,
    ) -> Self {
        Self {
            name: name.to_string(),
            priority,
            plugin_name: plugin_name.to_string(),
            hook_type: HookType::Filter,
            handler,
        }
    }

    pub fn new_action(
        name: &str,
        priority: i32,
        plugin_name: &str,
        handler: Arc<dyn HookHandler>,
    ) -> Self {
        Self {
            name: name.to_string(),
            priority,
            plugin_name: plugin_name.to_string(),
            hook_type: HookType::Action,
            handler,
        }
    }
}

impl Clone for Hook {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            priority: self.priority,
            plugin_name: self.plugin_name.clone(),
            hook_type: self.hook_type,
            handler: self.handler.clone(),
        }
    }
}

pub struct HookContext {
    pub hook_name: String,
    pub data: HookData,
}

impl Clone for HookContext {
    fn clone(&self) -> Self {
        Self {
            hook_name: self.hook_name.clone(),
            data: self.data.clone(),
        }
    }
}

#[derive(Clone)]
pub enum HookData {
    PostBeforeSave(PostBeforeSaveData),
    PostAfterSave(PostAfterSaveData),
    PostAfterPublish(PostAfterPublishData),
    PostBeforeRender(PostBeforeRenderData),
    CommentBeforeCreate(CommentBeforeCreateData),
}

#[derive(Clone)]
pub struct PostBeforeSaveData {
    pub title: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    pub slug: String,
    pub tags: Vec<String>,
    pub category_id: Option<String>,
    pub content_type: String,
    pub request_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone)]
pub struct PostAfterSaveData {
    pub post_id: String,
    pub title: String,
    pub slug: String,
    pub is_new: bool,
    pub status: String,
}

#[derive(Clone)]
pub struct PostAfterPublishData {
    pub post_id: String,
    pub title: String,
    pub slug: String,
    pub old_status: String,
    pub new_status: String,
}

#[derive(Clone)]
pub struct PostBeforeRenderData {
    pub post_id: String,
    pub title: String,
    pub slug: String,
    pub content_html: String,
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone)]
pub struct CommentBeforeCreateData {
    pub content: String,
    pub author_name: String,
    pub author_email: Option<String>,
    pub post_id: String,
    pub post_title: String,
    pub request_ip: Option<String>,
}
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过（hook.rs 已创建但尚未被 mod.rs 引用，不影响）。

- [ ] **Step 3: Commit**

```bash
git add src/modules/plugin/hook.rs
git commit -m "feat: 定义 Hook/HookContext/HookHandler/HookData 类型体系"
```

---

## Task 2: 创建 HookRegistry

**Files:**
- Create: `src/modules/plugin/hook_registry.rs`

**目的:** 实现插件钩子的注册、按优先级+插件名字典序排序分发、卸载。Filter 串行管道失败传播错误，Action 并行 + 5s 超时吞错误记日志。

- [ ] **Step 1: 创建 hook_registry.rs**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

use super::hook::{Hook, HookContext, HookType};
use crate::shared::error::AppResult;

pub struct HookRegistry {
    hooks: Arc<RwLock<HashMap<String, Vec<Hook>>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, plugin_name: &str, hooks: Vec<Hook>) {
        let mut guard = self.hooks.write().await;
        let mut total = 0usize;
        for hook in hooks {
            let entry = guard.entry(hook.name.clone()).or_default();
            entry.push(hook);
            total += 1;
        }
        for hooks in guard.values_mut() {
            hooks.sort_by_key(|h| (h.priority, h.plugin_name.clone()));
        }
        tracing::info!(
            module = "hook",
            plugin = plugin_name,
            count = total,
            "registered {} hook(s)",
            total
        );
    }

    pub async fn unregister_all(&self, plugin_name: &str) {
        let mut guard = self.hooks.write().await;
        for hooks in guard.values_mut() {
            hooks.retain(|h| h.plugin_name != plugin_name);
        }
        guard.retain(|_, v| !v.is_empty());
        tracing::info!(module = "hook", plugin = plugin_name, "unregistered all hooks");
    }

    pub async fn dispatch_filter(&self, name: &str, ctx: &mut HookContext) -> AppResult<()> {
        let hooks = {
            let guard = self.hooks.read().await;
            guard.get(name).cloned().unwrap_or_default()
        };

        for hook in &hooks {
            if !matches!(hook.hook_type, HookType::Filter) {
                continue;
            }
            hook.handler.run(ctx).await.map_err(|e| {
                tracing::error!(
                    module = "hook",
                    hook = name,
                    plugin = hook.plugin_name,
                    error = %e,
                    "filter hook failed"
                );
                e
            })?;
        }
        Ok(())
    }

    pub async fn dispatch_filter_best_effort(&self, name: &str, ctx: &mut HookContext) {
        let hooks = {
            let guard = self.hooks.read().await;
            guard.get(name).cloned().unwrap_or_default()
        };

        for hook in &hooks {
            if !matches!(hook.hook_type, HookType::Filter) {
                continue;
            }
            if let Err(e) = hook.handler.run(ctx).await {
                tracing::warn!(
                    module = "hook",
                    hook = name,
                    plugin = hook.plugin_name,
                    error = %e,
                    "filter hook failed, skipping plugin"
                );
            }
        }
    }

    pub async fn dispatch_action(&self, name: &str, ctx: &HookContext) {
        let hooks = {
            let guard = self.hooks.read().await;
            guard.get(name).cloned().unwrap_or_default()
        };

        let ctx = Arc::new(ctx.clone());
        for hook in &hooks {
            if !matches!(hook.hook_type, HookType::Action) {
                continue;
            }
            let ctx = ctx.clone();
            let handler = hook.handler.clone();
            let plugin_name = hook.plugin_name.clone();
            let hook_name = name.to_string();
            tokio::spawn(async move {
                let mut action_ctx = HookContext {
                    hook_name: ctx.hook_name.clone(),
                    data: ctx.data.clone(),
                };
                match timeout(Duration::from_secs(5), handler.run(&mut action_ctx)).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        tracing::error!(
                            module = "hook",
                            hook = hook_name,
                            plugin = plugin_name,
                            error = %e,
                            "action hook failed"
                        );
                    }
                    Err(_) => {
                        tracing::warn!(
                            module = "hook",
                            hook = hook_name,
                            plugin = plugin_name,
                            "action hook timed out after 5s"
                        );
                    }
                }
            });
        }
    }
}
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add src/modules/plugin/hook_registry.rs
git commit -m "feat: 创建 HookRegistry（注册/分发/卸载 + Filter串行管道 + Action并行超时）"
```

---

## Task 3: Plugin trait 扩展 + 模块注册

**Files:**
- Modify: `src/modules/plugin/mod.rs`

**目的:** 在 `Plugin` trait 中添加默认的 `hooks()` 方法，并注册 `hook` 和 `hook_registry` 两个子模块。

- [ ] **Step 1: 修改 mod.rs**

在 `pub mod loader;` 之前添加两个新模块声明，并在 `Plugin` trait 的方法列表中 `shutdown()` 之后添加 `hooks()` 默认方法。

修改后的 `src/modules/plugin/mod.rs`：

```rust
use async_trait::async_trait;
use axum::Router;
use minijinja::Environment;
use std::sync::Arc;

use crate::shared::error::AppResult;
use crate::state::AppState;

pub mod asset;
pub mod registry;
pub mod manager;
pub mod manifest;
pub mod id_strategy;
pub mod status;
pub mod hook;
pub mod hook_registry;
pub mod loader;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    async fn init(&self, _state: &Arc<AppState>) -> AppResult<()> {
        Ok(())
    }

    async fn shutdown(&self) -> AppResult<()> {
        Ok(())
    }

    fn hooks(&self) -> Vec<crate::modules::plugin::hook::Hook> {
        vec![]
    }

    fn api_routes(&self) -> Router<Arc<AppState>> {
        Router::new()
    }

    fn extend_template_env(&self, _env: &mut Environment<'_>) -> AppResult<()> {
        Ok(())
    }

    fn frontend_assets(&self) -> Vec<crate::modules::plugin::asset::PluginAsset> {
        vec![]
    }
}
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过（`mod.rs` 自身未引用 `hook::Hook` 的具体方法，仅声明模块和 trait 默认方法）。

- [ ] **Step 3: Commit**

```bash
git add src/modules/plugin/mod.rs
git commit -m "feat: Plugin trait 添加 hooks() 默认方法 + 注册 hook/hook_registry 模块"
```

---

## Task 4: PluginManager 集成 HookRegistry

**Files:**
- Modify: `src/modules/plugin/manager.rs`

**目的:** `PluginManager` 持有 `HookRegistry`，在 `init_all()` 中自动收集所有已发现插件的 `hooks()` 并注册，暴露 `hook_registry()` getter 供 post/comment/theme 模块调用。

- [ ] **Step 1: 修改 manager.rs**

完整替换 `src/modules/plugin/manager.rs`：

```rust
use std::collections::HashSet;
use std::sync::Arc;

use axum::Router;
use minijinja::Environment;

use crate::shared::error::AppResult;
use crate::state::AppState;

use super::asset::PluginAsset;
use super::hook_registry::HookRegistry;
use super::loader::DiscoveredPlugin;
use super::registry;
use super::Plugin;

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    hook_registry: Arc<HookRegistry>,
}

impl PluginManager {
    pub async fn load() -> Self {
        let plugins = registry::take_all().await;
        tracing::info!(
            module = "plugin",
            count = plugins.len(),
            "PluginManager loaded {} plugin(s)",
            plugins.len()
        );
        Self {
            plugins,
            hook_registry: Arc::new(HookRegistry::new()),
        }
    }

    pub async fn load_with(discovered: Vec<DiscoveredPlugin>) -> Self {
        let all_plugins = registry::take_all().await;
        let discovered_ids: HashSet<String> = discovered
            .iter()
            .map(|d| d.manifest.plugin.id.clone())
            .collect();
        let plugins: Vec<Box<dyn Plugin>> = all_plugins
            .into_iter()
            .filter(|p| discovered_ids.contains(p.name()))
            .collect();
        tracing::info!(
            module = "plugin",
            discovered = discovered.len(),
            loaded = plugins.len(),
            "PluginManager loaded {}/{} discovered plugin(s)",
            plugins.len(),
            discovered.len()
        );
        Self {
            plugins,
            hook_registry: Arc::new(HookRegistry::new()),
        }
    }

    pub async fn init_all(&self, state: &Arc<AppState>) -> AppResult<()> {
        for plugin in &self.plugins {
            let hooks = plugin.hooks();
            if !hooks.is_empty() {
                self.hook_registry.register(plugin.name(), hooks).await;
            }
            tracing::info!(
                module = "plugin",
                plugin = plugin.name(),
                version = plugin.version(),
                "initializing plugin"
            );
            plugin.init(state).await?;
        }
        Ok(())
    }

    pub async fn shutdown_all(&self) -> AppResult<()> {
        for plugin in &self.plugins {
            tracing::info!(
                module = "plugin",
                plugin = plugin.name(),
                "shutting down plugin"
            );
            self.hook_registry.unregister_all(plugin.name()).await;
            plugin.shutdown().await?;
        }
        Ok(())
    }

    pub fn hook_registry(&self) -> &Arc<HookRegistry> {
        &self.hook_registry
    }

    pub fn collect_routes(&self) -> Router<Arc<AppState>> {
        let mut router = Router::new();
        for plugin in &self.plugins {
            let plugin_routes = plugin.api_routes();
            router = router.merge(plugin_routes);
        }
        router
    }

    pub fn extend_template_env(&self, env: &mut Environment<'_>) -> AppResult<()> {
        for plugin in &self.plugins {
            plugin.extend_template_env(env)?;
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn collect_assets(&self) -> Vec<PluginAsset> {
        let mut assets = Vec::new();
        for plugin in &self.plugins {
            assets.extend(plugin.frontend_assets());
        }
        assets
    }

    pub fn render_asset_html(&self, placement: &str) -> String {
        self.collect_assets()
            .iter()
            .filter(|a| match placement {
                "head" => matches!(a.placement, super::asset::AssetPlacement::Head),
                "body" => matches!(a.placement, super::asset::AssetPlacement::Body),
                _ => false,
            })
            .map(|a| a.render_html())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。现有代码的 `PluginManager` 构造通过 `load()` / `load_with()` 完成，新字段已提供默认值。

- [ ] **Step 3: Commit**

```bash
git add src/modules/plugin/manager.rs
git commit -m "feat: PluginManager 集成 HookRegistry（init_all 注册钩子 + shutdown_all 卸载）"
```

---

## Task 5: 在 post service 中插入钩子调用点

**Files:**
- Modify: `src/modules/post/service.rs`

**目的:** 在 `create_post` 和 `update_post` 中插入 `post.before_save`（写 DB 前）、`post.after_save`（写 DB 后）、`post.after_publish`（状态变更为 published 时）三个调用点。

- [ ] **Step 1: 修改 add import**

在 `use crate::{` 块之后的 `use super::{` 块之前添加 import：

```rust
use crate::modules::plugin::hook::{HookContext, HookData, PostBeforeSaveData, PostAfterSaveData, PostAfterPublishData};
```

- [ ] **Step 2: 修改 create_post**

在 `let content_html = markdown_to_html(&content_md);` 之后、`let id = repository::insert_post(` 之前插入 before_save 钩子调用。在函数末尾 `get_admin_post(state, &id).await` 之后插入 after_save 和 after_publish 调用。

修改后的 `create_post` 函数（仅展示插入部分，其余代码不变）：

```rust
pub async fn create_post(
    state: Arc<AppState>,
    auth: &AuthUser,
    body: CreatePostRequest,
) -> AppResult<AdminPostResponse> {
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }

    let content_type = normalize_content_type(body.content_type.as_deref())?;
    let is_page = content_type == "page";
    let page_render_mode = normalize_page_render_mode(body.page_render_mode.as_deref(), is_page);

    let content_md = body
        .content_md
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    let slug = body
        .slug
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| slugify(&body.title));
    if repository::slug_exists(&state.pool, &slug, None).await? {
        return Err(AppError::Conflict("post slug already exists".into()));
    }

    let status = normalize_status(body.status.as_deref())?;
    let visibility = normalize_visibility(body.visibility.as_deref())?;
    let content_html = markdown_to_html(&content_md);

    let hook_registry = state.plugin_manager.hook_registry();
    let tag_names: Vec<String> = body.tags.clone().unwrap_or_default();
    let mut ctx = HookContext {
        hook_name: "post.before_save".into(),
        data: HookData::PostBeforeSave(PostBeforeSaveData {
            title: body.title.trim().to_string(),
            content_html: content_html.clone(),
            excerpt: body.excerpt.clone(),
            slug: slug.clone(),
            tags: tag_names,
            category_id: body.category_id.clone(),
            content_type: content_type.clone(),
            request_ip: None,
            user_agent: None,
        }),
    };
    hook_registry.dispatch_filter("post.before_save", &mut ctx).await?;

    let filtered = match &ctx.data {
        HookData::PostBeforeSave(d) => d.clone(),
        _ => unreachable!(),
    };

    let final_title = if filtered.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    } else {
        filtered.title
    };
    let final_slug = if filtered.slug.trim().is_empty() {
        return Err(AppError::BadRequest("slug is required".into()));
    } else {
        filtered.slug
    };

    let id = repository::insert_post(
        &state.pool,
        &auth.id,
        &final_title,
        &final_slug,
        filtered.excerpt.as_deref(),
        &content_md,
        &filtered.content_html,
        body.cover_media_id.as_deref(),
        &status,
        &visibility,
        filtered.category_id.as_deref(),
        body.allow_comment.unwrap_or(content_type == "post"),
        body.pinned.unwrap_or(false),
        &content_type,
        body.custom_html_path.as_deref(),
        &page_render_mode,
    )
    .await?;

    if content_type == "post" {
        if let Some(tag_ids) = body.tag_ids {
            repository::replace_tags(&state.pool, &id, &tag_ids).await?;
        }
    }

    let after_ctx = HookContext {
        hook_name: "post.after_save".into(),
        data: HookData::PostAfterSave(PostAfterSaveData {
            post_id: id.clone(),
            title: final_title.clone(),
            slug: final_slug.clone(),
            is_new: true,
            status: status.clone(),
        }),
    };
    hook_registry.dispatch_action("post.after_save", &after_ctx).await;

    if status == "published" {
        let publish_ctx = HookContext {
            hook_name: "post.after_publish".into(),
            data: HookData::PostAfterPublish(PostAfterPublishData {
                post_id: id.clone(),
                title: final_title.clone(),
                slug: final_slug.clone(),
                old_status: String::new(),
                new_status: "published".to_string(),
            }),
        };
        hook_registry.dispatch_action("post.after_publish", &publish_ctx).await;
    }

    get_admin_post(state, &id).await
}
```

- [ ] **Step 3: 修改 update_post**

同理，在 `update_post` 中：
- 写 DB 前调用 `post.before_save`
- 写 DB 后调用 `post.after_save`
- 如果旧状态不是 published 且新状态是 published，调用 `post.after_publish`

修改后的 `update_post` 函数（仅展示需要插入的钩子部分，在 `repository::update_post()` 调用前后）：

在 `let content_html = markdown_to_html(&content_md);` 之后、`if repository::slug_exists(...)` 之前插入 before_save：

```rust
    let content_html = markdown_to_html(&content_md);

    let hook_registry = state.plugin_manager.hook_registry();
    let mut ctx = HookContext {
        hook_name: "post.before_save".into(),
        data: HookData::PostBeforeSave(PostBeforeSaveData {
            title: title.clone(),
            content_html: content_html.clone(),
            excerpt: excerpt.clone(),
            slug: slug.clone(),
            tags: vec![],
            category_id: category_id.clone(),
            content_type: content_type.clone(),
            request_ip: None,
            user_agent: None,
        }),
    };
    hook_registry.dispatch_filter("post.before_save", &mut ctx).await?;

    let filtered = match &ctx.data {
        HookData::PostBeforeSave(d) => d.clone(),
        _ => unreachable!(),
    };

    let title = filtered.title;
    let slug = if repository::slug_exists(&state.pool, &filtered.slug, Some(id)).await? {
        return Err(AppError::Conflict("post slug already exists".into()));
    } else {
        filtered.slug
    };
    let content_html = filtered.content_html;
    let excerpt = filtered.excerpt;
    let category_id = filtered.category_id;
```

然后在 `repository::update_post(...)` 调用之后，`get_admin_post(state, id).await` 之前插入 after_save 和 after_publish：

```rust
    let was_published = current.status == "published";
    let now_published = status == "published";

    let after_ctx = HookContext {
        hook_name: "post.after_save".into(),
        data: HookData::PostAfterSave(PostAfterSaveData {
            post_id: id.to_string(),
            title: title.clone(),
            slug: slug.clone(),
            is_new: false,
            status: status.clone(),
        }),
    };
    hook_registry.dispatch_action("post.after_save", &after_ctx).await;

    if !was_published && now_published {
        let publish_ctx = HookContext {
            hook_name: "post.after_publish".into(),
            data: HookData::PostAfterPublish(PostAfterPublishData {
                post_id: id.to_string(),
                title: title.clone(),
                slug: slug.clone(),
                old_status: current.status.clone(),
                new_status: "published".to_string(),
            }),
        };
        hook_registry.dispatch_action("post.after_publish", &publish_ctx).await;
    }

    get_admin_post(state, id).await
```

- [ ] **Step 4: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过（post service 已引用 plugin hook 模块）。

- [ ] **Step 5: Commit**

```bash
git add src/modules/post/service.rs
git commit -m "feat: post service 集成 before_save/after_save/after_publish 钩子调用点"
```

---

## Task 6: 在 comment service 中插入钩子调用点

**Files:**
- Modify: `src/modules/comment/service.rs`

**目的:** 在 `create_comment` 中，写 DB 前调用 `comment.before_create` filter 钩子。

- [ ] **Step 1: 修改 create_comment**

在 `use crate::{` 块的 import 区域添加：

```rust
use crate::modules::plugin::hook::{CommentBeforeCreateData, HookContext, HookData};
```

在 `create_comment` 函数中，查找 post 通过后、调用 `repository::insert_comment` 之前，插入钩子调用：

```rust
    let post = post_repository::find_comment_target(&state.pool, slug)
        .await?
        .ok_or(AppError::NotFound)?;
    if post.status != "published" || post.visibility != "public" || post.allow_comment == 0 {
        tracing::warn!(
            module = "comment",
            event = "create_rejected_post_state",
            user_id = %auth.id,
            slug = %slug,
            post_status = %post.status,
            post_visibility = %post.visibility,
            post_allow_comment = post.allow_comment,
            "comment creation rejected"
        );
        return Err(AppError::Forbidden);
    }

    let hook_registry = state.plugin_manager.hook_registry();
    let mut hook_ctx = HookContext {
        hook_name: "comment.before_create".into(),
        data: HookData::CommentBeforeCreate(CommentBeforeCreateData {
            content: body.content.trim().to_string(),
            author_name: auth.username.clone(),
            author_email: None,
            post_id: post.id.clone(),
            post_title: post.title.clone(),
            request_ip: None,
        }),
    };
    hook_registry.dispatch_filter("comment.before_create", &mut hook_ctx).await?;

    let filtered = match &hook_ctx.data {
        HookData::CommentBeforeCreate(d) => d.clone(),
        _ => unreachable!(),
    };

    let final_content = filtered.content;
    if final_content.trim().is_empty() {
        return Err(AppError::BadRequest("comment content is required".into()));
    }

    let moderation_mode =
        setting_repository::get_string(&state.pool, "comment_moderation_mode", "all").await?;
    let has_approved = repository::count_approved_by_user(&state.pool, &auth.id).await? > 0;
    let status = moderation_status(&moderation_mode, has_approved);

    tracing::info!(
        module = "comment",
        event = "create_attempt",
        user_id = %auth.id,
        username = %auth.username,
        slug = %slug,
        moderation_mode = %moderation_mode,
        has_approved_comment = has_approved,
        final_status = %status,
        "creating comment"
    );

    let (comment_id, created_at) = repository::insert_comment(
        &state.pool,
        &post.id,
        &auth.id,
        &final_content,
        body.parent_id.as_deref(),
        status,
    )
    .await?;
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add src/modules/comment/service.rs
git commit -m "feat: comment service 集成 before_create 钩子调用点"
```

---

## Task 7: before_render 集成到主题渲染链路

**Files:**
- Modify: `src/modules/theme/handler.rs`

**目的:** 在 `render_post` 中，构建 `TemplateContext` 后、渲染模板前，调用 `post.before_render` filter（best_effort 策略）。

- [ ] **Step 1: 修改 render_post**

在顶层 import 区域添加：

```rust
use crate::modules::plugin::hook::{HookContext, HookData, PostBeforeRenderData};
```

在 `render_post` 函数中，`let env = engine::build_template_engine(...)` 之前，插入钩子调用：

```rust
    let ctx = TemplateContext::load(&state).await?;

    let hook_registry = state.plugin_manager.hook_registry();
    let mut hook_ctx = HookContext {
        hook_name: "post.before_render".into(),
        data: HookData::PostBeforeRender(PostBeforeRenderData {
            post_id: p.id.clone(),
            title: p.title.clone(),
            slug: p.slug.clone(),
            content_html: p.content_html.clone(),
            extra: std::collections::HashMap::new(),
        }),
    };
    hook_registry.dispatch_filter_best_effort("post.before_render", &mut hook_ctx).await;

    let extra_context = match &hook_ctx.data {
        HookData::PostBeforeRender(d) => d.extra.clone(),
        _ => std::collections::HashMap::new(),
    };

    let seo_meta = crate::modules::seo::meta::build_post_meta_with_content_type(
        &ctx.site_title,
        &ctx.site_url,
        &p.title,
        &p.slug,
        p.excerpt.as_deref(),
        &p.content_html,
        "",
        "",
        &p.content_type,
    );

    let comments = crate::modules::comment::repository::list_approved_for_post(&state.pool, &p.id)
        .await
        .unwrap_or_default();

    let env = engine::build_template_engine(&ctx, &state.theme_dir, state.plugin_manager.as_ref())?;
    let tmpl = env
        .get_template("post.html")
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {
            post => p,
            seo_meta => seo_meta,
            comments => comments,
            current_user => auth,
            plugin_context => extra_context
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add src/modules/theme/handler.rs
git commit -m "feat: render_post 集成 post.before_render 钩子（best_effort 策略）"
```

---

## Task 8: HelloWorld 注册示例钩子 + HookHandler 实现范本

**Files:**
- Modify: `plugins/hello-world-a3f9b2c1/lib.rs`

**目的:** 为 HelloWorld 插件添加一个 `post.after_publish` action 钩子作为范本，演示插件如何实现 `HookHandler` trait。

- [ ] **Step 1: 修改 lib.rs**

在文件头部 `use` 区域添加：

```rust
use async_trait::async_trait;
use crate::modules::plugin::hook::{Hook, HookContext, HookHandler};
```

在 `use crate::shared::error::AppResult;` 下添加：

```rust
use crate::shared::error::{AppResult, AppError};
```

在 `impl Plugin for HelloWorldPlugin` 块的 `fn version()` 方法之后添加 `hooks()` 实现：

```rust
    fn hooks(&self) -> Vec<Hook> {
        vec![
            Hook::new_action(
                "post.after_publish",
                10,
                self.name(),
                std::sync::Arc::new(LogPublishHook),
            ),
        ]
    }
```

在文件末尾（`impl Plugin for HelloWorldPlugin` 块闭合之后）添加 `LogPublishHook` 定义：

```rust
struct LogPublishHook;

#[async_trait]
impl HookHandler for LogPublishHook {
    async fn run(&self, ctx: &mut HookContext) -> AppResult<()> {
        match &ctx.data {
            crate::modules::plugin::hook::HookData::PostAfterPublish(data) => {
                tracing::info!(
                    module = "plugin",
                    plugin = "hello-world-a3f9b2c1",
                    hook = "post.after_publish",
                    post_id = %data.post_id,
                    title = %data.title,
                    "HelloWorld: post published!"
                );
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
```

- [ ] **Step 2: 验证 build.rs 仍能注册**

```bash
cargo check -p inkforge
```

Expected: 编译通过。build stage 输出 `registering plugin: hello-world-a3f9b2c1`。

- [ ] **Step 3: Commit**

```bash
git add plugins/hello-world-a3f9b2c1/lib.rs
git commit -m "feat: HelloWorld 插件注册 post.after_publish 示例钩子"
```

---

## Task 9: 编写测试

**Files:**
- Create: `src/tests/plugin_hook_tests.rs`
- Modify: `src/tests.rs`

**目的:** 覆盖 HookRegistry 的核心行为：注册+分发、优先级排序、Filter 失败中断、Action 超时不阻塞、卸载、空注册表安全分发。

- [ ] **Step 1: 创建 plugin_hook_tests.rs**

```rust
#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{Arc, Mutex};

    use crate::modules::plugin::hook::{
        Hook, HookContext, HookData, HookHandler, PostAfterPublishData, PostBeforeSaveData,
    };
    use crate::modules::plugin::hook_registry::HookRegistry;
    use crate::shared::error::{AppError, AppResult};

    struct CounterHook {
        counter: Arc<AtomicI32>,
    }

    #[async_trait]
    impl HookHandler for CounterHook {
        async fn run(&self, ctx: &mut HookContext) -> AppResult<()> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            if let HookData::PostBeforeSave(data) = &mut ctx.data {
                data.content_html.push_str(" [modified by counter]");
            }
            Ok(())
        }
    }

    struct FailHook;

    #[async_trait]
    impl HookHandler for FailHook {
        async fn run(&self, _ctx: &mut HookContext) -> AppResult<()> {
            Err(AppError::BadRequest("fail hook triggered".into()))
        }
    }

    struct SlowHook;

    #[async_trait]
    impl HookHandler for SlowHook {
        async fn run(&self, _ctx: &mut HookContext) -> AppResult<()> {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            Ok(())
        }
    }

    struct RecordHook {
        name: String,
        records: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl HookHandler for RecordHook {
        async fn run(&self, _ctx: &mut HookContext) -> AppResult<()> {
            self.records.lock().unwrap().push(self.name.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn register_and_dispatch_filter_calls_handler() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicI32::new(0));

        registry
            .register(
                "test-plugin",
                vec![Hook::new_filter(
                    "post.before_save",
                    10,
                    "test-plugin",
                    Arc::new(CounterHook {
                        counter: counter.clone(),
                    }),
                )],
            )
            .await;

        let mut ctx = HookContext {
            hook_name: "post.before_save".into(),
            data: HookData::PostBeforeSave(PostBeforeSaveData {
                title: "hello".into(),
                content_html: "world".into(),
                excerpt: None,
                slug: "hello".into(),
                tags: vec![],
                category_id: None,
                content_type: "post".into(),
                request_ip: None,
                user_agent: None,
            }),
        };

        registry
            .dispatch_filter("post.before_save", &mut ctx)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        match &ctx.data {
            HookData::PostBeforeSave(d) => {
                assert!(d.content_html.contains("[modified by counter]"));
            }
            _ => panic!("expected PostBeforeSave data"),
        }
    }

    #[tokio::test]
    async fn multiple_plugins_sorted_by_priority_then_name() {
        let registry = HookRegistry::new();
        let records = Arc::new(Mutex::new(Vec::new()));

        registry
            .register(
                "plugin-b",
                vec![Hook::new_filter(
                    "post.before_save",
                    10,
                    "plugin-b",
                    Arc::new(RecordHook {
                        name: "b".into(),
                        records: records.clone(),
                    }),
                )],
            )
            .await;

        registry
            .register(
                "plugin-a",
                vec![Hook::new_filter(
                    "post.before_save",
                    5,
                    "plugin-a",
                    Arc::new(RecordHook {
                        name: "a".into(),
                        records: records.clone(),
                    }),
                )],
            )
            .await;

        let mut ctx = HookContext {
            hook_name: "post.before_save".into(),
            data: HookData::PostBeforeSave(PostBeforeSaveData {
                title: "hello".into(),
                content_html: "world".into(),
                excerpt: None,
                slug: "hello".into(),
                tags: vec![],
                category_id: None,
                content_type: "post".into(),
                request_ip: None,
                user_agent: None,
            }),
        };

        registry
            .dispatch_filter("post.before_save", &mut ctx)
            .await
            .unwrap();

        let order = records.lock().unwrap();
        assert_eq!(order[0], "a");
        assert_eq!(order[1], "b");
    }

    #[tokio::test]
    async fn filter_failure_interrupts_pipeline() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicI32::new(0));

        registry
            .register(
                "fail-first",
                vec![
                    Hook::new_filter(
                        "post.before_save",
                        1,
                        "fail-first",
                        Arc::new(FailHook),
                    ),
                    Hook::new_filter(
                        "post.before_save",
                        2,
                        "never-reach",
                        Arc::new(CounterHook {
                            counter: counter.clone(),
                        }),
                    ),
                ],
            )
            .await;

        let mut ctx = HookContext {
            hook_name: "post.before_save".into(),
            data: HookData::PostBeforeSave(PostBeforeSaveData {
                title: "hello".into(),
                content_html: "world".into(),
                excerpt: None,
                slug: "hello".into(),
                tags: vec![],
                category_id: None,
                content_type: "post".into(),
                request_ip: None,
                user_agent: None,
            }),
        };

        let result = registry.dispatch_filter("post.before_save", &mut ctx).await;
        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn action_timeout_does_not_block() {
        let registry = HookRegistry::new();

        registry
            .register(
                "slow-plugin",
                vec![Hook::new_action(
                    "post.after_publish",
                    10,
                    "slow-plugin",
                    Arc::new(SlowHook),
                )],
            )
            .await;

        let ctx = HookContext {
            hook_name: "post.after_publish".into(),
            data: HookData::PostAfterPublish(PostAfterPublishData {
                post_id: "1".into(),
                title: "hi".into(),
                slug: "hi".into(),
                old_status: "draft".into(),
                new_status: "published".into(),
            }),
        };

        let start = tokio::time::Instant::now();
        registry.dispatch_action("post.after_publish", &ctx).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(7),
            "dispatch_action should return promptly even with slow hooks: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn unregister_all_removes_plugin_hooks() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicI32::new(0));

        registry
            .register(
                "removable",
                vec![Hook::new_filter(
                    "post.before_save",
                    10,
                    "removable",
                    Arc::new(CounterHook {
                        counter: counter.clone(),
                    }),
                )],
            )
            .await;

        registry.unregister_all("removable").await;

        let mut ctx = HookContext {
            hook_name: "post.before_save".into(),
            data: HookData::PostBeforeSave(PostBeforeSaveData {
                title: "hello".into(),
                content_html: "world".into(),
                excerpt: None,
                slug: "hello".into(),
                tags: vec![],
                category_id: None,
                content_type: "post".into(),
                request_ip: None,
                user_agent: None,
            }),
        };

        registry
            .dispatch_filter("post.before_save", &mut ctx)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn dispatch_on_unregistered_hook_returns_ok() {
        let registry = HookRegistry::new();

        let mut ctx = HookContext {
            hook_name: "post.before_save".into(),
            data: HookData::PostBeforeSave(PostBeforeSaveData {
                title: "hello".into(),
                content_html: "world".into(),
                excerpt: None,
                slug: "hello".into(),
                tags: vec![],
                category_id: None,
                content_type: "post".into(),
                request_ip: None,
                user_agent: None,
            }),
        };

        let result = registry
            .dispatch_filter("post.before_save", &mut ctx)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn dispatch_filter_best_effort_skips_failing_plugin() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicI32::new(0));

        registry
            .register(
                "test-plugin",
                vec![
                    Hook::new_filter(
                        "post.before_render",
                        1,
                        "failer",
                        Arc::new(FailHook),
                    ),
                    Hook::new_filter(
                        "post.before_render",
                        2,
                        "runner",
                        Arc::new(CounterHook {
                            counter: counter.clone(),
                        }),
                    ),
                ],
            )
            .await;

        let mut ctx = HookContext {
            hook_name: "post.before_render".into(),
            data: HookData::PostBeforeSave(PostBeforeSaveData {
                title: "hello".into(),
                content_html: "world".into(),
                excerpt: None,
                slug: "hello".into(),
                tags: vec![],
                category_id: None,
                content_type: "post".into(),
                request_ip: None,
                user_agent: None,
            }),
        };

        registry
            .dispatch_filter_best_effort("post.before_render", &mut ctx)
            .await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
```

- [ ] **Step 2: 更新 src/tests.rs**

在 `src/tests.rs` 末尾的测试模块列表中添加：

```rust
mod plugin_hook_tests;
```

修改后的 `src/tests.rs` 末尾：

```rust
mod plugin_manager_tests;
mod plugin_manifest_tests;
mod plugin_id_strategy_tests;
mod plugin_hook_tests;
```

- [ ] **Step 3: 运行测试**

```bash
cargo test -p inkforge plugin_hook -- --nocapture
```

Expected: 10 个测试全部通过。

- [ ] **Step 4: Commit**

```bash
git add src/tests/plugin_hook_tests.rs src/tests.rs
git commit -m "test: 添加 HookRegistry 注册/分发/优先级/超时/卸载 单元测试"
```

---

## Task 10: 全量验证 + 文档更新

**Files:**
- 运行 `cargo test -p inkforge`
- 运行 `cargo clippy -p inkforge`
- 更新 `memories/PROJECT_STATUS.md` 记录 P1a 完成

- [ ] **Step 1: 运行完整测试套件**

```bash
cargo test -p inkforge
```

Expected: 所有测试通过（含新增的 10 个 `plugin_hook_tests` 和之前所有测试）。

- [ ] **Step 2: 运行 clippy**

```bash
cargo clippy -p inkforge -- -D warnings
```

Expected: 无 warning。

- [ ] **Step 3: 更新 PROJECT_STATUS.md**

在 `memories/PROJECT_STATUS.md` 的近期优先级或插件章节中标记 P1a Hooks 系统完成，添加简要说明：5 个钩子（post.before_save/after_save/after_publish/before_render, comment.before_create）、Filter/Action 二分模型、优先级排序管道、并行 Action + 5s 超时、Plugin trait hooks() 扩展、HookRegistry 注册/分发/卸载已就绪。

- [ ] **Step 4: Commit**

```bash
git add memories/PROJECT_STATUS.md
git commit -m "docs: 标记 P1a Hooks 系统完成"
```

---

## 验证清单

| 检查项 | 命令 | 期望 |
|--------|------|------|
| 编译通过 | `cargo check -p inkforge` | 0 errors |
| 测试全绿 | `cargo test -p inkforge` | All passed |
| Clippy 无警告 | `cargo clippy -p inkforge -- -D warnings` | 0 warnings |
| hook.rs 存在 | `Test-Path src/modules/plugin/hook.rs` | True |
| hook_registry.rs 存在 | `Test-Path src/modules/plugin/hook_registry.rs` | True |
| Plugin trait 含 hooks() | `rg "fn hooks" src/modules/plugin/mod.rs` | 2 处（trait 定义 1 + 插件 impl 1） |
| PluginManager 含 hook_registry | `rg "hook_registry" src/modules/plugin/manager.rs` | >2 处 |
| post service 含 before_save | `rg "before_save" src/modules/post/service.rs` | >2 处 |
| comment service 含 before_create | `rg "before_create" src/modules/comment/service.rs` | >1 处 |
| theme handler 含 before_render | `rg "before_render" src/modules/theme/handler.rs` | >1 处 |
| HelloWorld 含 hooks() | `rg "fn hooks" plugins/hello-world-a3f9b2c1/lib.rs` | 1 处 |
| 测试模块已注册 | `rg "plugin_hook_tests" src/tests.rs` | 1 处 |
| 测试通过 | `cargo test -p inkforge plugin_hook` | 10 passed |
