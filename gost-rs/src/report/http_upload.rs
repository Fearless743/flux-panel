//! HTTP 上报：流量 (`/flow/upload`) 与 gost.json 全量 (`/flow/config`)。
//!
//! 与 Go 版 `x/service/traffic_reporter.go` + `x/service/config_reporter.go` 对齐：
//! - `/flow/upload`：每 5s 上传 delta 流量（AES 加密 POST，body = `"ok"` 响应）
//! - `/flow/config`：每 10min 上传 gost.json（AES 加密 POST，body = `"ok"`）
//!
//! base URL = `http://<panel_addr>`（来自 NodeConfig.addr）。

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::types::NodeConfig;
use crate::util::aes::encrypt;

/// 流量上报 delta 数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficDelta {
    pub services: Vec<ServiceTraffic>,
    #[serde(rename = "timestamp", default)]
    pub timestamp: i64,
}

/// 单服务流量。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTraffic {
    pub name: String,
    #[serde(rename = "inputBytes", default)]
    pub input_bytes: u64,
    #[serde(rename = "outputBytes", default)]
    pub output_bytes: u64,
    #[serde(rename = "totalConns", default)]
    pub total_conns: u64,
    #[serde(rename = "currentConns", default)]
    pub current_conns: u64,
}

/// HTTP 上报器。
pub struct HttpUploader {
    client: Client,
    base_url: String,
    secret: String,
}

impl HttpUploader {
    /// 构造（带 5s 超时）。
    pub fn new(node_cfg: &NodeConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .context("构造 reqwest client 失败")?;
        let scheme = if node_cfg.ssl { "https" } else { "http" };
        Ok(Self {
            client,
            base_url: format!("{}://{}", scheme, node_cfg.addr),
            secret: node_cfg.secret.clone(),
        })
    }

    /// 上传流量 delta。
    ///
    /// 流程：
    /// 1. 序列化 delta 为 JSON
    /// 2. AES 加密 → base64
    /// 3. POST `/flow/upload`，body = 加密结果
    /// 4. 期望响应 body = "ok"
    pub async fn upload_traffic(&self, delta: &TrafficDelta) -> Result<()> {
        let body = serde_json::to_vec(delta).context("序列化流量失败")?;
        let encrypted = encrypt(&body, &self.secret)?;
        let url = format!("{}/flow/upload", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "text/plain")
            .body(encrypted)
            .send()
            .await
            .context("POST /flow/upload 失败")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("/flow/upload HTTP {}: {}", status, text);
        }
        if text.trim() != "ok" {
            anyhow::bail!("/flow/upload 响应非 ok：{}", text);
        }
        Ok(())
    }

    /// 上传 gost.json 全量配置。
    ///
    /// Java 端用 `/flow/config` 接收 gost.json 完整内容（AES 加密）。
    pub async fn upload_config(&self, config_json: &[u8]) -> Result<()> {
        let encrypted = encrypt(config_json, &self.secret)?;
        let url = format!("{}/flow/config", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "text/plain")
            .body(encrypted)
            .send()
            .await
            .context("POST /flow/config 失败")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("/flow/config HTTP {}: {}", status, text);
        }
        if text.trim() != "ok" {
            anyhow::bail!("/flow/config 响应非 ok：{}", text);
        }
        Ok(())
    }

    /// TCP 探测（gost 节点端用，与 Go `x/service/tcp_ping.go` 对齐）。
    pub async fn tcp_ping(&self, addr: &str) -> Result<u128> {
        use tokio::net::TcpStream;
        use tokio::time::Instant;
        let start = Instant::now();
        let timeout_dur = Duration::from_secs(3);
        let res = tokio::time::timeout(timeout_dur, TcpStream::connect(addr)).await;
        let elapsed = start.elapsed().as_millis();
        match res {
            Ok(Ok(_)) => Ok(elapsed),
            Ok(Err(e)) => Err(anyhow::anyhow!("connect {addr}: {e}")),
            Err(_) => Err(anyhow::anyhow!("timeout {addr}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodeConfig;

    #[test]
    fn uploader_builds_with_ssl() {
        let cfg = NodeConfig {
            addr: "1.2.3.4:8080".into(),
            secret: "k".into(),
            ssl: true,
            ..Default::default()
        };
        let u = HttpUploader::new(&cfg).unwrap();
        assert_eq!(u.base_url, "https://1.2.3.4:8080");
    }

    #[test]
    fn uploader_builds_without_ssl() {
        let cfg = NodeConfig {
            addr: "1.2.3.4:8080".into(),
            secret: "k".into(),
            ssl: false,
            ..Default::default()
        };
        let u = HttpUploader::new(&cfg).unwrap();
        assert_eq!(u.base_url, "http://1.2.3.4:8080");
    }

    #[tokio::test]
    async fn tcp_ping_localhost() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        // 持续 accept，让 connect 能成功
        let accept_task = tokio::spawn(async move {
            loop {
                let _ = listener.accept().await;
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let cfg = NodeConfig::default();
        let u = HttpUploader::new(&cfg).unwrap();
        let ms = u.tcp_ping(&addr.to_string()).await.unwrap();
        assert!(ms < 1000);
        accept_task.abort();
    }

    #[tokio::test]
    async fn tcp_ping_invalid_addr() {
        let cfg = NodeConfig::default();
        let u = HttpUploader::new(&cfg).unwrap();
        assert!(u.tcp_ping("127.0.0.1:1").await.is_err());
    }
}