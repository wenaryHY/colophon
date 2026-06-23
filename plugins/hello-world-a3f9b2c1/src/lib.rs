use extism_pdk::*;
use serde::{Deserialize, Serialize};

/// 宿主传入的请求结构（必须与宿主侧 sandbox.rs 的 HookRequest 一致）
#[derive(Deserialize)]
struct HookRequest {
    hook_name: String,
    data: serde_json::Value,
}

/// 返回给宿主的响应结构（必须与宿主侧 sandbox.rs 的 HookResponse 一致）
#[derive(Serialize)]
struct HookResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    modified_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[plugin_fn]
pub fn handle_hook(input: String) -> FnResult<String> {
    let request: HookRequest = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => return Err(WithReturnCode(extism_pdk::Error::msg(e.to_string()), 1)),
    };

    match request.hook_name.as_str() {
        "post.before_save" => handle_post_before_save(request.data),
        _ => {
            let response = HookResponse {
                modified_data: None,
                error: Some(format!("unknown hook: {}", request.hook_name)),
            };
            Ok(serde_json::to_string(&response).unwrap())
        }
    }
}

fn handle_post_before_save(mut data: serde_json::Value) -> FnResult<String> {
    // 修改 title：追加验证标记，证明 Wasm Filter 全链路工作
    if let Some(title) = data.get("title").and_then(|v| v.as_str()) {
        let modified_title = format!("{} [Wasm Validated]", title);
        if let Some(obj) = data.as_object_mut() {
            obj.insert("title".into(), serde_json::Value::String(modified_title));
        }
    }

    // 修改 content_html：在末尾追加一段注释
    if let Some(content) = data.get("content_html").and_then(|v| v.as_str()) {
        let modified_content = format!(
            "{}<!-- processed by hello-world-wasm v0.1.0 -->",
            content
        );
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "content_html".into(),
                serde_json::Value::String(modified_content),
            );
        }
    }

    let response = HookResponse {
        modified_data: Some(data),
        error: None,
    };

    let output = match serde_json::to_string(&response) {
        Ok(s) => s,
        Err(e) => return Err(WithReturnCode(extism_pdk::Error::msg(e.to_string()), 2)),
    };

    info!("hello-world-wasm: processed post.before_save hook");
    Ok(output)
}
