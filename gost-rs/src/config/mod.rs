//! config 模块入口。
//!
//! 提供：
//! - `types`：serde 结构（与 Go 版 x/config/config.go 字段 1:1）
//! - `persist`：原子读写 gost.json（写临时文件后 rename）
//! - `load_node_config()`：从工作目录读取 config.json 并反序列化
//!
//! 字段名约定：
//! - 默认使用 `#[serde(rename_all = "camelCase")]`
//! - 部分与 Go yaml tag 不一致的字段用 `#[serde(rename = "...")]` 覆盖
//!
//! 与 Java springboot-backend WebSocketServer.java 的 AES 解密载荷保持兼容。

pub mod persist;
pub mod types;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub use types::*;

/// 从工作目录读取 `config.json` 并反序列化为 [`NodeConfig`]。
///
/// `config.json` 是节点端的运行时配置（面板地址、密钥、协议屏蔽等），
/// 不是 gost 服务/链/限流器配置（后者在 `gost.json`）。
pub fn load_node_config() -> Result<NodeConfig> {
    load_node_config_from("config.json")
}

/// 从指定路径加载 config.json。
pub fn load_node_config_from<P: AsRef<Path>>(path: P) -> Result<NodeConfig> {
    let path = path.as_ref();
    let data = fs::read(path).with_context(|| format!("读取 {} 失败", path.display()))?;
    let cfg: NodeConfig =
        serde_json::from_slice(&data).with_context(|| format!("解析 {} 失败", path.display()))?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_config_default_serde() {
        let s = r#"{
            "addr": "1.2.3.4:8080",
            "secret": "abc123",
            "http": 0,
            "tls": 0,
            "socks": 0,
            "ssl": false
        }"#;
        let cfg: NodeConfig = serde_json::from_str(s).unwrap();
        assert_eq!(cfg.addr, "1.2.3.4:8080");
        assert_eq!(cfg.secret, "abc123");
        assert_eq!(cfg.http, 0);
        assert_eq!(cfg.tls, 0);
        assert_eq!(cfg.socks, 0);
        assert!(!cfg.ssl);
    }

    #[test]
    fn node_config_optional_fields_default() {
        // http/tls/socks/ssl 缺省时应默认 0/false
        let s = r#"{"addr": "x:1", "secret": "y"}"#;
        let cfg: NodeConfig = serde_json::from_str(s).unwrap();
        assert_eq!(cfg.http, 0);
        assert_eq!(cfg.tls, 0);
        assert_eq!(cfg.socks, 0);
        assert!(!cfg.ssl);
    }
}