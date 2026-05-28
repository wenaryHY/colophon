# Phase 2 — 模板引擎 Trait 解耦 Implementation Plan

**状态:** ✅ 已完成（2026-05-29）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `TemplateContext::load()` 中的直接 DB 查询抽象为 `TemplateDataProvider` trait，使模板引擎可独立测试、可缓存、可替换数据源。

**Architecture:** 定义 `TemplateDataProvider` async trait（使用 `async-trait` crate），实现 `DbTemplateDataProvider` 包裹当前 DB 调用，重构 `TemplateContext::load()` 为 `TemplateContext::from_provider()`。添加 `CachedTemplateDataProvider` 包裹任意 provider 提供 TTL 缓存。保留 `TemplateContext::load(state)` 作为便捷方法以最小化 caller 变更。

**Tech Stack:** Rust, async-trait, tokio (RwLock for cache), MiniJinja

**Pre-requisites:** Phase 1 已完成（6 commits merged），`cargo test -p inkforge` 全绿。

**运行测试命令:** `cargo test -p inkforge`

---

## 文件结构

| 文件 | 职责 | 操作 |
|------|------|------|
| `Cargo.toml` | 添加 `async-trait` 依赖 | 修改 |
| `src/modules/theme/provider.rs` | `TemplateDataProvider` trait + `DbTemplateDataProvider` 实现 | 新建 |
| `src/modules/theme/cache.rs` | `CachedTemplateDataProvider` TTL 缓存包装器 | 新建 |
| `src/modules/theme/context.rs` | 重构 `load()` → 内部调用 `from_provider` | 修改 |
| `src/modules/theme/mod.rs` | 导出新模块 | 修改 |
| `src/modules/theme/engine.rs` | 无变更（已经是纯同步） | 不动 |
| `src/tests/theme_engine_tests.rs` | `build_template_engine` 单元测试 | 新建 |
| `src/tests/theme_provider_tests.rs` | provider trait + cache 单元测试 | 新建 |
| `src/tests.rs` | 注册新测试模块 | 修改 |

---

## Task 1: 为 `build_template_engine` 补充基线测试

**Files:**
- Create: `src/tests/theme_engine_tests.rs`
- Modify: `src/tests.rs`

**目的:** 在重构之前锁定当前行为。`build_template_engine` 是纯同步函数，接受手动构造的 `TemplateContext`，不需要 DB。

- [ ] **Step 1: 创建测试文件**

```rust
// src/tests/theme_engine_tests.rs
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use crate::modules::theme::context::TemplateContext;
    use crate::modules::theme::engine::build_template_engine;

    fn sample_context() -> TemplateContext {
        TemplateContext {
            active_theme: "default".to_string(),
            site_title: "Test Blog".to_string(),
            site_description: "A test blog".to_string(),
            site_url: "http://localhost:2000".to_string(),
            admin_url: "/admin".to_string(),
            theme_config: None,
            recent_posts: vec![],
            tags: vec![],
            categories: vec![],
        }
    }

    #[test]
    fn build_engine_succeeds_with_valid_theme_dir() {
        let theme_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
        let ctx = sample_context();
        let env = build_template_engine(&ctx, &theme_dir);
        assert!(env.is_ok(), "build_template_engine should succeed: {:?}", env.err());
    }

    #[test]
    fn build_engine_sets_globals() {
        let theme_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
        let ctx = sample_context();
        let env = build_template_engine(&ctx, &theme_dir).unwrap();
        let globals = env.globals();
        assert_eq!(globals.get_attr("site_title").unwrap().to_string(), "Test Blog");
        assert_eq!(globals.get_attr("site_url").unwrap().to_string(), "http://localhost:2000");
    }

    #[test]
    fn build_engine_renders_index_template() {
        let theme_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
        let ctx = sample_context();
        let env = build_template_engine(&ctx, &theme_dir).unwrap();
        let tmpl = env.get_template("index.html");
        assert!(tmpl.is_ok(), "index.html should be loadable from default theme");
    }

    #[test]
    fn build_engine_get_recent_posts_returns_empty_vec() {
        let theme_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
        let ctx = sample_context();
        let env = build_template_engine(&ctx, &theme_dir).unwrap();
        let func = env.globals().get_attr("get_recent_posts");
        // get_recent_posts is registered as a function, not a global attribute
        // We verify it via template rendering
        let result = env.render_str("{{ get_recent_posts() }}", minijinja::context!());
        assert!(result.is_ok());
    }

    #[test]
    fn build_engine_theme_assets_url_generates_correct_path() {
        let theme_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
        let ctx = sample_context();
        let env = build_template_engine(&ctx, &theme_dir).unwrap();
        let result = env.render_str(
            "{{ theme_assets_url('css/style.css') }}",
            minijinja::context!(),
        ).unwrap();
        assert_eq!(result, "/static/themes/default/css/style.css");
    }

    #[test]
    fn build_engine_rejects_path_traversal_in_loader() {
        let theme_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
        let ctx = sample_context();
        let env = build_template_engine(&ctx, &theme_dir).unwrap();
        let result = env.get_template("../../Cargo.toml");
        assert!(result.is_err(), "path traversal should be blocked");
    }
}
```

