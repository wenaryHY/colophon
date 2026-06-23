# Colophon Wasm 沙箱故障排查手册

## 1. wasmtime 缓存目录错误

### 现象

日志中出现：
```
Wasm 运行时错误: failed to create cache directory: /var/lib/inkforge/.cache/wasmtime
```

### 根因

extism 内嵌的 wasmtime 引擎**不认** `WASMTIME_CACHE_PATH`、`WASMTIME_CACHE_DISABLE` 等环境变量。
它只根据 `$HOME/.cache/wasmtime` 推导缓存路径。

colophon 用户的 passwd HOME=`/var/lib/inkforge`，但该目录不存在且 `ProtectHome=true` 阻止访问。

### 修复

systemd 服务配置中覆盖 HOME 到一个已有可写目录：

```ini
# deploy/colophon.service
Environment=HOME=/var/lib/colophon
```

同时创建缓存目录（可选手动或在 install.sh 中）：

```bash
mkdir -p /var/lib/colophon/.cache/wasmtime
chown colophon:colophon /var/lib/colophon/.cache
```

### 版本信息

- extism: 1.30（wasmtime 运行时）
- 系统: Debian 13, systemd

---

## 2. Wasm Hook 未修改数据（JSON ABI 嵌套层级）

### 现象

Hook 执行成功（日志显示 `filter hook executed successfully`），但业务数据未被修改。
例如标题没有追加 `[Wasm Validated]`。

### 根因

宿主侧 `HookData` 是 Rust enum（外部 tagged 序列化）：

```json
{"PostBeforeSave": {"title": "...", "content_html": "..."}}
```

修复前，这个完整结构被直接发给 Wasm 插件。插件在顶层 `data.get("title")` 查找，但 title 实际在 `data.PostBeforeSave.title`。

### 修复

在 `src/modules/plugin/sandbox.rs` 中新增两个辅助函数，在 ABI 边界做 flatten/re-wrap：

```rust
// 发送前：剥掉 enum 包装，只传内部 struct 的 JSON
fn hook_data_to_value(data: &HookData) -> Result<serde_json::Value, AppError> {
    match data {
        HookData::PostBeforeSave(d) => serde_json::to_value(d),
        // ... 其他 variant
    }
}

// 接收后：根据 hook_name 重新装回正确的 enum variant
fn value_to_hook_data(hook_name: &str, value: serde_json::Value) -> Result<HookData, AppError> {
    match hook_name {
        "post.before_save" => HookData::PostBeforeSave(serde_json::from_value(value)?),
        // ... 其他 hook
    }
}
```

### 设计原则

ABI 边界两侧不应共享类型系统。宿主侧抽象掉序列化格式，插件侧只看到纯业务数据。

---

## 3. Hook 执行静默失败（缺乏可观测性）

### 现象

请求返回 200，但不确定 Wasm Hook 是否真的执行了。

### 修复位置

`src/modules/plugin/hook_registry.rs` 的三个 dispatch 函数：

```rust
// dispatch_filter / dispatch_action 入口：记录找到了几个 hook
tracing::info!("dispatch_filter: found {count} hooks for event");

// handler 执行后：记录成功或失败
tracing::info!("filter hook executed successfully");
// 或
tracing::error!("filter hook failed");
```

### 排查命令

```bash
# 查看 hook 是否被找到和执行
journalctl -u colophon --since "5 minutes ago" -o cat | grep "dispatch_filter\|filter hook"
```

---

## 4. Wasm 超时与内存防御配置

### Manifest 构建

```rust
// src/modules/plugin/sandbox.rs
Manifest::new([wasm])
    .with_allowed_hosts(std::iter::empty())  // 禁止网络
    .with_allowed_paths(std::iter::empty())  // 禁止文件系统
    .with_timeout(Duration::from_secs(5))    // Fuel/epoch 超时
    .with_memory_max(160);                   // 160 页 × 64KB = 10MB
```

**注意**: `with_memory_max` 接受的是 **Wasm 页数**（每页 64KB），不是字节数。

### 返回值大小限制

```rust
// sandbox.rs: plugin.call() 之后、serde_json::from_str() 之前
const MAX_WASM_OUTPUT_BYTES: usize = 1 * 1024 * 1024; // 1MB
if output.len() > MAX_WASM_OUTPUT_BYTES {
    return Err(PluginError::SerializationError("返回值过大"));
}
```

### 死循环守护（三层纵深）

| 层级 | 机制 | 防什么 |
|---|---|---|
| extism `with_timeout(5s)` | Wasm 引擎 Fuel/epoch 掐断 | 死循环 |
| extism `with_memory_max(160)` | 限制线性内存 | 恶意分配 |
| 应用层 `MAX_WASM_OUTPUT_BYTES` | 返回值长度守卫 | OOM payload |
| tokio `timeout(5s)` | 外部掐断 spawn_blocking | 沙箱失败兜底 |

---

## 5. 开发调试：给 Wasm Hook 注入跟踪日志

### 在 dispatch_filter 中加临时日志

```rust
// hook_registry.rs
let hooks = { ... };
tracing::info!(found = hooks.len(), "dispatch_filter found hooks");
for hook in &hooks {
    let result = hook.handler.run(ctx).await;
    tracing::info!(plugin = %hook.plugin_name, success = result.is_ok());
    result?;
}
```

### 在 WasmHookHandler::run() 中加输出日志

```rust
// sandbox.rs
let (json_output, took_ms) = ...;
tracing::info!(
    raw_output_preview = %json_output.chars().take(200).collect::<String>(),
    "wasm raw response"
);
```

---

## 6. 部署清单

每次涉及 Wasm 的部署后，确认以下三项：

```bash
# 1. 编译和缓存
journalctl -u colophon --since "30 seconds ago" | grep "wasm module compiled"

# 2. Hook 注册
journalctl -u colophon --since "30 seconds ago" | grep "wasm hooks registered"

# 3. 无缓存错误
journalctl -u colophon --since "30 seconds ago" | grep -i "inkforge\|cache.*fail" | wc -l
# 预期：0
```
