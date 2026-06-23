use extism_pdk::*;

#[plugin_fn]
pub fn handle_hook(_input: String) -> FnResult<String> {
    // 返回 1MB + 1 字节的字符串，超过 MAX_WASM_OUTPUT_BYTES (1MB) 限制
    Ok("A".repeat(1024 * 1024 + 1))
}