- [ ] **Step 2: 注册测试模块**

在 `src/tests.rs` 末尾添加：

```rust
#[cfg(test)]
mod theme_engine_tests;
```

注意：由于 `src/tests.rs` 使用 `mod` 声明子模块，但测试文件在 `src/tests/` 目录下，需确认模块路径正确。检查现有的模块声明方式：如果 `src/tests.rs` 是一个文件（非目录的 `mod.rs`），则子模块文件应在 `src/tests/` 目录下。

- [ ] **Step 3: 运行测试验证基线**

Run: `cargo test -p inkforge theme_engine_tests -- --nocapture`
Expected: 所有测试通过（部分可能需要调整 MiniJinja API 细节）

- [ ] **Step 4: Commit**

```bash
git add src/tests/theme_engine_tests.rs src/tests.rs
git commit -m "test: 添加模板引擎基线测试覆盖"
```

---

## Task 2: 添加 `async-trait` 依赖

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 在 `[dependencies]` 中添加 async-trait**

在 `Cargo.toml` 的 `[dependencies]` 部分（工具类附近）添加：

```toml
async-trait = "=0.1.88"
```

- [ ] **Step 2: 验证编译通过**

Run: `cargo check -p inkforge`
Expected: 编译通过，无错误

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: 添加 async-trait 依赖（为 TemplateDataProvider trait 准备）"
```

---

## Task 3: 定义 `TemplateDataProvider` trait + `DbTemplateDataProvider`

**Files:**
- Create: `src/modules/theme/provider.rs`
- Modify: `src/modules/theme/mod.rs`

- [ ] **Step 1: 创建 provider.rs 定义 trait 和 DB 实现**

```rust
// src/modules/theme/provider.rs
use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::modules::category::domain::Category;
use crate::modules::post::domain::PublicPostSummary;
use crate::modules::tag::domain::Tag;
use crate::modules::theme::ThemeConfig;
use crate::shared::error::AppResult;

#[async_trait]
pub trait TemplateDataProvider: Send + Sync {
    async fn get_active_theme(&self) -> AppResult<String>;
    async fn get_setting(&self, key: &str, default: &str) -> String;
    async fn get_theme_config(&self, slug: &str) -> Option<ThemeConfig>;
    async fn get_recent_posts(&self, limit: i64) -> Vec<PublicPostSummary>;
    async fn get_tags(&self) -> Vec<Tag>;
    async fn get_categories(&self) -> Vec<Category>;
}

pub struct DbTemplateDataProvider<'a> {
    pool: &'a SqlitePool,
}

impl<'a> DbTemplateDataProvider<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl<'a> TemplateDataProvider for DbTemplateDataProvider<'a> {
    async fn get_active_theme(&self) -> AppResult<String> {
        super::repository::get_active_theme(self.pool).await
    }

    async fn get_setting(&self, key: &str, default: &str) -> String {
        crate::modules::setting::repository::get_string(self.pool, key, default)
            .await
            .unwrap_or_else(|_| default.to_string())
    }

    async fn get_theme_config(&self, slug: &str) -> Option<ThemeConfig> {
        super::repository::get_config(self.pool, slug)
            .await
            .unwrap_or_default()
    }

    async fn get_recent_posts(&self, limit: i64) -> Vec<PublicPostSummary> {
        crate::modules::post::repository::list_recent_public_posts(self.pool, limit)
            .await
            .unwrap_or_default()
    }

    async fn get_tags(&self) -> Vec<Tag> {
        crate::modules::tag::repository::list_tags(self.pool)
            .await
            .unwrap_or_default()
    }

    async fn get_categories(&self) -> Vec<Category> {
        crate::modules::category::repository::list_categories(self.pool)
            .await
            .unwrap_or_default()
    }
}
```

- [ ] **Step 2: 在 mod.rs 注册模块**

在 `src/modules/theme/mod.rs` 中添加：

```rust
pub mod provider;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p inkforge`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src/modules/theme/provider.rs src/modules/theme/mod.rs
git commit -m "feat: 定义 TemplateDataProvider trait 及 DbTemplateDataProvider 实现"
```

