use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::crypto;
use crate::service::node as node_svc;
use crate::service::node_sync;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    #[serde(default)]
    pub secret: String,
    #[serde(rename = "type", default)]
    pub typ: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub http: String,
    #[serde(default)]
    pub tls: String,
    #[serde(default)]
    pub socks: String,
}

/// `/system-info`
/// Query: secret, type, version, http, tls, socks
pub async fn handle_websocket(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(q): Query<WsQuery>,
) -> Response {
    let WsQuery {
        secret,
        typ,
        version,
        http,
        tls,
        socks,
    } = q;

    let session_id: i64;
    let node_secret: String;
    let is_node: bool;

    if typ == "1" {
        // 节点连接：按 secret 查 node
        if secret.is_empty() {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        match node_svc::get_by_secret(&state, &secret).await {
            Ok(Some(node)) => {
                session_id = node.id;
                node_secret = secret;
                is_node = true;
                tracing::info!(nodeId = node.id, version = %version, "节点通过验证");
            }
            _ => {
                tracing::info!("节点验证失败：未找到匹配的secret");
                return StatusCode::UNAUTHORIZED.into_response();
            }
        }
    } else {
        // 管理员：JWT 在 secret 字段
        if secret.is_empty() || !state.jwt.validate(&secret) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        let Ok(uid) = state.jwt.user_id(&secret) else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        session_id = uid;
        node_secret = String::new();
        is_node = false;
    }

    ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            state,
            session_id,
            node_secret,
            is_node,
            version,
            http,
            tls,
            socks,
        )
    })
}

#[allow(clippy::too_many_arguments)]
async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    session_id: i64,
    node_secret: String,
    is_node: bool,
    version: String,
    http: String,
    tls: String,
    socks: String,
) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let conn_id: u64;
    if is_node {
        conn_id = state
            .hub
            .register_node(session_id, tx.clone(), &node_secret, &version);
        // 更新在线状态
        match node_svc::mark_online(&state, session_id, &version, &http, &tls, &socks).await {
            Ok(()) => {
                tracing::info!(nodeId = session_id, version = %version, "节点连接建立成功");
                broadcast_status(&state, session_id, 1);
                // 上线后按 DB 全量重放期望配置（异步，不阻塞读循环）
                let st = state.clone();
                let node_id = session_id;
                tokio::spawn(async move {
                    node_sync::sync_node_config(&st, node_id).await;
                });
            }
            Err(e) => {
                tracing::warn!(nodeId = session_id, err = %e, "节点状态更新失败");
            }
        }
    } else {
        conn_id = state.hub.register_admin(tx.clone());
        tracing::info!(userId = session_id, "管理员连接建立");
    }

    while let Some(Ok(msg)) = receiver.next().await {
        let raw = match msg {
            Message::Text(t) => t.as_str().as_bytes().to_vec(),
            Message::Binary(b) => b.to_vec(),
            Message::Close(_) => break,
            _ => continue,
        };

        // 解密副本用于广播
        let plain = decrypt_if_needed(&node_secret, &raw);
        state
            .hub
            .handle_incoming(
                if is_node { Some(session_id) } else { None },
                &node_secret,
                &raw,
            )
            .await;

        if is_node {
            let msg = json!({
                "id": session_id.to_string(),
                "type": "info",
                "data": String::from_utf8_lossy(&plain),
            });
            if let Ok(bytes) = serde_json::to_vec(&msg) {
                // 异步广播：绝不阻塞节点读循环 / 指令通道
                state.hub.broadcast_admins(bytes);
            }
        }
    }

    if is_node {
        state.hub.unregister_node(session_id, conn_id);
        // 仅当仍是当前 session 时 UnregisterNode 会清 map；再查是否在线
        if !state.hub.is_node_online(session_id) {
            match node_svc::mark_offline(&state, session_id).await {
                Ok(()) => {
                    tracing::info!(nodeId = session_id, "节点状态更新为离线");
                    broadcast_status(&state, session_id, 0);
                }
                Err(e) => {
                    tracing::warn!(nodeId = session_id, err = %e, "节点离线状态更新失败");
                }
            }
        } else {
            tracing::info!(
                nodeId = session_id,
                "节点连接关闭但已有新连接，跳过状态更新"
            );
        }
    } else {
        state.hub.unregister_admin(conn_id);
        tracing::info!(userId = session_id, "管理员连接关闭");
    }

    drop(tx);
    writer.abort();
}

fn broadcast_status(state: &AppState, node_id: i64, status: i32) {
    let msg = json!({
        "id": node_id.to_string(),
        "type": "status",
        "data": status,
    });
    if let Ok(bytes) = serde_json::to_vec(&msg) {
        state.hub.broadcast_admins(bytes);
    }
}

fn decrypt_if_needed(secret: &str, raw: &[u8]) -> Vec<u8> {
    if secret.is_empty() {
        return raw.to_vec();
    }
    let Some(c) = crypto::get_or_create(secret) else {
        return raw.to_vec();
    };
    if let Ok(wrap) = serde_json::from_slice::<Value>(raw) {
        let encrypted = wrap
            .get("encrypted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let data = wrap.get("data").and_then(Value::as_str).unwrap_or("");
        if encrypted && !data.is_empty() {
            if let Ok(plain) = c.decrypt(data) {
                return plain;
            }
        }
    }
    raw.to_vec()
}
