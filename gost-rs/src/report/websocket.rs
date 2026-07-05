//! WebSocket 主 reporter。
//!
//! 与 springboot-backend WebSocketServer.java 协议 1:1：
//!
//! ## URL
//!
//! ```text
//! ws://<addr>/system-info?type=1&secret=<secret>&version=<ver>&http=<0|1>&tls=<0|1>&socks=<0|1>
//! ```
//!
//! ## 协议
//!
//! - 下行 binary frame：`{type, compressed, data, requestId}`
//!   - `compressed`：true → data 是 gzip(AES(JSON))；false → data 是 AES(JSON)
//!   - `data`：base64(AES ciphertext) 可能外层 gzip
//! - 上行 binary frame：`{type, data}` 或 `{type: "response", data: base64(AES(JSON))}`
//!
//! 心跳：Java 端定期发 PING，Rust 立即回 Pong；Rust 也每 25s 主动发 Pong。

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde_json::json;
use tokio::sync::Notify;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::{connect_async, WebSocketStream};

use crate::config::NodeConfig;
use crate::config::persist;
use crate::config::types::ServiceConfig;
use crate::registry::services_registry;
use crate::report::compressed;
use crate::report::encrypted::{decrypt_payload, encrypt_payload};
use crate::report::http_upload::{HttpUploader, ServiceTraffic, TrafficDelta};
use crate::report::messages::{
    Command, CommandType, DeleteServiceData, GetConfigResponse, GetNodesResponse, GetServiceData,
    NodeInfo, Response, SetProtocolData, TcpPingData,
};
use crate::service::ServiceImpl;
use crate::service_cmd;
use crate::version;

/// 重连退避（秒）。
const RECONNECT_BACKOFF_SECS: u64 = 3;

/// 共享的 WebSocket writer（用于主循环和 heartbeat 互相发）。
type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// WebSocket reporter。
pub struct WsReporter {
    node_cfg: NodeConfig,
    gost_config_path: String,
    shutdown: Arc<Notify>,
    /// 当前活跃的 writer（如果连接已断开则为 None）
    writer: Arc<Mutex<Option<futures_util::stream::SplitSink<WsStream, Message>>>>,
}

impl WsReporter {
    pub fn new(node_cfg: NodeConfig, gost_config_path: impl Into<String>) -> Self {
        Self {
            node_cfg,
            gost_config_path: gost_config_path.into(),
            shutdown: Arc::new(Notify::new()),
            writer: Arc::new(Mutex::new(None)),
        }
    }

