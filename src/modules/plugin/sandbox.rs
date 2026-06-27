use extism::{Manifest, Plugin, Wasm};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;

use super::hook::{HookContext, HookData, HookHandler};
use crate::shared::error::{AppError, AppResult};

/// Wasm 沙箱错误类型
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Wasm 编译失败: {0}")]
    WasmCompileError(String),

    #[error("Wasm 运行时错误: {0}")]
    RuntimeError(String),

    #[error("序列化/反序列化失败: {0}")]
    SerializationError(String),

    #[error("Hook 执行超时")]
    TimeoutError,
}

/// 宿主 -> Wasm 的请求
#[derive(Serialize)]
struct HookRequest {
    hook_name: String,
    data: serde_json::Value,
}

/// Wasm -> 宿主 的响应（Filter 必须返回修改后的数据，Action 返回 null）
#[derive(Deserialize)]
struct HookResponse {
    #[serde(default)]
    modified_data: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
}

/// 管理所有已编译的 Wasm Manifest（线程安全，可跨 tokio 任务共享）
pub struct WasmRuntime {
    pub manifests: HashMap<String, Manifest>,
}

impl WasmRuntime {
    pub fn new() -> Self {
        Self {
            manifests: HashMap::new(),
        }
    }

    /// 从 .wasm 文件编译 Manifest 并缓存
    pub fn load_module(&mut self, plugin_id: &str, wasm_path: &Path) -> Result<(), PluginError> {
        // Wasm::file 是 infallible 的（仅构造描述符，编译在 Plugin 创建时发生）
        let wasm = Wasm::file(wasm_path);
        let manifest = Manifest::new([wasm])
            .with_allowed_hosts(std::iter::empty()) // 禁止网络
            .with_allowed_paths(std::iter::empty()) // 禁止文件系统
            .with_timeout(Duration::from_secs(5)) // Wasm 引擎内掐断死循环 (Fuel/epoch)
            // with_memory_max 参数为 Wasm 页数，每页 64KB
            // 160 页 = 10MB，限制 Wasm 线性内存防止恶意内存分配
            .with_memory_max(160);
        self.manifests.insert(plugin_id.to_string(), manifest);
        Ok(())
    }

    /// 检查某个插件是否已加载
    pub fn has_module(&self, plugin_id: &str) -> bool {
        self.manifests.contains_key(plugin_id)
    }
}

/// Wasm 插件 Hook 处理器 — 实现 HookHandler trait，桥接 Wasm 调用
pub struct WasmHookHandler {
    pub plugin_id: String,
    pub wasm_runtime: Arc<RwLock<WasmRuntime>>,
}

/// Wasm hook 调用的最大允许执行时间。
///
/// 双重防御：
/// 1. extism Manifest 层的 `with_timeout(Duration::from_secs(5))` 在 Wasm 引擎
///    内部掐断死循环（Fuel/epoch 机制）
/// 2. tokio 层的 `timeout()` 在外部掐断 spawn_blocking 等待
const WASM_HOOK_TIMEOUT_SECS: u64 = 5;

