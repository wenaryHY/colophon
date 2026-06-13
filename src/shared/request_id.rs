use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use tokio::task_local;

pub const CLIENT_REQUEST_ID_HEADER: &str = "x-client-request-id";
pub const RESPONSE_REQUEST_ID_HEADER: &str = "x-request-id";

task_local! {
    static CURRENT_REQUEST_ID: String;
}

fn generate_request_id() -> String {
    format!("srv_{}", uuid::Uuid::new_v4())
}

pub fn current_request_id() -> Option<String> {
    CURRENT_REQUEST_ID.try_with(Clone::clone).ok()
}

pub fn current_or_generate_request_id() -> String {
    current_request_id().unwrap_or_else(generate_request_id)
}

pub fn extract_client_request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CLIENT_REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn extract_or_generate_client_request_id(headers: &HeaderMap) -> String {
    extract_client_request_id(headers)
        .or_else(current_request_id)
        .unwrap_or_else(generate_request_id)
}

pub async fn request_id_context(request: Request, next: Next) -> Response {
    let request_id =
        extract_client_request_id(request.headers()).unwrap_or_else(generate_request_id);
    CURRENT_REQUEST_ID
        .scope(request_id.clone(), async move {
            let mut response = next.run(request).await;
            if let Ok(header_value) = HeaderValue::from_str(&request_id) {
                response
                    .headers_mut()
                    .insert(CLIENT_REQUEST_ID_HEADER, header_value.clone());
                response
                    .headers_mut()
                    .insert(RESPONSE_REQUEST_ID_HEADER, header_value);
            }
            response
        })
        .await
}