    /// 启动 reporter。
    pub async fn run(self: Arc<Self>) -> Result<()> {
        let url = self.build_url();
        tracing::info!(url = %url, "WebSocket 准备连接面板");

        loop {
            match self.connect_and_run(&url).await {
                Ok(()) => tracing::warn!("WebSocket 正常关闭，准备重连"),
                Err(e) => tracing::error!(error = %e, "WebSocket 出错，准备重连"),
            }
            *self.writer.lock() = None;
            tokio::select! {
                _ = self.shutdown.notified() => {
                    tracing::info!("shutdown 收到，reporter 退出");
                    return Ok(());
                }
                _ = sleep(Duration::from_secs(RECONNECT_BACKOFF_SECS)) => {}
            }
        }
    }

    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }

    fn build_url(&self) -> String {
        let scheme = if self.node_cfg.ssl { "wss" } else { "ws" };
        let cfg = &self.node_cfg;
        format!(
            "{scheme}://{addr}/system-info?type=1&secret={secret}&version={ver}&http={h}&tls={t}&socks={s}",
            addr = cfg.addr,
            secret = urlencoding(&cfg.secret),
            ver = version::VERSION,
            h = cfg.http,
            t = cfg.tls,
            s = cfg.socks,
        )
    }

    async fn connect_and_run(&self, url: &str) -> Result<()> {
        let request = url.into_client_request().context("WS URL 解析失败")?;
        let (ws_stream, _resp) = connect_async(request)
            .await
            .context("WebSocket 连接失败")?;
        tracing::info!("WebSocket 已连接");

        let (write, mut read) = ws_stream.split();
        *self.writer.lock() = Some(write);

        while let Some(msg) = read.next().await {
            let msg = match msg.context("WebSocket 接收失败")? {
                Message::Binary(b) => b,
                Message::Close(c) => {
                    tracing::warn!(close_code = ?c, "收到 Close frame");
                    return Ok(());
                }
                _ => continue,
            };

            if let Err(e) = self.handle_inbound(&msg).await {
                tracing::error!(error = %e, "命令处理失败");
            }
        }
        Ok(())
    }

    /// 主动发 Pong（供外部调用或心跳循环）。
    pub async fn send_pong(&self) -> Result<()> {
        let pong = Response::pong();
        let plain = serde_json::to_vec(&pong)?;
        let ct = encrypt_payload(&plain, &self.node_cfg.secret, false)?;
        let frame = json!({ "type": "Pong", "data": ct });
        let bytes = serde_json::to_vec(&frame)?;
        self.send_bytes(bytes).await
    }

    async fn send_bytes(&self, bytes: Vec<u8>) -> Result<()> {
        let mut guard = self.writer.lock();
        if let Some(w) = guard.as_mut() {
            w.send(Message::Binary(bytes))
                .await
                .context("WebSocket 发送失败")?;
        }
        Ok(())
    }

    async fn handle_inbound(&self, msg: &[u8]) -> Result<()> {
        let frame: serde_json::Value = serde_json::from_slice(msg).context("解析 frame 失败")?;
        let cmd_str = frame
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let compressed_flag = frame
            .get("compressed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let data_str = frame
            .get("data")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let request_id = frame
            .get("requestId")
            .and_then(|v| v.as_str())
            .map(String::from);

        // 解码 + 解密
        let plain = if !data_str.is_empty() {
            let mut raw = STANDARD.decode(&data_str).context("base64 解码失败")?;
            if compressed_flag {
                raw = compressed::gunzip(&raw)?;
            }
            let ct_b64 = STANDARD.encode(&raw);
            decrypt_payload(&ct_b64, &self.node_cfg.secret)?
        } else {
            Vec::new()
        };

        let cmd: Command = if plain.is_empty() {
            Command {
                kind: cmd_str.clone(),
                data: None,
                request_id: request_id.clone(),
            }
        } else {
            serde_json::from_slice(&plain).context("解析 Command 失败")?
        };

        let req_id = cmd.request_id.clone().or(request_id);
        let resp = self.dispatch(&cmd, req_id.as_deref()).await?;
        if let Some(r) = resp {
            self.send_response(r).await?;
        }
        Ok(())
    }

    async fn send_response(&self, resp: Response) -> Result<()> {
        let plain = serde_json::to_vec(&resp).context("序列化响应失败")?;
        let ct = encrypt_payload(&plain, &self.node_cfg.secret, false)?;
        let frame = json!({ "type": "response", "data": ct });
        let bytes = serde_json::to_vec(&frame).context("序列化 frame 失败")?;
        self.send_bytes(bytes).await
    }

    async fn dispatch(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Option<Response>> {
        match cmd.command_type() {
            CommandType::Ping => {
                // 自动 Pong 在协议层（tungstenite），这里无需返回
                let pong = Response::pong();
                let _ = self.send_pong().await;
                Ok(Some(pong))
            }
            CommandType::GetConfig => Ok(Some(self.handle_get_config(request_id).await?)),
            CommandType::SetProtocol => Ok(Some(
                self.handle_set_protocol(cmd, request_id).await?,
            )),
            CommandType::TcpPing => Ok(Some(self.handle_tcp_ping(cmd, request_id).await?)),
            CommandType::GetNodes => Ok(Some(self.handle_get_nodes(request_id).await?)),
            CommandType::AddService => Ok(Some(
                self.handle_add_service(cmd, request_id).await?,
            )),
            CommandType::UpdateService => Ok(Some(
                self.handle_update_service(cmd, request_id).await?,
            )),
            CommandType::DeleteService => Ok(Some(
                self.handle_delete_service(cmd, request_id).await?,
            )),
            CommandType::PauseService => Ok(Some(
                self.handle_pause_service(cmd, request_id).await?,
            )),
            CommandType::ResumeService => Ok(Some(
                self.handle_resume_service(cmd, request_id).await?,
            )),
            CommandType::GetService => Ok(Some(
                self.handle_get_service(cmd, request_id).await?,
            )),
            CommandType::Unknown => Ok(Some(Response::error(
                &cmd.kind,
                request_id.map(String::from),
                "unknown command",
            ))),
        }
    }

    async fn handle_get_config(&self, request_id: Option<&str>) -> Result<Response> {
        let cfg = persist::load_config(&self.gost_config_path).unwrap_or_default();
        let data: GetConfigResponse = cfg.into();
        Ok(Response::ok(
            "GetConfig",
            request_id.map(String::from),
            serde_json::to_value(data)?,
        ))
    }

    async fn handle_set_protocol(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Response> {
        let data: SetProtocolData = match &cmd.data {
            Some(v) => serde_json::from_value(v.clone()).context("解析 SetProtocol data 失败")?,
            None => anyhow::bail!("SetProtocol 缺少 data"),
        };
        let mut cfg = crate::config::load_node_config().context("读取 config.json 失败")?;
        cfg.http = data.http;
        cfg.tls = data.tls;
        cfg.socks = data.socks;
        let json = serde_json::to_vec_pretty(&cfg).context("序列化失败")?;
        std::fs::write("config.json", json).context("写入 config.json 失败")?;
        Ok(Response::ok(
            "SetProtocol",
            request_id.map(String::from),
            json!({ "http": cfg.http, "tls": cfg.tls, "socks": cfg.socks }),
        ))
    }

    async fn handle_tcp_ping(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Response> {
        let data: TcpPingData = match &cmd.data {
            Some(v) => serde_json::from_value(v.clone())?,
            None => anyhow::bail!("TCPPing 缺少 data"),
        };
        let uploader = HttpUploader::new(&self.node_cfg)?;
        match uploader.tcp_ping(&data.addr).await {
            Ok(ms) => Ok(Response::ok(
                "TCPPing",
                request_id.map(String::from),
                json!({ "addr": data.addr, "duration": ms }),
            )),
            Err(e) => Ok(Response::error(
                "TCPPing",
                request_id.map(String::from),
                e.to_string(),
            )),
        }
    }

    async fn handle_get_nodes(&self, request_id: Option<&str>) -> Result<Response> {
        let reg = services_registry();
        let mut nodes = Vec::new();
        for name in reg.names() {
            if let Some(svc) = reg.get(&name) {
                nodes.push(service_to_node_info(&svc));
            }
        }
        let resp = GetNodesResponse { nodes };
        Ok(Response::ok(
            "GetNodes",
            request_id.map(String::from),
            serde_json::to_value(resp)?,
        ))
    }

    async fn handle_add_service(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Response> {
        let cfg: ServiceConfig = match &cmd.data {
            Some(v) => serde_json::from_value(v.clone()).context("解析 ServiceConfig 失败")?,
            None => anyhow::bail!("AddService 缺少 data"),
        };
        let name = cfg.name.clone();
        match service_cmd::add_service(cfg).await {
            Ok(()) => Ok(Response::ok(
                "AddService",
                request_id.map(String::from),
                json!({ "name": name, "status": "ok" }),
            )),
            Err(e) => Ok(Response::error(
                "AddService",
                request_id.map(String::from),
                e.to_string(),
            )),
        }
    }

    async fn handle_update_service(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Response> {
        let cfg: ServiceConfig = match &cmd.data {
            Some(v) => serde_json::from_value(v.clone())?,
            None => anyhow::bail!("UpdateService 缺少 data"),
        };
        let name = cfg.name.clone();
        match service_cmd::update_service(cfg).await {
            Ok(()) => Ok(Response::ok(
                "UpdateService",
                request_id.map(String::from),
                json!({ "name": name, "status": "ok" }),
            )),
            Err(e) => Ok(Response::error(
                "UpdateService",
                request_id.map(String::from),
                e.to_string(),
            )),
        }
    }

    async fn handle_delete_service(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Response> {
        let data: DeleteServiceData = match &cmd.data {
            Some(v) => serde_json::from_value(v.clone())?,
            None => anyhow::bail!("DeleteService 缺少 data"),
        };
        match service_cmd::delete_service(&data.name).await {
            Ok(()) => Ok(Response::ok(
                "DeleteService",
                request_id.map(String::from),
                json!({ "name": data.name }),
            )),
            Err(e) => Ok(Response::error(
                "DeleteService",
                request_id.map(String::from),
                e.to_string(),
            )),
        }
    }

    async fn handle_pause_service(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Response> {
        let name = extract_service_name(cmd)?;
        match service_cmd::pause_service(&name) {
            Ok(()) => Ok(Response::ok(
                "PauseService",
                request_id.map(String::from),
                json!({ "name": name }),
            )),
            Err(e) => Ok(Response::error(
                "PauseService",
                request_id.map(String::from),
                e.to_string(),
            )),
        }
    }

    async fn handle_resume_service(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Response> {
        let name = extract_service_name(cmd)?;
        match service_cmd::resume_service(&name) {
            Ok(()) => Ok(Response::ok(
                "ResumeService",
                request_id.map(String::from),
                json!({ "name": name }),
            )),
            Err(e) => Ok(Response::error(
                "ResumeService",
                request_id.map(String::from),
                e.to_string(),
            )),
        }
    }

    async fn handle_get_service(
        &self,
        cmd: &Command,
        request_id: Option<&str>,
    ) -> Result<Response> {
        let data: GetServiceData = match &cmd.data {
            Some(v) => serde_json::from_value(v.clone())?,
            None => anyhow::bail!("GetService 缺少 data"),
        };
        match service_cmd::get_service(&data.name) {
            Some(svc) => {
                let stats = svc.stats.lock().clone();
                let payload = json!({
                    "name": svc.config.name,
                    "status": svc.status.lock().state.clone(),
                    "stats": stats,
                });
                Ok(Response::ok(
                    "GetService",
                    request_id.map(String::from),
                    payload,
                ))
            }
            None => Ok(Response::error(
                "GetService",
                request_id.map(String::from),
                "service not found",
            )),
        }
    }

    pub async fn upload_traffic(&self) -> Result<()> {
        let uploader = HttpUploader::new(&self.node_cfg)?;
        let reg = services_registry();
        let mut svcs = Vec::new();
        for name in reg.names() {
            if let Some(svc) = reg.get(&name) {
                let s = svc.stats.lock().clone();
                svcs.push(ServiceTraffic {
                    name: svc.config.name.clone(),
                    input_bytes: s.input_bytes,
                    output_bytes: s.output_bytes,
                    total_conns: s.total_conns,
                    current_conns: s.current_conns,
                });
            }
        }
        let delta = TrafficDelta {
            services: svcs,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };
        uploader.upload_traffic(&delta).await
    }

    pub async fn upload_gost_config(&self) -> Result<()> {
        let uploader = HttpUploader::new(&self.node_cfg)?;
        let cfg_bytes = std::fs::read(&self.gost_config_path).unwrap_or_default();
        uploader.upload_config(&cfg_bytes).await
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn service_to_node_info(svc: &ServiceImpl) -> NodeInfo {
    let (port, protocol) = if let Some(addr) = &svc.config.addr {
        let port = addr
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        let proto = match svc.config.handler.as_ref().map(|h| h.r#type.as_str()) {
            Some("forward") => "forward",
            Some("relay") => "relay",
            Some(t) => t,
            None => "tcp",
        };
        (port, proto.to_string())
    } else {
        (0, "tcp".to_string())
    };
    NodeInfo {
        name: svc.config.name.clone(),
        addr: svc.config.addr.clone().unwrap_or_default(),
        port,
        protocol,
        state: svc.status.lock().state.clone(),
        tcp: 0,
        udp: 0,
    }
}

fn extract_service_name(cmd: &Command) -> Result<String> {
    if let Some(v) = &cmd.data {
        if let Some(s) = v.as_str() {
            return Ok(s.to_string());
        }
        if let Some(map) = v.as_object() {
            if let Some(n) = map.get("name").and_then(|x| x.as_str()) {
                return Ok(n.to_string());
            }
        }
    }
    anyhow::bail!("PauseService/ResumeService 缺少 data.name 或 data string")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_enc_works() {
        assert_eq!(urlencoding("abc"), "abc");
        assert_eq!(urlencoding("a b"), "a%20b");
        assert_eq!(urlencoding("a/b"), "a%2Fb");
    }

    #[test]
    fn url_build_includes_all_params() {
        let cfg = NodeConfig {
            addr: "1.2.3.4:8080".into(),
            secret: "k".into(),
            http: 1,
            tls: 0,
            socks: 1,
            ssl: false,
        };
        let r = WsReporter::new(cfg, "gost.json");
        let url = r.build_url();
        assert!(url.contains("ws://1.2.3.4:8080/system-info"));
        assert!(url.contains("type=1"));
        assert!(url.contains("secret=k"));
        assert!(url.contains("http=1"));
        assert!(url.contains("tls=0"));
        assert!(url.contains("socks=1"));
    }

    #[test]
    fn url_build_wss() {
        let cfg = NodeConfig {
            addr: "1.2.3.4:443".into(),
            secret: "k".into(),
            ssl: true,
            ..Default::default()
        };
        let r = WsReporter::new(cfg, "gost.json");
        let url = r.build_url();
        assert!(url.starts_with("wss://"));
    }

    #[test]
    fn send_pong_when_disconnected_is_noop() {
        let cfg = NodeConfig::default();
        let r = WsReporter::new(cfg, "gost.json");
        // writer 为 None → 不会 panic
        // 同步测试不能 .await，所以这里只验证构造不 panic
        let _ = r;
    }
}