---

## Task 4: 重构 `TemplateContext::load()` 使用 provider

**Files:**
- Modify: `src/modules/theme/context.rs`

**设计:** 添加 `from_provider()` 泛型方法，将 `load()` 改为内部使用 `DbTemplateDataProvider` 的便捷方法。所有现有调用方（6 处）无需修改。

- [ ] **Step 1: 重构 context.rs**

```rust
// src/modules/theme/context.rs
use crate::modules::category::domain::Category;
use crate::modules::post::domain::PublicPostSummary;
use crate::modules::tag::domain::Tag;
use crate::modules::theme::ThemeConfig;
use crate::shared::error::AppResult;
use crate::state::AppState;
use std::sync::Arc;

use super::provider::{DbTemplateDataProvider, TemplateDataProvider};

const DEFAULT_POST_LIMIT: i64 = 10;

#[derive(Debug, Clone)]
pub struct TemplateContext {
    pub active_theme: String,
    pub site_title: String,
    pub site_description: String,
    pub site_url: String,
    pub admin_url: String,
    pub theme_config: Option<ThemeConfig>,
    pub recent_posts: Vec<PublicPostSummary>,
    pub tags: Vec<Tag>,
    pub categories: Vec<Category>,
}

impl TemplateContext {
    pub async fn from_provider(provider: &dyn TemplateDataProvider) -> AppResult<Self> {
        let active_theme = provider.get_active_theme().await?;
        let site_title = provider.get_setting("site_title", "InkForge").await;
        let site_description = provider.get_setting("site_description", "").await;
        let site_url = provider.get_setting("site_url", "").await;
        let admin_url = provider.get_setting("admin_url", "/admin").await;
        let theme_config = provider.get_theme_config(&active_theme).await;
        let recent_posts = provider.get_recent_posts(DEFAULT_POST_LIMIT).await;
        let tags = provider.get_tags().await;
        let categories = provider.get_categories().await;

        Ok(Self {
            active_theme,
            site_title,
            site_description,
            site_url,
            admin_url,
            theme_config,
            recent_posts,
            tags,
            categories,
        })
    }

    pub async fn load(state: &Arc<AppState>) -> AppResult<Self> {
        let provider = DbTemplateDataProvider::new(&state.pool);
        Self::from_provider(&provider).await
    }
}
```

**注意:** 需要给 `TemplateContext` 添加 `Clone` derive（缓存层需要 clone）。同时 `from_provider` 使用 `&dyn TemplateDataProvider` trait object — 这要求 trait 是 object-safe 的，`async-trait` 会自动处理。

- [ ] **Step 2: 运行全量测试确认无回归**

Run: `cargo test -p inkforge`
Expected: 所有测试通过（31 unit + 1 integration + 新的 theme_engine_tests）

- [ ] **Step 3: Commit**

```bash
git add src/modules/theme/context.rs
git commit -m "refactor: TemplateContext::load 内部改用 TemplateDataProvider trait"
```

---

## Task 5: 添加 `CachedTemplateDataProvider`

**Files:**
- Create: `src/modules/theme/cache.rs`
- Modify: `src/modules/theme/mod.rs`

**设计:** 包装任何 `TemplateDataProvider`，缓存 `TemplateContext` 整体（非逐字段），使用 TTL 过期。缓存粒度为整个上下文，因为 handler 每次请求都会重建，缓存可以避免 9 次 DB 查询。

- [ ] **Step 1: 创建 cache.rs**

```rust
// src/modules/theme/cache.rs
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::context::TemplateContext;
use super::provider::TemplateDataProvider;
use crate::shared::error::AppResult;

const DEFAULT_CACHE_TTL_SECS: u64 = 30;

struct CacheEntry {
    context: TemplateContext,
    created_at: Instant,
}

pub struct TemplateContextCache {
    entry: Arc<RwLock<Option<CacheEntry>>>,
    ttl: Duration,
}

impl TemplateContextCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            entry: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub fn with_default_ttl() -> Self {
        Self::new(DEFAULT_CACHE_TTL_SECS)
    }

    pub async fn get_or_load(&self, provider: &dyn TemplateDataProvider) -> AppResult<TemplateContext> {
        {
            let guard = self.entry.read().await;
            if let Some(ref cached) = *guard {
                if cached.created_at.elapsed() < self.ttl {
                    return Ok(cached.context.clone());
                }
            }
        }

        let ctx = TemplateContext::from_provider(provider).await?;
        {
            let mut guard = self.entry.write().await;
            *guard = Some(CacheEntry {
                context: ctx.clone(),
                created_at: Instant::now(),
            });
        }
        Ok(ctx)
    }

    pub async fn invalidate(&self) {
        let mut guard = self.entry.write().await;
        *guard = None;
    }
}
```

