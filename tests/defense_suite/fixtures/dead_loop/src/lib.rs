use extism_pdk::*;

#[plugin_fn]
pub fn handle_hook(_input: String) -> FnResult<String> {
    loop {} // 永不返回
}
