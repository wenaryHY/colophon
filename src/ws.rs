use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::HeaderMap,
    response::Response,
};
use futures_util::{SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::{
    shared::auth::{decode_token, session_token_from_headers},
    state::AppState,
};

// ─── Server Event ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerEvent {
    #[serde(rename = "comment_created")]
    CommentCreated {
        id: String,
        post_id: String,
        author_name: String,
        content: String,
        status: String,
        created_at: String,
    },
    #[serde(rename = "comment_approved")]
    CommentApproved {
        id: String,
        post_id: String,
        author_name: String,
        content: String,
        created_at: String,
    },
    #[serde(rename = "comment_deleted")]
    CommentDeleted { id: String },
}

// ─── Admin WebSocket ────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct WsAdminParams {
    token: Option<String>,
}

pub async fn ws_admin_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<WsAdminParams>,
) -> Response {
    let token = params
        .token
        .or_else(|| session_token_from_headers(&headers));

    let Some(token) = token else {
        return Response::builder()
            .status(401)
            .body("Unauthorized".into())
            .unwrap();
    };

    let claims = match decode_token(&token, &state.config.auth.secret) {
        Ok(c) => c,
        Err(_) => {
            return Response::builder()
                .status(401)
                .body("Unauthorized".into())
                .unwrap()
        }
    };
    if claims.role != "admin" {
        return Response::builder()
            .status(403)
            .body("Forbidden".into())
            .unwrap();
    }

    ws.on_upgrade(move |socket| handle_ws(socket, state.event_tx.subscribe(), None))
}

// ─── Public WebSocket ───────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WsPublicParams {
    post_id: Option<String>,
}

pub async fn ws_public_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Query(params): Query<WsPublicParams>,
) -> Response {
    let post_id = params.post_id;
    ws.on_upgrade(move |socket| handle_ws(socket, state.event_tx.subscribe(), post_id))
}

// ─── Common WS Loop ─────────────────────────────────────────────────────────

/// 通用 WebSocket 消息推送循环
///
/// - `post_id`：前台连接时传入文章 ID，仅推送该文章的 approved 事件；管理后台传 None 推送全部事件
async fn handle_ws(
    socket: WebSocket,
    mut rx: broadcast::Receiver<ServerEvent>,
    post_id: Option<String>,
) {
    let (mut sender, mut receiver) = socket.split();

    let _recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            if matches!(msg, Ok(Message::Close(_))) {
                break;
            }
        }
    });

    while let Ok(event) = rx.recv().await {
        if let Some(ref pid) = post_id {
            match &event {
                ServerEvent::CommentApproved { post_id, .. } if post_id == pid => {}
                _ => continue,
            }
        }

        let json = match serde_json::to_string(&event) {
            Ok(j) => j,
            Err(_) => continue,
        };

        if sender.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
