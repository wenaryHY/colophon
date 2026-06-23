use extism_pdk::*;

#[plugin_fn]
pub fn handle_hook(_input: String) -> FnResult<String> {
    // 尝试分配 20MB (超过 10MB 限制即 160 页)
    let v = vec![0u8; 20 * 1024 * 1024];
    Ok(format!("allocated {} bytes", v.len()))
}