#[async_trait::async_trait]
impl HookHandler for WasmHookHandler {
    async fn run(&self, ctx: &mut HookContext) -> AppResult<()> {
        // 1. 序列化 HookContext 为 JSON
        // Flatten HookData: 只发送内部数据（去掉 Rust enum 包装），
        // 插件不需要知道宿主的序列化格式。
        let inner_data = hook_data_to_value(&ctx.data)?;
        let request = HookRequest {
            hook_name: ctx.hook_name.clone(),
            data: inner_data,
        };
        let json_input = serde_json::to_string(&request)
            .map_err(|e| AppError::Internal(format!("Wasm 请求序列化失败: {e}")))?;

        // 2. 通过 spawn_blocking 调用 Wasm（extism::Plugin::call 是同步的）
        let runtime = self.wasm_runtime.clone();
        let plugin_id = self.plugin_id.clone();

        let blocking_task = tokio::task::spawn_blocking(move || {
            // 读取锁获取 Manifest 引用后立即克隆，释放锁
            let manifest =
                {
                    let guard = runtime.blocking_read();
                    guard.manifests.get(&plugin_id).cloned().ok_or_else(|| {
                        PluginError::RuntimeError("plugin manifest not found".into())
                    })?
                };

            let mut plugin = Plugin::new(&manifest, [], true)
                .map_err(|e| PluginError::RuntimeError(e.to_string()))?;

            let start = std::time::Instant::now();
            let result: Result<String, extism::Error> = plugin.call("handle_hook", &json_input);
            let took_ms = start.elapsed().as_millis();

            // RAII: 显式释放 Plugin，确保 Wasm 线性内存正确回收
            drop(plugin);

            let output = result.map_err(|e| PluginError::RuntimeError(e.to_string()))?;

            // 纵深防御：拒绝超大返回值，防止 serde_json 解析时 OOM
            const MAX_WASM_OUTPUT_BYTES: usize = 1 * 1024 * 1024; // 1MB
            if output.len() > MAX_WASM_OUTPUT_BYTES {
                return Err(PluginError::SerializationError(format!(
                    "Wasm 返回值过大: {} bytes (上限 {})",
                    output.len(),
                    MAX_WASM_OUTPUT_BYTES
                )));
            }

            Ok((output, took_ms))
        });

        // 3. 带超时的 await
        let spawn_result =
            timeout(Duration::from_secs(WASM_HOOK_TIMEOUT_SECS), blocking_task).await;

        let (json_output, took_ms) = match spawn_result {
            Ok(Ok(Ok(value))) => value,
            Ok(Ok(Err(e))) => {
                tracing::error!(
                    module = "wasm",
                    plugin = %self.plugin_id,
                    hook = %ctx.hook_name,
                    error = %e,
                    "wasm execution failed"
                );
                return Err(AppError::Internal(format!("Wasm 插件错误: {e}")));
            }
            Ok(Err(join_err)) => {
                tracing::error!(
                    module = "wasm",
                    plugin = %self.plugin_id,
                    hook = %ctx.hook_name,
                    error = %join_err,
                    "wasm spawn_blocking join error"
                );
                return Err(AppError::Internal(format!("Wasm 调用线程异常: {join_err}")));
            }
            Err(_elapsed) => {
                tracing::warn!(
                    module = "wasm",
                    plugin = %self.plugin_id,
                    hook = %ctx.hook_name,
                    timeout_secs = WASM_HOOK_TIMEOUT_SECS,
                    "wasm hook timed out"
                );
                return Err(AppError::Internal(format!(
                    "Wasm hook '{0}' 执行超时（超过 {1} 秒）",
                    ctx.hook_name, WASM_HOOK_TIMEOUT_SECS
                )));
            }
        };

        tracing::debug!(
            module = "wasm",
            plugin = %self.plugin_id,
            hook = %ctx.hook_name,
            took_ms = took_ms,
            "wasm hook executed"
        );

        // 4. 反序列化响应
        let response: HookResponse = serde_json::from_str(&json_output).map_err(|e| {
            tracing::error!(
                module = "wasm",
                plugin = %self.plugin_id,
                error = %e,
                raw_output = %json_output,
                "wasm response deserialization failed"
            );
            AppError::Internal(format!("Wasm 返回值反序列化失败: {e}"))
        })?;

        // 5. 检查 wasm 返回的错误
        if let Some(ref err) = response.error {
            tracing::warn!(
                module = "wasm",
                plugin = %self.plugin_id,
                hook = %ctx.hook_name,
                error = %err,
                "wasm plugin returned error"
            );
            return Err(AppError::Internal(format!("Wasm 插件错误: {err}")));
        }

        // 6. 如果有 modified_data，重新包装为 HookData enum 并更新 ctx
        if let Some(modified) = response.modified_data {
            ctx.data = value_to_hook_data(&ctx.hook_name, modified)
                .map_err(|e| AppError::Internal(format!("修改后的数据反序列化失败: {e}")))?;
        }

        Ok(())
    }
}

/// 将 HookData 转换为扁平的 serde_json::Value（去掉外部 tagged enum 包装）
fn hook_data_to_value(data: &HookData) -> Result<serde_json::Value, AppError> {
    use super::hook::*;
    let value = match data {
        HookData::PostBeforeSave(d) => serde_json::to_value(d),
        HookData::PostAfterSave(d) => serde_json::to_value(d),
        HookData::PostAfterPublish(d) => serde_json::to_value(d),
        HookData::PostBeforeRender(d) => serde_json::to_value(d),
        HookData::CommentBeforeCreate(d) => serde_json::to_value(d),
    };
    value.map_err(|e| AppError::Internal(format!("HookData 序列化失败: {e}")))
}

/// 将扁平的 serde_json::Value 重新包装为 HookData enum
fn value_to_hook_data(hook_name: &str, value: serde_json::Value) -> Result<HookData, AppError> {
    use super::hook::*;
    let data = match hook_name {
        "post.before_save" => HookData::PostBeforeSave(
            serde_json::from_value(value)
                .map_err(|e| AppError::Internal(format!("PostBeforeSaveData 反序列化: {e}")))?,
        ),
        "post.after_save" => HookData::PostAfterSave(
            serde_json::from_value(value)
                .map_err(|e| AppError::Internal(format!("PostAfterSaveData 反序列化: {e}")))?,
        ),
        "post.after_publish" => HookData::PostAfterPublish(
            serde_json::from_value(value)
                .map_err(|e| AppError::Internal(format!("PostAfterPublishData 反序列化: {e}")))?,
        ),
        "post.before_render" => HookData::PostBeforeRender(
            serde_json::from_value(value)
                .map_err(|e| AppError::Internal(format!("PostBeforeRenderData 反序列化: {e}")))?,
        ),
        "comment.before_create" => {
            HookData::CommentBeforeCreate(serde_json::from_value(value).map_err(|e| {
                AppError::Internal(format!("CommentBeforeCreateData 反序列化: {e}"))
            })?)
        }
        _ => {
            return Err(AppError::Internal(format!(
                "unknown hook for re-wrap: {hook_name}"
            )))
        }
    };
    Ok(data)
}
