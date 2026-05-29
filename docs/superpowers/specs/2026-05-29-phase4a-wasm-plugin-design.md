> **注意：此 WASM 方案经审查后被否决（wasmtime Store 线程安全问题 + WASI 沙箱缺失 + 无 SDK）。实际实施采用 Rust trait 方案（见 plans/2026-05-29-phase4a-manifest-discovery.md）。**

# Phase 4a — WASM 插件系统设计

**日期:** 2026-05-29
**状态:** 已确认
**依赖:** Phase 3 插件系统（Plugin trait 将被 WASM 接口替代）

---

## 目标

将插件系统从"编译期 Rust trait"迁移到"运行时 WASM 模块"：
- `plugin.toml` 声明式 manifest
- 文件系统自动发现
- 启用/禁用持久化状态
- 版本兼容性检查
- 热加载：增删插件无需 `cargo build`
- 故障隔离：插件 panic 不影响宿主

## 架构概览

```
plugins/
  hello-world-a3f9b2c1/
    plugin.toml        ← manifest（id 必须等于目录名）
    plugin.wasm        ← 编译好的 WASM 模块
    static/            ← 插件静态资源
      hello.css

启动流程:
  PluginLoader::scan("plugins/")
    → 遍历目录 → 解析 plugin.toml → 校验 id == 目录名
    → VersionChecker::check(inkforge >= requires)
    → PluginStatusStore::get_enabled_ids() 只加载已启用
    → WasmPluginRuntime::load(plugin.wasm) → wasmtime 实例化
    → PluginManager 管理全部活跃 WASM 实例
```

## 核心模块

### PluginManifest（src/modules/plugin/manifest.rs）

`plugin.toml` 结构：

```toml
[plugin]
id = "hello-world-a3f9b2c1"    # 唯一标识，由 CLI 生成，必须等于目录名
title = "Hello World"
version = "0.1.0"
description = "A demo plugin"
author = "InkForge Team"

[engine]
inkforge = ">=0.3.0"            # 宿主最低版本（SemVer range）
wasm_target = "wasm32-wasip1"

[hooks]
template = true                 # 是否注入模板函数/过滤器
routes = true                   # 是否注册 API 路由
assets = ["css"]                # 声明静态资产类型
```

解析规则：
- `id` 必须等于目录名，否则拒绝加载
- `hooks` 声明了的能力，宿主才初始化对应 bridge
- `[engine].inkforge` 使用 semver crate 解析，满足 range 才加载

### PluginIdStrategy（src/modules/plugin/id_strategy.rs）

可替换的 ID 生成/校验 trait：

```rust
pub trait PluginIdStrategy: Send + Sync {
    fn generate(name: &str) -> String;
    fn validate(id: &str) -> bool;
}

pub struct ShortHashIdStrategy;
impl PluginIdStrategy for ShortHashIdStrategy {
    fn generate(name: &str) -> String {
        // {name}-{base64url(sha256(name+timestamp))[..8]}
    }
    fn validate(id: &str) -> bool {
        id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && id.len() <= 64
    }
}
```

未来替换方案：实现 `MarketplaceIdStrategy`（`author.name`）、`TimestampIdStrategy` 等，改一行配置。

### PluginLoader（src/modules/plugin/loader.rs）

扫描 `plugins/` 目录，为每个子目录：
1. 读取 `plugin.toml` 并解析
2. 校验 `id == 目录名`
3. 解析 `[engine].inkforge` 版本约束，与当前版本比较
4. 查询 `PluginStatusStore` 是否已启用
5. 找到 `plugin.wasm` 文件

返回：`Vec<DiscoveredPlugin>`（包含 manifest + wasm 路径 + 状态）

### PluginStatusStore（src/modules/plugin/status.rs）

DB 表：

```sql
CREATE TABLE plugins (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    version TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    installed_at TEXT NOT NULL,
    error_message TEXT
);
```

