//! WebSocket 命令/响应类型定义（与 springboot-backend WebSocketServer 1:1）。
//!
//! ## 11 个下行命令
//!
//! 1. `PING` - 心跳（返回 Pong）
//! 2. `GetConfig` - 查询 gost.json（返回全量 Config）
//! 3. `SetProtocol` - 设置协议屏蔽（写 config.json）
//! 4. `TCPPing` - TCP 探测（addr 返回耗时）
//! 5. `GetNodes` - 查询节点列表（从 service registry 提取）
//! 6. `AddService` - 注册新服务
//! 7. `UpdateService` - 替换服务
//! 8. `DeleteService` - 删除服务
//! 9. `PauseService` - 暂停
//! 10. `ResumeService` - 恢复
//! 11. `GetService` - 查询单个服务（含流量 stats）
//!
//! ## 命令 JSON 格式（Java 端解密后的格式）
//!
//! ```json
//! {
//!   "type": "AddService",
//!   "data": { /* ServiceConfig JSON */ },
//!   "requestId": "uuid"
//! }
//! ```
//!
//! 注：Java 端常用 PascalCase type，data 是内嵌 JSON 对象；
//! 我们的 CommandType 用 kebab-case 但反序列化支持 PascalCase 兼容。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::types::{Config, ServiceConfig};

/// 命令类型字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandType {
    Ping,
    GetConfig,
    SetProtocol,
    TcpPing,
    GetNodes,
    AddService,
    UpdateService,
    DeleteService,
    PauseService,
    ResumeService,
    GetService,
    /// 未知命令（用于将来扩展）
    #[serde(other)]
    Unknown,
}

impl CommandType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ping => "PING",
            Self::GetConfig => "GetConfig",
            Self::SetProtocol => "SetProtocol",
            Self::TcpPing => "TCPPing",
            Self::GetNodes => "GetNodes",
            Self::AddService => "AddService",
            Self::UpdateService => "UpdateService",
            Self::DeleteService => "DeleteService",
            Self::PauseService => "PauseService",
            Self::ResumeService => "ResumeService",
            Self::GetService => "GetService",
            Self::Unknown => "Unknown",
        }
    }
}

/// 下行命令载荷。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Command {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(rename = "requestId", default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl Command {
    /// 解析为 enum。
    pub fn command_type(&self) -> CommandType {
        // Java 端有 PascalCase ("AddService") 与 camelCase ("addService") 两种
        let upper: String = self.kind.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        match upper.to_uppercase().as_str() {
            "PING" => CommandType::Ping,
            "GETCONFIG" => CommandType::GetConfig,
            "SETPROTOCOL" => CommandType::SetProtocol,
            "TCPPING" => CommandType::TcpPing,
            "GETNODES" => CommandType::GetNodes,
            "ADDSERVICE" => CommandType::AddService,
            "UPDATESERVICE" => CommandType::UpdateService,
            "DELETESERVICE" => CommandType::DeleteService,
            "PAUSESERVICE" => CommandType::PauseService,
            "RESUMESERVICE" => CommandType::ResumeService,
            "GETSERVICE" => CommandType::GetService,
            _ => CommandType::Unknown,
        }
    }
}

/// 上行响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(rename = "type")]
    pub kind: String,
    /// 命令类型（PascalCase）—— 用于面板匹配 request
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(rename = "requestId", default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// "ok" / "error"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Response {
    pub fn ok(cmd: &str, request_id: Option<String>, data: serde_json::Value) -> Self {
        Self {
            kind: "response".into(),
            cmd: Some(cmd.into()),
            data: Some(data),
            request_id,
            status: Some("ok".into()),
            message: None,
        }
    }

    pub fn error(cmd: &str, request_id: Option<String>, msg: impl Into<String>) -> Self {
        Self {
            kind: "response".into(),
            cmd: Some(cmd.into()),
            data: None,
            request_id,
            status: Some("error".into()),
            message: Some(msg.into()),
        }
    }

    pub fn pong() -> Self {
        Self {
            kind: "Pong".into(),
            cmd: Some("PING".into()),
            data: None,
            request_id: None,
            status: Some("ok".into()),
            message: None,
        }
    }
}

/// SetProtocol data：HTTP/TLS/SOCKS 屏蔽标志。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SetProtocolData {
    #[serde(default)]
    pub http: i32,
    #[serde(default)]
    pub tls: i32,
    #[serde(default)]
    pub socks: i32,
}

/// TcpPing data：探测目标。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TcpPingData {
    pub addr: String,
}

/// DeleteService data：服务名（裸字符串）。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeleteServiceData {
    pub name: String,
}

/// GetService data：服务名 + 是否清空 stats。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetServiceData {
    pub name: String,
    #[serde(default)]
    pub clear: bool,
}

/// GetNodes 响应的单个节点信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub name: String,
    pub addr: String,
    pub port: i32,
    pub protocol: String,
    pub state: String,
    pub tcp: i64,
    pub udp: i64,
}

/// GetNodes 响应：节点列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodesResponse {
    pub nodes: Vec<NodeInfo>,
}

/// 全量 Config 响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfigResponse {
    pub services: Vec<ServiceConfig>,
    pub chains: Vec<crate::config::types::ChainConfig>,
    pub hops: Vec<crate::config::types::HopConfig>,
    #[serde(flatten)]
    pub rest: HashMap<String, serde_json::Value>,
}

impl From<Config> for GetConfigResponse {
    fn from(c: Config) -> Self {
        Self {
            services: c.services,
            chains: c.chains,
            hops: c.hops,
            rest: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_type_pascal_case() {
        let s = r#"{"type":"AddService","requestId":"r1"}"#;
        let c: Command = serde_json::from_str(s).unwrap();
        assert_eq!(c.command_type(), CommandType::AddService);
    }

    #[test]
    fn command_type_lower_case() {
        let s = r#"{"type":"addService","requestId":"r1"}"#;
        let c: Command = serde_json::from_str(s).unwrap();
        assert_eq!(c.command_type(), CommandType::AddService);
    }

    #[test]
    fn response_ok_serializes() {
        let r = Response::ok("AddService", Some("r1".into()), serde_json::json!({"name":"s"}));
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"type\":\"response\""));
        assert!(s.contains("\"cmd\":\"AddService\""));
        assert!(s.contains("\"status\":\"ok\""));
    }

    #[test]
    fn pong_serializes() {
        let p = Response::pong();
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("Pong"));
    }
}