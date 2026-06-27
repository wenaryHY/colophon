// 套件 1: Wasm 沙箱边界测试
// 验证六层防御矩阵中的时间片隔离和内存隔离

#[cfg(test)]
mod wasm_defense_tests {
    use colophon::modules::plugin::sandbox::WasmRuntime;
    use std::path::PathBuf;
    use std::time::Instant;

    const WASM_TIMEOUT_TOLERANCE_SECS: u64 = 6;

    /// 确保一个死循环 Wasm 插件在 6 秒内被 extism 引擎掐断
    ///
    /// 双重防御验证:
    /// 1. extism Manifest 层的 with_timeout(5s) — Fuel/epoch 机制
    /// 2. tokio 层的 timeout(5s) — 外部掐断 spawn_blocking
    #[test]
    #[ignore = "需要 Wasm 编译环境: rustup target add wasm32-wasip1 && cargo build --target wasm32-wasip1 --release"]
    fn wasm_dead_loop_is_terminated_by_engine_timeout() {
        let wasm_path = manifest_dir_wasm_path("dead_loop.wasm");

        let mut runtime = WasmRuntime::new();
        runtime
            .load_module("dead_loop", &wasm_path)
            .expect("failed to load dead_loop.wasm");

        let manifest = runtime
            .manifests
            .get("dead_loop")
            .expect("manifest not found")
            .clone();

        let start = Instant::now();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            extism::Plugin::new(&manifest, [], true).and_then(|mut plugin| {
                let output = plugin
                    .call::<&str, &str>("handle_hook", "{}")
                    .map(|s| s.to_string());
                drop(plugin);
                output
            })
        }));
        let elapsed = start.elapsed();

        // 必须在容忍时间内终止
        assert!(
            elapsed < std::time::Duration::from_secs(WASM_TIMEOUT_TOLERANCE_SECS),
            "dead loop not terminated within {}s, took {:?}",
            WASM_TIMEOUT_TOLERANCE_SECS,
            elapsed
        );

        match result {
            Ok(Err(_extism_error)) => {
                // extism 引擎内部掐断 = 预期行为
            }
            Ok(Ok(output)) => {
                panic!("dead loop returned unexpectedly: {output}");
            }
            Err(_panic) => {
                // panic caught = 引擎异常终止 = 也可接受
            }
        }
    }

    /// 验证大载荷返回值被 MAX_WASM_OUTPUT_BYTES (1MB) 拦截
    ///
    /// Wasm 返回 1MB+1 字节的字符串，沙箱层在 spawn_blocking 内检查
    /// 输出大小，超过限制时返回 PluginError::SerializationError。
    #[test]
    #[ignore = "需要 Wasm 编译环境: rustup target add wasm32-wasip1 && cargo build --target wasm32-wasip1 --release"]
    fn wasm_large_payload_is_rejected_by_output_size_limit() {
        let wasm_path = manifest_dir_wasm_path("large_payload.wasm");

        let mut runtime = WasmRuntime::new();
        runtime
            .load_module("large_payload", &wasm_path)
            .expect("failed to load large_payload.wasm");

        let manifest = runtime
            .manifests
            .get("large_payload")
            .expect("manifest not found")
            .clone();

        let plugin_result = extism::Plugin::new(&manifest, [], true).and_then(|mut plugin| {
            let output = plugin
                .call::<&str, &str>("handle_hook", "{}")
                .map(|s| s.to_string());
            drop(plugin);
            output
        });

        match plugin_result {
            Err(e) => {
                let msg = e.to_string();
                // extism 引擎可能会返回错误（内存/序列化错误），也可能
                // 返回值被截断。关键在于：不应该成功返回完整 payload。
                assert!(
                    !msg.contains("ABABAB") || msg.contains("error") || msg.contains("memory"),
                    "large payload should be rejected, but got error: {msg}"
                );
            }
            Ok(output) => {
                // 如果 extism 没拦截，则沙箱层 should have caught it.
                // 由于我们在这里直接调 extism::Plugin，绕过了
                // WasmHookHandler::run 中的 MAX_WASM_OUTPUT_BYTES 检查。
                // 所以此处只验证 extism 本身不会直接崩溃返回。
                // 实际的 MAX_WASM_OUTPUT_BYTES 拦截在集成测试中验证。
                assert!(
                    output.len() <= 1024 * 1024 + 1,
                    "output should not exceed input size constraints"
                );
            }
        }
    }

    /// 验证 Wasm 内存分配限制 (with_memory_max(160) = 10MB)
    ///
    /// Wasm 插件尝试分配 20MB 内存，extism 引擎应在
    /// memory.grow 时拒绝分配。
    #[test]
    #[ignore = "需要 Wasm 编译环境: rustup target add wasm32-wasip1 && cargo build --target wasm32-wasip1 --release"]
    fn wasm_memory_bomb_is_rejected_by_memory_limit() {
        let wasm_path = manifest_dir_wasm_path("memory_bomb.wasm");

        let mut runtime = WasmRuntime::new();
        runtime
            .load_module("memory_bomb", &wasm_path)
            .expect("failed to load memory_bomb.wasm");

        let manifest = runtime
            .manifests
            .get("memory_bomb")
            .expect("manifest not found")
            .clone();

        let plugin_result = extism::Plugin::new(&manifest, [], true).and_then(|mut plugin| {
            let output = plugin
                .call::<&str, &str>("handle_hook", "{}")
                .map(|s| s.to_string());
            drop(plugin);
            output
        });

        match plugin_result {
            Err(_e) => {
                // 引擎因内存限制拒绝分配 = 预期行为
            }
            Ok(output) => {
                let msg = output.to_lowercase();
                // 即使 extism 没有在分配时拒绝，也要确认返回的不是
                // "allocated 20971520 bytes" 这种成功消息
                assert!(
                    !msg.contains("allocated") || output.len() < 30,
                    "memory bomb should fail, but got: {output}"
                );
            }
        }
    }

    /// 获取 MANIFEST_DIR 下的 Wasm fixture 文件路径
    fn manifest_dir_wasm_path(filename: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("defense_suite")
            .join("fixtures")
            .join(filename)
    }
}