提供：
- `get_enabled_ids()` → Vec<String>
- `set_enabled(id, enabled)`
- `set_error(id, msg)`

### WasmPluginRuntime（src/modules/plugin/wasm_runtime.rs）

使用 `wasmtime` crate：

```rust
pub struct WasmPluginInstance {
    manifest: PluginManifest,
    store: wasmtime::Store<PluginContext>,
    instance: wasmtime::Instance,
}

impl WasmPluginInstance {
    pub fn load(wasm_path: &Path, manifest: PluginManifest) -> Result<Self>;
    pub fn call_init(&mut self) -> Result<serde_json::Value>;
    pub fn call_shutdown(&mut self) -> Result<serde_json::Value>;
    pub fn call_api_routes(&mut self) -> Result<Vec<RouteDef>>;
    pub fn call_extend_template_env(&mut self, ctx_json: &str) -> Result<Vec<FnDef>>;
    pub fn call_assets(&mut self) -> Result<Vec<AssetDef>>;
}
```

WASM 导出函数（JSON 协议）：
- `plugin_init()` → `{"ok": true}`
- `plugin_shutdown()` → `{"ok": true}`
- `plugin_api_routes()` → `[{"method":"GET","path":"/api/v1/plugins/hello"}]`
- `plugin_extend_template_env(ctx_json)` → `[{"name":"hello_world","kind":"function","params":["name"]}]`
- `plugin_assets()` → `[{"kind":"css","path":"hello.css","placement":"head"}]`

WASM 导入函数（宿主暴露给插件）：
- `env_log(msg_ptr, msg_len)` — 插件调宿主日志

未来按需扩展 `env_get_setting`、`env_query` 等。

### 集成改造

**PluginManager** 重构：
- 移除 `Vec<Box<dyn Plugin>>`，改为 `Vec<WasmPluginInstance>`
- `init_all()` → 依次调用 `instance.call_init()`
- `collect_routes()` → 依次调用 `instance.call_api_routes()`
- `extend_template_env()` → 依次调用 `instance.call_extend_template_env()`
- `collect_assets()` → 依次调用 `instance.call_assets()`

**AppState** 不变，`plugin_manager: Arc<PluginManager>` 字段保持。

**lib.rs** 启动改动：
- 移除硬编码的 `registry::register(HelloWorldPlugin)`
- 改为 `PluginLoader::scan("plugins/")` → 加载所有 WASM 插件

**HelloWorld 改造**：
- 从 `src/plugins/hello_world.rs` 移到 `plugins/hello-world-a3f9b2c1/`
- 新建 `Cargo.toml`：`crate-type = ["cdylib"]`, target = `wasm32-wasip1`
- 重新实现为 WASM 导出函数（JSON 接口）

**Plugin trait 废弃**：
- 保留 `src/modules/plugin/mod.rs` 中的 `Plugin` trait（其他模块引用）
- 不再有 `registry.rs`（静态注册表废弃）
- `PluginManager` 内部逻辑从"迭代 trait 对象"改为"迭代 WASM 实例"

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| wasmtime 编译增量 ~200KB | 接受 |
| 模板函数 bridge 性能开销 | 每次调用 1-2ms JSON 序列化，可接受 |
| HelloWorld 重构工作量 | 逻辑极简，10 分钟移植 |
| 现有测试失效（PluginManager mock） | 重写 mock：用 Rust 模拟 WASM 行为 |
| 无插件时 PluginManager 为空 | 保持路由/模板注入兼容（空操作） |

## 验证

- `cargo test -p inkforge` — 全量测试通过（mock WASM 插件）
- `cargo run` — 启动时扫描 `plugins/` 目录并加载 HelloWorld
- `curl http://127.0.0.1:2000/api/v1/plugins/hello` — 返回 `{"status":"ok"}`
- 首页模板渲染 `{{ hello_world() }}` — 返回 `"Hello, World!"`
- 首页 HTML 含 `<link rel="stylesheet" href="/static/plugins/hello-world-a3f9b2c1/hello.css">`
