# P1b — 配置面板 Implementation Plan

**状态:** 🔲 待实施

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现插件配置面板系统：manifest 声明式简单配置（text/textarea/bool/select/number）、复杂配置 via iframe admin.html、`plugin_settings` 表 CRUD、API handler、Admin UI 设置页。

**Architecture:**

```
plugin.toml
  [[settings]]          → PluginManifest::settings: Vec<SettingDef>
  [admin]               → PluginManifest::admin: AdminMeta
  [resources]           → PluginManifest::resources: ResourcesMeta
       ↓ (PluginLoader::discover 时解析)
PluginManager::discovered_manifests: Vec<PluginManifest>
       ↓ (handler 运行时获取 schema)
handler::get_settings(plugin_name)
       ├── 读 manifest → SettingDef[] (schema)
       ├── 读 plugin_settings 表 → values
       └── 返回 { plugin_name, settings, values }
handler::update_settings(plugin_name, body)
       └── 写 plugin_settings 表 (UPSERT)
Admin UI PluginSettings.tsx
       ├── GET → 渲染表单 (text/textarea/bool/select/number)
       └── PUT → 保存
```

**Tech Stack:** Rust, serde, toml, sqlx (SqlitePool), axum, chrono, React 19, TypeScript, react-router-dom

**Pre-requisites:** Phase 1–4a + P1a Hooks 系统已完成，`cargo test -p inkforge` 全绿。

**运行测试命令:** `cargo test -p inkforge plugin_settings`

---

## 文件结构

| 文件 | 职责 | 操作 |
|------|------|------|
| `migrations/015_plugin_settings.sql` | plugin_settings 表 | 新建 |
| `src/modules/plugin/settings.rs` | PluginSettingDef/SettingValue 类型 + CRUD | 新建 |
| `src/modules/plugin/handler.rs` | GET/PUT /api/v1/admin/plugins/:name/settings | 新建 |
| `src/modules/plugin/manifest.rs` | 扩展结构体支持 settings/admin/resources/slots 段 | 修改 |
| `src/modules/plugin/manager.rs` | PluginManager 存储 discovered manifests + 暴露 get_manifest | 修改 |
| `src/modules/plugin/mod.rs` | 注册 settings/handler 子模块 | 修改 |
| `src/bootstrap/router.rs` | 添加 settings API 路由 | 修改 |
| `plugins/hello-world-a3f9b2c1/plugin.toml` | 添加示例配置项 | 修改 |
| `src/admin/ui/src/pages/PluginSettings.tsx` | 插件设置页面 UI | 新建 |
| `src/admin/ui/src/App.tsx` | 添加 /plugins/:name/settings 路由 | 修改 |
| `src/tests/plugin_settings_tests.rs` | 配置解析+CRUD 测试 | 新建 |
| `src/tests.rs` | 注册测试模块 | 修改 |

---

## Task 1: 创建 plugin_settings 表

**Files:**
- Create: `migrations/015_plugin_settings.sql`

**目的:** 创建插件设置 k-v 存储表，使用 (plugin_name, key) 复合主键。

- [ ] **Step 1: 创建 015_plugin_settings.sql**

```sql
CREATE TABLE IF NOT EXISTS plugin_settings (
    plugin_name TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plugin_name, key)
);
```

- [ ] **Step 2: 验证迁移可执行**

```bash
sqlite3 data/inkforge.db < migrations/015_plugin_settings.sql
```

Expected: 表创建成功，无报错。

- [ ] **Step 3: Commit**

```bash
git add migrations/015_plugin_settings.sql
git commit -m "feat: 创建 plugin_settings 表 (plugin_name, key, value, updated_at)"
```

---

## Task 2: 扩展 PluginManifest 支持 settings/admin/resources/slots

**Files:**
- Modify: `src/modules/plugin/manifest.rs`

**目的:** 在 PluginManifest 结构体中添加 `resources`、`admin`、`settings`、`slots` 四个可选段及其子结构体，支持从 plugin.toml 反序列化声明式配置。

- [ ] **Step 1: 修改 manifest.rs**