- [ ] **Step 2: 在 mod.rs 注册模块**

在 `src/modules/theme/mod.rs` 中添加：

```rust
pub mod cache;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p inkforge`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src/modules/theme/cache.rs src/modules/theme/mod.rs
git commit -m "feat: 添加 TemplateContextCache TTL 缓存层"
```

---

## Task 6: 为 provider 和 cache 编写测试

**Files:**
- Create: `src/tests/theme_provider_tests.rs`
- Modify: `src/tests.rs`

- [ ] **Step 1: 创建 provider 测试文件（使用 mock provider）**

```rust
// src/tests/theme_provider_tests.rs
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    use async_trait::async_trait;

    use crate::modules::category::domain::Category;
    use crate::modules::post::domain::PublicPostSummary;
    use crate::modules::tag::domain::Tag;
    use crate::modules::theme::cache::TemplateContextCache;
    use crate::modules::theme::context::TemplateContext;
    use crate::modules::theme::provider::TemplateDataProvider;
    use crate::modules::theme::ThemeConfig;
    use crate::shared::error::AppResult;

    struct MockProvider {
        call_count: Arc<AtomicU32>,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                call_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn calls(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl TemplateDataProvider for MockProvider {
        async fn get_active_theme(&self) -> AppResult<String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok("mock-theme".to_string())
        }

        async fn get_setting(&self, _key: &str, default: &str) -> String {
            default.to_string()
        }

        async fn get_theme_config(&self, _slug: &str) -> Option<ThemeConfig> {
            None
        }

        async fn get_recent_posts(&self, _limit: i64) -> Vec<PublicPostSummary> {
            vec![]
        }

        async fn get_tags(&self) -> Vec<Tag> {
            vec![]
        }

        async fn get_categories(&self) -> Vec<Category> {
            vec![]
        }
    }

    #[tokio::test]
    async fn from_provider_populates_all_fields() {
        let provider = MockProvider::new();
        let ctx = TemplateContext::from_provider(&provider).await.unwrap();
        assert_eq!(ctx.active_theme, "mock-theme");
        assert_eq!(ctx.site_title, "InkForge");
        assert_eq!(ctx.admin_url, "/admin");
        assert!(ctx.recent_posts.is_empty());
        assert!(ctx.tags.is_empty());
        assert!(ctx.categories.is_empty());
    }

    #[tokio::test]
    async fn cache_avoids_redundant_provider_calls() {
        let provider = MockProvider::new();
        let cache = TemplateContextCache::new(60);

        let ctx1 = cache.get_or_load(&provider).await.unwrap();
        assert_eq!(provider.calls(), 1);

        let ctx2 = cache.get_or_load(&provider).await.unwrap();
        assert_eq!(provider.calls(), 1, "second call should use cache");
        assert_eq!(ctx1.active_theme, ctx2.active_theme);
    }

    #[tokio::test]
    async fn cache_expires_after_ttl() {
        let provider = MockProvider::new();
        let cache = TemplateContextCache::new(0); // TTL=0 即刻过期

        let _ctx1 = cache.get_or_load(&provider).await.unwrap();
        assert_eq!(provider.calls(), 1);

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let _ctx2 = cache.get_or_load(&provider).await.unwrap();
        assert_eq!(provider.calls(), 2, "expired cache should re-fetch");
    }

    #[tokio::test]
    async fn cache_invalidate_forces_reload() {
        let provider = MockProvider::new();
        let cache = TemplateContextCache::new(60);

        let _ctx1 = cache.get_or_load(&provider).await.unwrap();
        assert_eq!(provider.calls(), 1);

        cache.invalidate().await;

        let _ctx2 = cache.get_or_load(&provider).await.unwrap();
        assert_eq!(provider.calls(), 2, "invalidated cache should re-fetch");
    }
}
```

- [ ] **Step 2: 在 src/tests.rs 注册新模块**

```rust
#[cfg(test)]
mod theme_provider_tests;
```

- [ ] **Step 3: 运行测试**

Run: `cargo test -p inkforge theme_provider_tests -- --nocapture`
Expected: 4 个测试全部通过

- [ ] **Step 4: Commit**

```bash
git add src/tests/theme_provider_tests.rs src/tests.rs
git commit -m "test: 添加 TemplateDataProvider mock 测试 + cache TTL 测试"
```

---

## Task 7: 将缓存集成到 AppState

**Files:**
- Modify: `src/state.rs`
- Modify: `src/modules/theme/context.rs`

**设计:** 在 `AppState` 中添加一个可选的 `TemplateContextCache`，`TemplateContext::load()` 优先使用缓存。这样调用方无需任何改动。

- [ ] **Step 1: 在 AppState 中添加缓存字段**

在 `src/state.rs` 的 `AppState` struct 中添加：

```rust
use crate::modules::theme::cache::TemplateContextCache;

