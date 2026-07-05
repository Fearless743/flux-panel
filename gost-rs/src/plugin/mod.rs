//! plugin 模块：gost 风格 stdio JSON 协议 + 8 个内置 plugin 子模块入口。
//!
//! ## protocol
//!
//! ```text
//! CLI → 节点端：每请求一行 JSON `{"type": "Authenticate", ...}\n`
//! 节点端 → CLI：每响应一行 JSON `{"ok": true, "data": {...}}\n`
//! ```
//!
//! 每个 plugin 子进程 (authenticator/resolver/hosts/recorder/sd/...)
//! 通过 stdin/stdout 与 gost 节点通信。

pub mod authenticator;
pub mod handler;
pub mod hosts;
pub mod observer;
pub mod recorder;
pub mod resolver;
pub mod sd;

pub use handler::run_handler_plugin;

use serde::{Deserialize, Serialize};

/// 节点端 → plugin 的请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequest {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Plugin → 节点端的响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PluginResponse {
    pub fn ok<T: Serialize>(data: T) -> Self {
        Self {
            ok: true,
            data: Some(serde_json::to_value(data).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }
    pub fn err<E: std::fmt::Display>(e: E) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(e.to_string()),
        }
    }
}