完整替换 `src/modules/plugin/manifest.rs`：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub engine: Option<EngineMeta>,
    pub hooks: Option<HooksMeta>,
    pub resources: Option<ResourcesMeta>,
    pub admin: Option<AdminMeta>,
    pub settings: Option<Vec<SettingDef>>,
    pub slots: Option<Vec<SlotDef>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesMeta {
    pub admin_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminMeta {
    pub enabled: Option<bool>,
    pub entry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingDef {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub setting_type: String,
    pub default: Option<String>,
    pub description: Option<String>,
    pub options: Option<Vec<SettingOption>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotDef {
    pub target: String,
    pub label: String,
    pub entry: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl PluginManifest {
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = toml::from_str(&content)?;
        Ok(manifest)
    }
}
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。现有代码引用 `PluginManifest` 的地方（loader.rs 的 `d.manifest.plugin.id`、`d.manifest.plugin.title`）不受影响，新字段均为 `Option` 类型兼容旧 toml。

- [ ] **Step 3: Commit**

```bash
git add src/modules/plugin/manifest.rs
git commit -m "feat: PluginManifest 扩展 resources/admin/settings/slots 结构体"
```

---

## Task 3: 创建 PluginSettings CRUD

**Files:**
- Create: `src/modules/plugin/settings.rs`

**目的:** 提供 `plugin_settings` 表的 get/set/delete_all 操作，从 manifest 的 SettingDef 合并默认值与已存值。

- [ ] **Step 1: 创建 settings.rs**

```rust
use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::shared::error::AppResult;

use super::manifest::SettingDef;

pub async fn get_all(pool: &SqlitePool, plugin_name: &str) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value FROM plugin_settings WHERE plugin_name = ?",
    )
    .bind(plugin_name)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

pub async fn set(
    pool: &SqlitePool,
    plugin_name: &str,
    key: &str,
    value: &str,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO plugin_settings (plugin_name, key, value, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(plugin_name, key) DO UPDATE SET value = ?, updated_at = ?",
    )
    .bind(plugin_name)
    .bind(key)
    .bind(value)
    .bind(&now)
    .bind(value)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_all(pool: &SqlitePool, plugin_name: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM plugin_settings WHERE plugin_name = ?")
        .bind(plugin_name)
        .execute(pool)
        .await?;
    Ok(())
}

pub fn merge_with_defaults(
    defs: &[SettingDef],
    values: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = HashMap::new();
    for def in defs {
        let val = values
            .get(&def.key)
            .cloned()
            .unwrap_or_else(|| def.default.clone().unwrap_or_default());
        merged.insert(def.key.clone(), val);
    }
    merged
}
```

- [ ] **Step 2: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。`settings.rs` 已创建，依赖 `crate::shared::error::AppResult`、`crate::modules::plugin::manifest::SettingDef`、`sqlx::SqlitePool`。

- [ ] **Step 3: Commit**

```bash
git add src/modules/plugin/settings.rs
git commit -m "feat: 创建 PluginSettings CRUD (get_all/set/delete_all + 默认值合并)"
```

---

## Task 4: 创建 Settings API handler + PluginManager 暴露 manifests

**Files:**
- Create: `src/modules/plugin/handler.rs`
- Modify: `src/modules/plugin/manager.rs`

**目的:** 实现 GET / PUT 两个 API endpoint。GET 返回插件的 schema + 合并默认值的当前值；PUT 接收 k-v 对并保存。同时修改 PluginManager 存储 discovered manifests 以供给 handler 使用。

- [ ] **Step 1: 修改 manager.rs — 存储 manifests + 添加 getter**

在 `src/modules/plugin/manager.rs` 中添加字段和方法。

在 `use` 区域添加 `super::manifest::PluginManifest;`，在 `pub struct PluginManager` 中添加 `discovered_manifests` 字段：

```rust
use super::manifest::PluginManifest;
```

修改结构体：

```rust
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    hook_registry: Arc<HookRegistry>,
    discovered_manifests: Vec<PluginManifest>,
}
```

修改 `load()` 方法，添加 `discovered_manifests: Vec::new()`：

```rust
Self {
    plugins,
    hook_registry: Arc::new(HookRegistry::new()),
    discovered_manifests: Vec::new(),
}
```

修改 `load_with()` 方法，存储 manifests：

```rust
Self {
    plugins,
    hook_registry: Arc::new(HookRegistry::new()),
    discovered_manifests: discovered.into_iter().map(|d| d.manifest).collect(),
}
```

在 `impl PluginManager` 块中 `hook_registry()` 方法之后添加 getter：

```rust
pub fn get_manifest(&self, plugin_name: &str) -> Option<&PluginManifest> {
    self.discovered_manifests
        .iter()
        .find(|m| m.plugin.id == plugin_name)
}
```

- [ ] **Step 2: 创建 handler.rs**

```rust
use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

use crate::shared::error::{AppError, AppResult};
use crate::shared::response::ApiResponse;
use crate::state::AppState;

use super::manifest::SettingDef;
use super::settings;

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    Path(plugin_name): Path<String>,
) -> AppResult<impl IntoResponse> {
    let manifest = state
        .plugin_manager
        .get_manifest(&plugin_name)
        .ok_or_else(|| AppError::NotFound)?;

    let defs = manifest.settings.clone().unwrap_or_default();
    let values = settings::get_all(&state.pool, &plugin_name).await?;
    let merged = settings::merge_with_defaults(&defs, &values);

    Ok(Json(ApiResponse::success(serde_json::json!({
        "plugin_name": plugin_name,
        "settings": defs,
        "values": merged,
    }))))
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Path(plugin_name): Path<String>,
    Json(body): Json<HashMap<String, String>>,
) -> AppResult<impl IntoResponse> {
    let manifest = state
        .plugin_manager
        .get_manifest(&plugin_name)
        .ok_or_else(|| AppError::NotFound)?;

    let valid_keys: std::collections::HashSet<&str> = manifest
        .settings
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|d| d.key.as_str())
        .collect();

    for (key, value) in &body {
        if !valid_keys.contains(key.as_str()) {
            return Err(AppError::BadRequest(format!(
                "unknown setting key: {}",
                key
            )));
        }
        settings::set(&state.pool, &plugin_name, key, value).await?;
    }

    Ok(Json(ApiResponse::success(true)))
}
```

- [ ] **Step 3: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。`handler.rs` 引用 `AppState::plugin_manager`、`AppState::pool`、`settings::get_all/set/merge_with_defaults`。

- [ ] **Step 4: Commit**

```bash
git add src/modules/plugin/handler.rs src/modules/plugin/manager.rs
git commit -m "feat: 创建 settings API handler + PluginManager 存储 manifests"
```

---

## Task 5: 注册模块 + 添加路由

**Files:**
- Modify: `src/modules/plugin/mod.rs`
- Modify: `src/bootstrap/router.rs`

**目的:** 注册 `settings` 和 `handler` 子模块，在 router 中添加两条 API 路由。

- [ ] **Step 1: 修改 mod.rs — 注册子模块**

在 `pub mod loader;` 之前添加两行：

```rust
pub mod settings;
pub mod handler;
```

修改后的 `src/modules/plugin/mod.rs` 模块声明区域：

```rust
pub mod asset;
pub mod hook;
pub mod hook_registry;
pub mod registry;
pub mod manager;
pub mod manifest;
pub mod id_strategy;
pub mod status;
pub mod settings;
pub mod handler;
pub mod loader;
```

- [ ] **Step 2: 修改 router.rs — 添加路由**

在 `src/bootstrap/router.rs` 的 `v1` router 中，`.route("/api/v1/admin/trash/:item_type/:id", ...)` 之后、`.merge(state.plugin_manager.collect_routes());` 之前，添加两条路由：

```rust
        .route(
            "/api/v1/admin/plugins/:name/settings",
            get(modules::plugin::handler::get_settings)
                .put(modules::plugin::handler::update_settings),
        )
```

完整插入位置（在 router.rs 的 v1 router 构建链中）：

```rust
        .route(
            "/api/v1/admin/trash/:item_type/:id",
            delete(modules::trash::handler::purge_item),
        )
        .route(
            "/api/v1/admin/plugins/:name/settings",
            get(modules::plugin::handler::get_settings)
                .put(modules::plugin::handler::update_settings),
        )
        .merge(state.plugin_manager.collect_routes());
```

- [ ] **Step 3: 运行编译检查**

```bash
cargo check -p inkforge
```

Expected: 编译通过。新增路由通过 `modules::plugin::handler::get_settings` 引用。

- [ ] **Step 4: Commit**

```bash
git add src/modules/plugin/mod.rs src/bootstrap/router.rs
git commit -m "feat: 注册 settings/handler 模块 + 添加 plugin settings API 路由"
```

---

## Task 6: HelloWorld 添加示例配置项

**Files:**
- Modify: `plugins/hello-world-a3f9b2c1/plugin.toml`

**目的:** 为 HelloWorld 插件添加声明式配置项和 admin 段，作为后续测试和 UI 开发的范本。

- [ ] **Step 1: 修改 plugin.toml**

在 `[hooks]` 段之后追加 `[admin]` 和 `[[settings]]`：

```toml
[admin]
enabled = true
entry = "settings.html"

[[settings]]
key = "greeting_target"
label = "问候对象"
type = "text"
default = "World"
description = "hello_world 模板函数的默认参数"

[[settings]]
key = "show_debug_info"
label = "显示调试信息"
type = "bool"
default = "false"
description = "是否在页面底部显示插件版本和调试信息"

[[settings]]
key = "max_entries"
label = "最大条目数"
type = "number"
default = "10"
description = "hello_world 列表最大展示条数"
```

修改后的完整 `plugins/hello-world-a3f9b2c1/plugin.toml`：

```toml
[plugin]
id = "hello-world-a3f9b2c1"
title = "Hello World"
version = "0.1.0"
description = "A demo plugin"
author = "InkForge Team"

[engine]
inkforge = ">=0.3.0"

[hooks]
template = true
routes = true
assets = ["css"]

[admin]
enabled = true
entry = "settings.html"

[[settings]]
key = "greeting_target"
label = "问候对象"
type = "text"
default = "World"
description = "hello_world 模板函数的默认参数"

[[settings]]
key = "show_debug_info"
label = "显示调试信息"
type = "bool"
default = "false"
description = "是否在页面底部显示插件版本和调试信息"

[[settings]]
key = "max_entries"
label = "最大条目数"
type = "number"
default = "10"
description = "hello_world 列表最大展示条数"
```

- [ ] **Step 2: 验证 manifest 解析**

```bash
cargo test -p inkforge plugin_manifest
```

Expected: 测试通过。`from_file` 能正确解析新增的 `[[settings]]` 数组和 `[admin]` 段。

- [ ] **Step 3: Commit**

```bash
git add plugins/hello-world-a3f9b2c1/plugin.toml
git commit -m "feat: HelloWorld 添加 greeting_target/show_debug_info/max_entries 配置项"
```

---

## Task 7: 创建 Admin UI 插件设置页

**Files:**
- Create: `src/admin/ui/src/pages/PluginSettings.tsx`

**目的:** 创建插件设置页面，从 URL 获取 plugin name，调用 GET API 获取 schema + 值，渲染声明式表单，保存调用 PUT。

- [ ] **Step 1: 创建 PluginSettings.tsx**

```tsx
import { useCallback, useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { apiData, API_PREFIX } from "../lib/api";
import { PageHeader } from "../components/PageHeader";
import { Button } from "../components/Button";
import { Input } from "../components/Input";
import { Select } from "../components/Select";
import { useToast } from "../contexts/ToastContext";

interface SettingOption {
  value: string;
  label: string;
}

interface SettingDef {
  key: string;
  label: string;
  setting_type: string;
  default?: string;
  description?: string;
  options?: SettingOption[];
}

interface PluginSettingsResponse {
  plugin_name: string;
  settings: SettingDef[];
  values: Record<string, string>;
}

const sectionStyle: React.CSSProperties = {
  background: "var(--md-surface-container-lowest)",
  borderRadius: "var(--radius-lg)",
  marginBottom: "20px",
};

const secHeadStyle: React.CSSProperties = {
  padding: "18px 24px",
  background: "var(--md-surface-container-low)",
};

const secTitleStyle: React.CSSProperties = {
  fontSize: "15px",
  fontWeight: 700,
  color: "var(--md-on-surface)",
  letterSpacing: "-0.2px",
};

const secDescStyle: React.CSSProperties = {
  fontSize: "12.5px",
  color: "var(--md-outline)",
  marginTop: "3px",
};

const secBodyStyle: React.CSSProperties = {
  padding: "24px",
  display: "flex",
  flexDirection: "column",
  gap: "18px",
};

const formRowStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "160px 1fr",
  gap: "12px",
  alignItems: "start",
};

const labelStyle: React.CSSProperties = {
  fontSize: "13.5px",
  fontWeight: 600,
  color: "var(--md-on-surface-variant)",
  paddingTop: "10px",
};

const hintStyle: React.CSSProperties = {
  fontSize: "12px",
  color: "var(--md-outline)",
  opacity: 0.8,
};

function SettingSection({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div style={sectionStyle}>
      <div style={secHeadStyle}>
        <h3 style={secTitleStyle}>{title}</h3>
        {description && <p style={secDescStyle}>{description}</p>}
      </div>
      <div style={secBodyStyle}>{children}</div>
    </div>
  );
}

function FormRow({
  label,
  children,
  hint,
}: {
  label: string;
  children: React.ReactNode;
  hint?: string;
}) {
  return (
    <div style={formRowStyle}>
      <span style={labelStyle}>{label}</span>
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "5px",
        }}
      >
        {children}
        {hint && <span style={hintStyle}>{hint}</span>}
      </div>
    </div>
  );
}

function renderField(
  def: SettingDef,
  value: string,
  onChange: (value: string) => void
) {
  switch (def.setting_type) {
    case "text":
      return (
        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={def.default || ""}
        />
      );
    case "textarea":
      return (
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={def.default || ""}
          rows={4}
          style={{
            width: "100%",
            padding: "10px 12px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--md-outline-variant)",
            background: "var(--md-surface)",
            color: "var(--md-on-surface)",
            fontSize: "13px",
            fontFamily: "inherit",
            resize: "vertical",
            outline: "none",
          }}
        />
      );
    case "bool":
      return (
        <Select
          value={value || "false"}
          onChange={(e) => onChange(e.target.value)}
        >
          <option value="true">是</option>
          <option value="false">否</option>
        </Select>
      );
    case "select":
      return (
        <Select
          value={value || def.default || ""}
          onChange={(e) => onChange(e.target.value)}
        >
          {(def.options || []).map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </Select>
      );
    case "number":
      return (
        <Input
          type="number"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={def.default || "0"}
        />
      );
    default:
      return (
        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={def.default || ""}
        />
      );
  }
}

export default function PluginSettings() {
  const { name } = useParams<{ name: string }>();
  const toast = useToast();
  const [data, setData] = useState<PluginSettingsResponse | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    try {
      setLoading(true);
      const resp = await apiData<PluginSettingsResponse>(
        `${API_PREFIX}/admin/plugins/${name}/settings`
      );
      setData(resp);
      setValues(resp.values);
    } catch (error) {
      toast(
        error instanceof Error ? error.message : "加载插件设置失败",
        "error"
      );
    } finally {
      setLoading(false);
    }
  }, [name, toast]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleSave() {
    setSaving(true);
    try {
      await apiData(`${API_PREFIX}/admin/plugins/${name}/settings`, {
        method: "PUT",
        body: JSON.stringify(values),
      });
      toast("设置已保存", "success");
      await load();
    } catch (error) {
      toast(
        error instanceof Error ? error.message : "保存设置失败",
        "error"
      );
    } finally {
      setSaving(false);
    }
  }

  function updateValue(key: string, value: string) {
    setValues((prev) => ({ ...prev, [key]: value }));
  }

  if (loading) {
    return (
      <div style={{ padding: "40px", textAlign: "center", color: "var(--md-outline)" }}>
        加载中...
      </div>
    );
  }

  if (!data) {
    return (
      <div style={{ padding: "40px", textAlign: "center", color: "var(--md-outline)" }}>
        插件未找到
      </div>
    );
  }

  return (
    <>
      <PageHeader
        title={`插件设置 — ${data.plugin_name}`}
        subtitle="管理此插件的配置项"
        actions={
          <Button onClick={handleSave} disabled={saving} loading={saving}>
            保存设置
          </Button>
        }
      />

      <SettingSection
        title="基础配置"
        description="插件声明的可配置项，修改后立即生效"
      >
        {data.settings.length === 0 && (
          <div
            style={{
              padding: "20px 0",
              textAlign: "center",
              color: "var(--md-outline)",
              fontSize: "13px",
            }}
          >
            此插件没有可配置项
          </div>
        )}
        {data.settings.map((def) => (
          <FormRow key={def.key} label={def.label} hint={def.description}>
            {renderField(def, values[def.key] || def.default || "", (val) =>
              updateValue(def.key, val)
            )}
          </FormRow>
        ))}
      </SettingSection>
    </>
  );
}
```

- [ ] **Step 2: 验证前端编译**

```bash
cd src/admin/ui && npm run build
```

Expected: 编译成功，无 TS 错误。

- [ ] **Step 3: Commit**

```bash
git add src/admin/ui/src/pages/PluginSettings.tsx
git commit -m "feat: 创建 Admin UI PluginSettings 页面 (GET/PUT settings API)"
```

---

## Task 8: 注册 UI 路由

**Files:**
- Modify: `src/admin/ui/src/App.tsx`

**目的:** 在 Admin 路由表中添加 `/plugins/:name/settings` 路由，并从 Sidebar 菜单可访问。

- [ ] **Step 1: 修改 App.tsx — 添加 import 和路由**

在文件顶部的 import 区域添加：

```tsx
import PluginSettings from './pages/PluginSettings';
```

在 `getActivePage` 函数中添加 `plugins` 匹配：

```tsx
function getActivePage(pathname: string): string {
  if (pathname.startsWith('/posts')) return 'posts';
  if (pathname.startsWith('/themes')) return 'themes';
  if (pathname.startsWith('/categories')) return 'categories';
  if (pathname.startsWith('/tags')) return 'tags';
  if (pathname.startsWith('/comments')) return 'comments';
  if (pathname.startsWith('/settings')) return 'settings';
  if (pathname.startsWith('/upload')) return 'upload';
  if (pathname.startsWith('/media-categories')) return 'media-categories';
  if (pathname.startsWith('/trash')) return 'trash';
  if (pathname.startsWith('/plugins')) return 'plugins';
  return 'posts';
}
```

在 `pageToRoute` 中添加：

```tsx
const pageToRoute: Record<string, string> = {
  posts: '/posts',
  categories: '/categories',
  tags: '/tags',
  comments: '/comments',
  settings: '/settings',
  upload: '/upload',
  'media-categories': '/media-categories',
  themes: '/themes',
  trash: '/trash',
  plugins: '/plugins',
};
```

在 `AdminGate` 的 `<Routes>` 内，`<Route path="trash" element={<RecycleBin />} />` 之后添加：

```tsx
          <Route path="plugins" element={<div style={{ padding: '40px', textAlign: 'center', color: 'var(--md-outline)' }}>插件管理</div>} />
          <Route path="plugins/:name/settings" element={<PluginSettings />} />
```

- [ ] **Step 2: 修改 Sidebar.tsx — 添加插件菜单项**

在 `src/admin/ui/src/components/Sidebar.tsx` 的 `navConfig` 配置中，`system` 分组的 items 数组末尾（`settings` 之前或之后）添加插件菜单项。

在 `{ key: 'settings', icon: IconSettings, labelKey: 'settings' },` 之前添加：

```tsx
      { key: 'plugins', icon: IconSettings, labelKey: 'plugins' },
```

> **注意:** `plugins` 菜单项的 `labelKey` 需在 i18n 翻译文件中添加。如果暂无 i18n 定义，可暂时用硬编码字符串 "插件"，或添加到 `src/admin/ui/src/i18n/` 下的 zh.ts / en.ts 中。

- [ ] **Step 3: 验证前端编译**

```bash
cd src/admin/ui && npm run build
```

Expected: 编译成功，`/admin/plugins/:name/settings` 路由可访问。

- [ ] **Step 4: Commit**

```bash
git add src/admin/ui/src/App.tsx src/admin/ui/src/components/Sidebar.tsx
git commit -m "feat: 注册 /admin/plugins/:name/settings 路由 + Sidebar 插件菜单项"
```

---

## Task 9: 编写测试

**Files:**
- Create: `src/tests/plugin_settings_tests.rs`
- Modify: `src/tests.rs`

**目的:** 覆盖 manifest 解析含 settings 字段、PluginSettings CRUD 操作、默认值回退。

- [ ] **Step 1: 创建 plugin_settings_tests.rs**

```rust
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::modules::plugin::manifest::PluginManifest;
    use crate::modules::plugin::settings;

    #[test]
    fn manifest_parses_settings_array() {
        let toml_str = r#"
            [plugin]
            id = "test-plugin"
            title = "Test"
            version = "0.1.0"

            [[settings]]
            key = "theme"
            label = "主题"
            type = "select"
            default = "light"
            options = [
                { value = "light", label = "浅色" },
                { value = "dark", label = "深色" },
            ]

            [[settings]]
            key = "api_key"
            label = "API Key"
            type = "text"
            description = "第三方 API 密钥"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let settings = manifest.settings.unwrap();
        assert_eq!(settings.len(), 2);

        let theme = &settings[0];
        assert_eq!(theme.key, "theme");
        assert_eq!(theme.label, "主题");
        assert_eq!(theme.setting_type, "select");
        assert_eq!(theme.default.as_deref(), Some("light"));
        let opts = theme.options.as_ref().unwrap();
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].value, "light");
        assert_eq!(opts[1].value, "dark");

        let api_key = &settings[1];
        assert_eq!(api_key.key, "api_key");
        assert_eq!(api_key.setting_type, "text");
        assert_eq!(api_key.default, None);
    }

    #[test]
    fn manifest_parses_admin_section() {
        let toml_str = r#"
            [plugin]
            id = "test-admin"
            title = "Admin Test"
            version = "0.1.0"

            [admin]
            enabled = true
            entry = "settings.html"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let admin = manifest.admin.unwrap();
        assert_eq!(admin.enabled, Some(true));
        assert_eq!(admin.entry.as_deref(), Some("settings.html"));
    }

    #[test]
    fn manifest_parses_resources_section() {
        let toml_str = r#"
            [plugin]
            id = "test-resources"
            title = "Resources Test"
            version = "0.1.0"

            [resources]
            admin_root = "admin/"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let res = manifest.resources.unwrap();
        assert_eq!(res.admin_root.as_deref(), Some("admin/"));
    }

    #[test]
    fn manifest_parses_slots() {
        let toml_str = r#"
            [plugin]
            id = "test-slots"
            title = "Slots Test"
            version = "0.1.0"

            [[slots]]
            target = "dashboard.widget"
            label = "统计面板"
            entry = "widget.html"
            width = 400
            height = 300
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let slots = manifest.slots.unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].target, "dashboard.widget");
        assert_eq!(slots[0].entry, "widget.html");
        assert_eq!(slots[0].width, Some(400));
        assert_eq!(slots[0].height, Some(300));
    }

    #[test]
    fn manifest_without_optional_sections_parses() {
        let toml_str = r#"
            [plugin]
            id = "minimal"
            title = "Minimal"
            version = "0.1.0"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert!(manifest.settings.is_none());
        assert!(manifest.admin.is_none());
        assert!(manifest.resources.is_none());
        assert!(manifest.slots.is_none());
    }

    #[test]
    fn merge_with_defaults_uses_stored_value_over_default() {
        let defs = vec![
            crate::modules::plugin::manifest::SettingDef {
                key: "theme".into(),
                label: "主题".into(),
                setting_type: "select".into(),
                default: Some("light".into()),
                description: None,
                options: None,
            },
            crate::modules::plugin::manifest::SettingDef {
                key: "api_key".into(),
                label: "API Key".into(),
                setting_type: "text".into(),
                default: None,
                description: None,
                options: None,
            },
        ];

        let values = HashMap::from([("theme".to_string(), "dark".to_string())]);
        let merged = settings::merge_with_defaults(&defs, &values);
        assert_eq!(merged.get("theme").unwrap(), "dark");
        assert_eq!(merged.get("api_key").unwrap(), "");
    }

    #[test]
    fn merge_with_defaults_falls_back_to_default() {
        let defs = vec![crate::modules::plugin::manifest::SettingDef {
            key: "max".into(),
            label: "Max".into(),
            setting_type: "number".into(),
            default: Some("10".into()),
            description: None,
            options: None,
        }];

        let values = HashMap::new();
        let merged = settings::merge_with_defaults(&defs, &values);
        assert_eq!(merged.get("max").unwrap(), "10");
    }

    #[test]
    fn merge_with_defaults_no_default_returns_empty() {
        let defs = vec![crate::modules::plugin::manifest::SettingDef {
            key: "name".into(),
            label: "Name".into(),
            setting_type: "text".into(),
            default: None,
            description: None,
            options: None,
        }];

        let values = HashMap::new();
        let merged = settings::merge_with_defaults(&defs, &values);
        assert_eq!(merged.get("name").unwrap(), "");
    }
}
```

- [ ] **Step 2: 更新 src/tests.rs — 注册测试模块**

在 `src/tests.rs` 的模块注册区域末尾添加：

```rust
mod plugin_settings_tests;
```

修改后的 `src/tests.rs` 末尾：

```rust
mod plugin_manager_tests;
mod plugin_manifest_tests;
mod plugin_id_strategy_tests;
mod plugin_hook_tests;
mod plugin_settings_tests;
```

- [ ] **Step 3: 运行测试**

```bash
cargo test -p inkforge plugin_settings -- --nocapture
```

Expected: 8 个测试全部通过。

- [ ] **Step 4: Commit**

```bash
git add src/tests/plugin_settings_tests.rs src/tests.rs
git commit -m "test: 添加 manifest settings/admin/resources/slots 解析 + merge_with_defaults 测试"
```

---

## Task 10: 全量验证 + 文档更新

**Files:**
- 运行 `cargo test -p inkforge`
- 运行 `cargo clippy -p inkforge`
- 运行 `cd src/admin/ui && npm run build`
- 更新 `memories/PROJECT_STATUS.md` 记录 P1b 完成

- [ ] **Step 1: 运行完整测试套件**

```bash
cargo test -p inkforge
```

Expected: 所有测试通过（含新增的 8 个 `plugin_settings_tests` 和之前所有测试）。

- [ ] **Step 2: 运行 clippy**

```bash
cargo clippy -p inkforge -- -D warnings
```

Expected: 无 warning。

- [ ] **Step 3: 验证前端编译**

```bash
cd src/admin/ui && npm run build
```

Expected: 编译成功。`PluginSettings` 页面已构建到 admin dist。

- [ ] **Step 4: 更新 PROJECT_STATUS.md**

在 `memories/PROJECT_STATUS.md` 的近期优先级或插件章节中标记 P1b 配置面板完成，添加简要说明：声明式配置（text/textarea/bool/select/number）、`plugin_settings` 表 CRUD、GET/PUT API、Admin UI PluginSettings 页面、HelloWorld 示例配置项已就绪。

- [ ] **Step 5: Commit**

```bash
git add memories/PROJECT_STATUS.md
git commit -m "docs: 标记 P1b 配置面板完成"
```

---

## 验证清单

| 检查项 | 命令 | 期望 |
|--------|------|------|
| 编译通过 | `cargo check -p inkforge` | 0 errors |
| 测试全绿 | `cargo test -p inkforge` | All passed |
| Clippy 无警告 | `cargo clippy -p inkforge -- -D warnings` | 0 warnings |
| 前端编译通过 | `cd src/admin/ui && npm run build` | 0 errors |
| 迁移文件存在 | `Test-Path migrations/015_plugin_settings.sql` | True |
| manifest.rs 含 SettingDef | `rg "SettingDef" src/modules/plugin/manifest.rs` | >2 处 |
| settings.rs 存在 | `Test-Path src/modules/plugin/settings.rs` | True |
| handler.rs 存在 | `Test-Path src/modules/plugin/handler.rs` | True |
| mod.rs 注册 settings/handler | `rg "pub mod (settings|handler)" src/modules/plugin/mod.rs` | 2 处 |
| router.rs 含 plugins settings 路由 | `rg "plugins.*settings" src/bootstrap/router.rs` | 1 处 |
| PluginManager 含 get_manifest | `rg "get_manifest" src/modules/plugin/manager.rs` | 1 处 |
| PluginSettings.tsx 存在 | `Test-Path src/admin/ui/src/pages/PluginSettings.tsx` | True |
| App.tsx 含 PluginSettings 路由 | `rg "PluginSettings" src/admin/ui/src/App.tsx` | >1 处 |
| HelloWorld plugin.toml 含 settings | `rg "\[\[settings\]\]" plugins/hello-world-a3f9b2c1/plugin.toml` | 1 处 |
| plugin.toml 含 greeting_target | `rg "greeting_target" plugins/hello-world-a3f9b2c1/plugin.toml` | 1 处 |
| 测试模块已注册 | `rg "plugin_settings_tests" src/tests.rs` | 1 处 |
| 测试通过 | `cargo test -p inkforge plugin_settings` | 8 passed |
