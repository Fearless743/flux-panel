use axum::extract::ws::Message;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::crypto;
use crate::response::now_ms;

#[derive(Debug, Clone, Default)]
pub struct GostResult {
    pub msg: String,
    pub data: Option<Value>,
}

struct NodeSession {
    tx: mpsc::UnboundedSender<Message>,
    secret: String,
    version: String,
    conn_id: u64,
}

pub struct Hub {
    nodes: Mutex<HashMap<i64, NodeSession>>,
    admins: Mutex<HashMap<u64, mpsc::UnboundedSender<Message>>>,
    pending: Mutex<HashMap<String, oneshot::Sender<GostResult>>>,
    next_conn_id: AtomicU64,
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Hub {
    pub fn new() -> Self {
        Self {
            nodes: Mutex::new(HashMap::new()),
            admins: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            next_conn_id: AtomicU64::new(1),
        }
    }

    pub fn is_node_online(&self, node_id: i64) -> bool {
        self.nodes.lock().unwrap().contains_key(&node_id)
    }

    pub async fn send_msg(&self, node_id: i64, data: Value, typ: &str) -> GostResult {
        self.send_msg_timeout(node_id, data, typ, Duration::from_secs(10))
            .await
    }

    pub async fn send_msg_timeout(
        &self,
        node_id: i64,
        data: Value,
        typ: &str,
        timeout: Duration,
    ) -> GostResult {
        let (tx, secret) = {
            let nodes = self.nodes.lock().unwrap();
            match nodes.get(&node_id) {
                Some(s) => (s.tx.clone(), s.secret.clone()),
                None => {
                    return GostResult {
                        msg: "节点不在线".to_string(),
                        data: None,
                    }
                }
            }
        };
        let timeout = if timeout.is_zero() {
            Duration::from_secs(10)
        } else {
            timeout
        };

        let req_id = Uuid::new_v4().to_string();
        let (otx, orx) = oneshot::channel();
        self.pending.lock().unwrap().insert(req_id.clone(), otx);

        let payload = json!({
            "type": typ,
            "data": data,
            "requestId": req_id,
        });
        let raw = serde_json::to_vec(&payload).unwrap_or_default();
        let msg = self.wrap(&secret, raw);
        if tx.send(msg).is_err() {
            self.pending.lock().unwrap().remove(&req_id);
            return GostResult {
                msg: "发送消息失败: channel closed".to_string(),
                data: None,
            };
        }

        let res = tokio::time::timeout(timeout, orx).await;
        self.pending.lock().unwrap().remove(&req_id);
        match res {
            Ok(Ok(r)) => r,
            _ => GostResult {
                msg: "等待响应超时".to_string(),
                data: None,
            },
        }
    }

    pub async fn send_no_wait(&self, node_id: i64, data: Value, typ: &str) {
        let (tx, secret) = {
            let nodes = self.nodes.lock().unwrap();
            match nodes.get(&node_id) {
                Some(s) => (s.tx.clone(), s.secret.clone()),
                None => return,
            }
        };
        let payload = json!({
            "type": typ,
            "data": data,
            "requestId": Uuid::new_v4().to_string(),
        });
        let raw = serde_json::to_vec(&payload).unwrap_or_default();
        let _ = tx.send(self.wrap(&secret, raw));
    }

    pub fn register_node(
        &self,
        node_id: i64,
        tx: mpsc::UnboundedSender<Message>,
        secret: &str,
        version: &str,
    ) -> u64 {
        let conn_id = self.next_conn_id.fetch_add(1, Ordering::Relaxed);
        let old = {
            let mut nodes = self.nodes.lock().unwrap();
            let old = nodes.insert(
                node_id,
                NodeSession {
                    tx,
                    secret: secret.to_string(),
                    version: version.to_string(),
                    conn_id,
                },
            );
            old
        };
        if let Some(old) = old {
            let _ = old.tx.send(Message::Close(None));
        }
        conn_id
    }

    /// 仅当仍是当前连接时移除；返回是否移除了当前 session
    pub fn unregister_node(&self, node_id: i64, conn_id: u64) -> bool {
        let mut nodes = self.nodes.lock().unwrap();
        if let Some(cur) = nodes.get(&node_id) {
            if cur.conn_id == conn_id {
                nodes.remove(&node_id);
                return true;
            }
        }
        false
    }

    pub fn register_admin(&self, tx: mpsc::UnboundedSender<Message>) -> u64 {
        let id = self.next_conn_id.fetch_add(1, Ordering::Relaxed);
        self.admins.lock().unwrap().insert(id, tx);
        id
    }

    pub fn unregister_admin(&self, conn_id: u64) {
        self.admins.lock().unwrap().remove(&conn_id);
    }

    pub fn broadcast_admins(&self, message: Vec<u8>) {
        let txs: Vec<_> = self.admins.lock().unwrap().values().cloned().collect();
        let text = String::from_utf8_lossy(&message).to_string();
        for tx in txs {
            let _ = tx.send(Message::Text(text.clone().into()));
        }
    }

    pub async fn handle_incoming(&self, node_id: Option<i64>, secret: &str, raw: &[u8]) {
        let payload = self.decrypt_if_needed(secret, raw);
        let s = String::from_utf8_lossy(&payload);

        if s.contains("memory_usage") {
            if let Some(id) = node_id {
                self.send_raw(id, br#"{"type":"call"}"#.to_vec()).await;
            }
            return;
        }

        if s.contains("requestId") {
            let Ok(resp) = serde_json::from_slice::<Value>(&payload) else {
                tracing::warn!("parse ws response failed");
                return;
            };
            let req_id = resp
                .get("requestId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if req_id.is_empty() {
                return;
            }
            let sender = self.pending.lock().unwrap().remove(&req_id);
            if let Some(sender) = sender {
                let mut msg = resp
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if msg.is_empty() {
                    msg = "无响应消息".to_string();
                }
                let data = resp.get("data").and_then(|v| {
                    if v.is_null() {
                        None
                    } else {
                        Some(v.clone())
                    }
                });
                let _ = sender.send(GostResult { msg, data });
            }
            return;
        }

        let truncated = if s.len() > 200 {
            format!("{}...", &s[..s.char_indices().nth(200).map(|(i, _)| i).unwrap_or(s.len())])
        } else {
            s.to_string()
        };
        tracing::info!("ws message: {}", truncated);
    }

    async fn send_raw(&self, node_id: i64, raw: Vec<u8>) {
        let (tx, secret) = {
            let nodes = self.nodes.lock().unwrap();
            match nodes.get(&node_id) {
                Some(s) => (s.tx.clone(), s.secret.clone()),
                None => return,
            }
        };
        let _ = tx.send(self.wrap(&secret, raw));
    }

    fn wrap(&self, secret: &str, plain: Vec<u8>) -> Message {
        let mut msg = plain;
        if !secret.is_empty() {
            if let Some(c) = crypto::get_or_create(secret) {
                if let Ok(enc) = c.encrypt(&msg) {
                    let wrap = json!({
                        "encrypted": true,
                        "data": enc,
                        "timestamp": now_ms(),
                    });
                    msg = serde_json::to_vec(&wrap).unwrap_or(msg);
                }
            }
        }
        Message::Text(String::from_utf8_lossy(&msg).to_string().into())
    }

    fn decrypt_if_needed(&self, secret: &str, raw: &[u8]) -> Vec<u8> {
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
}
