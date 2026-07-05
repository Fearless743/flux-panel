//! report 模块入口。
//!
//! 与面板通信的两条通道：
//! - WebSocket：下行（接收 11 个命令）+ 上行（Pong/Stats）
//! - HTTP POST `/flow/upload`（5s 流量）、`/flow/config`（10min gost.json 全量）
//!
//! 两条通道的载荷都用 AES-GCM 加密（AES 复用 `crate::util::aes`）。
//! WebSocket 下行可能 gzip 压缩；上行一律明文 JSON（流量 stats）或加密 JSON（config）。
//!
//! 子模块：
//! - [`messages`]：命令/响应类型定义（与 Java AESCrypto 解密后的 JSON 兼容）
//! - [`compressed`]：gzip 解压/压缩（与 Java GzipUtils 兼容）
//! - [`encrypted`]：AES 加解密 + base64
//! - [`websocket`]：WsReporter 主体
//! - [`http_upload`]：流量/配置上报
//! - [`sysinfo_util`]：节点系统信息（uptime/cpu/mem/net）

pub mod compressed;
pub mod encrypted;
pub mod http_upload;
pub mod messages;
pub mod sysinfo_util;
pub mod websocket;

pub use messages::{Command, CommandType, Response};
pub use websocket::WsReporter;