// 在 AppState struct 中添加：
pub template_cache: TemplateContextCache,
```

在 `AppState::new()` 中初始化：

```rust
template_cache: TemplateContextCache::with_default_ttl(),
```

- [ ] **Step 2: 修改 TemplateContext::load 使用缓存**

```rust
// src/modules/theme/context.rs - load 方法
pub async fn load(state: &Arc<AppState>) -> AppResult<Self> {
    let provider = DbTemplateDataProvider::new(&state.pool);
    state.template_cache.get_or_load(&provider).await
}
```

- [ ] **Step 3: 运行全量测试**

Run: `cargo test -p inkforge`
Expected: 所有测试通过

- [ ] **Step 4: Commit**

```bash
git add src/state.rs src/modules/theme/context.rs
git commit -m "feat: 将模板上下文缓存集成到 AppState（30s TTL）"
```

---

## Task 8: 在主题/设置变更时失效缓存

**Files:**
- Modify: `src/modules/theme/handler.rs` (activate_theme, save_theme_config)
- Modify: `src/modules/setting/handler.rs` (如果有 save setting 的地方)

**设计:** 当管理员切换主题、保存主题配置、或修改站点设置时，调用 `state.template_cache.invalidate().await` 使缓存失效。

- [ ] **Step 1: 在 activate_theme handler 中失效缓存**

在 `src/modules/theme/handler.rs` 的 `activate_theme` 函数中，`service.activate_theme()` 成功后添加：

```rust
state.template_cache.invalidate().await;
```

- [ ] **Step 2: 在 save_theme_config handler 中失效缓存**

在 `save_theme_config` 函数中，保存成功后添加：

```rust
state.template_cache.invalidate().await;
```

- [ ] **Step 3: 查找并更新设置保存路径**

搜索 `setting` 模块中修改 `site_title`/`site_description`/`site_url`/`admin_url` 的位置，在相关 handler 中添加缓存失效。

- [ ] **Step 4: 运行全量测试**

Run: `cargo test -p inkforge`
Expected: 所有测试通过

- [ ] **Step 5: Commit**

```bash
git add src/modules/theme/handler.rs src/modules/setting/handler.rs
git commit -m "feat: 主题/设置变更时自动失效模板缓存"
```

---

## Task 9: 最终验证 + 文档更新

**Files:**
- 运行 `cargo test -p inkforge`
- 运行 `cargo clippy -p inkforge`
- 更新 `memories/PROJECT_STATUS.md` 记录 Phase 2 完成

- [ ] **Step 1: 运行完整测试套件**

Run: `cargo test -p inkforge`
Expected: 所有测试通过（原有 31+1 + 新增 ~10 个模板测试）

- [ ] **Step 2: 运行 clippy**

Run: `cargo clippy -p inkforge -- -D warnings`
Expected: 无 warning

- [ ] **Step 3: 更新 PROJECT_STATUS.md**

在 `memories/PROJECT_STATUS.md` 的技术债或近期优先级中标记 Phase 2 完成。

- [ ] **Step 4: Commit**

```bash
git add memories/PROJECT_STATUS.md
git commit -m "docs: 标记 Phase 2 模板 trait 解耦完成"
```

---

## 验证清单

| 检查项 | 命令 | 期望 |
|--------|------|------|
| 编译通过 | `cargo check -p inkforge` | 0 errors |
| 测试全绿 | `cargo test -p inkforge` | All passed |
| Clippy 无警告 | `cargo clippy -p inkforge -- -D warnings` | 0 warnings |
| 6 个调用方无变更 | `rg "TemplateContext::load" src/` | 仍然是 6 处 |
| 缓存命中 | 手动测试连续刷新首页 | 第二次无 DB 查询日志 |
