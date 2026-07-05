//! relay handler。
//!
//! 与 Go 版 `x/handler/relay/handler.go` 对齐：
//! 1. 读 Request 帧（VER + CMD/FLAGS + FEALEN + FEATURES）
//! 2. 提取 user/auth/addr/network
//! 3. 选目标：chain 优先 → 固定目标 fallback
//! 4. 拨号 → 写 Response
//! 5. 双向 copy
//!
//! 与 `cmd & CmdMask` 分发：Connect（默认）/ Bind（暂 stub）。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;

use crate::core::{BoxedStream, Chain, Handler};
use crate::dialer::DirectDialer;
use crate::util::relay::feature::{
    AddrFeature, Feature as _, NetworkFeature, NetworkID, UserAuthFeature,
};
use crate::util::relay::frame::{Cmd, FUDP, Request, Response, Status};

/// Relay handler 选项。
#[derive(Default, Clone)]
pub struct RelayHandlerOptions {
    pub chain: Option<Arc<dyn Chain>>,
    /// 固定目标（与 chain 二选一）
    pub target: Option<String>,
    pub dialer: Option<Arc<dyn crate::core::Dialer>>,
}

pub struct RelayHandler {
    kind: &'static str,
    opts: RelayHandlerOptions,
}

// 手动实现 Clone（opts 已经是 Clone）
impl Clone for RelayHandler {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            opts: self.opts.clone(),
        }
    }
}

impl RelayHandler {
    pub fn new(opts: RelayHandlerOptions) -> Self {
        Self {
            kind: "relay",
            opts,
        }
    }

    /// 完整实现：读 Request → 拨号 → 写 Response → 双向 copy。
    pub async fn handle_full(&self, mut conn: BoxedStream) -> io::Result<()> {
        // 1. 读 Request 帧
        let req = read_request_async(&mut conn).await?;
        tracing::debug!(cmd = req.cmd, "relay: 收到 Request");

        // 2. 解析 features
        let mut addr = AddrFeature::default();
        let mut user: Option<String> = None;
        let mut pass: Option<String> = None;
        let mut network_id = NetworkID::TCP;
        for f in &req.features {
            if let Some(ua) = f.as_any().downcast_ref::<UserAuthFeature>() {
                user = Some(ua.username.clone());
                pass = Some(ua.password.clone());
            } else if let Some(a) = f.as_any().downcast_ref::<AddrFeature>() {
                addr = a.clone();
            } else if let Some(n) = f.as_any().downcast_ref::<NetworkFeature>() {
                network_id = n.network;
            }
        }
        let _ = (user, pass);

        let cmd = req.cmd & 0x0F;
        if cmd == Cmd::Bind as u8 {
            let mut resp = Response::new(Status::InternalServerError);
            resp.write_to_async(&mut conn).await?;
            return Err(io::Error::other("relay: Bind not implemented"));
        }

        // 3. 选目标
        let target_addr = format!("{}:{}", addr.host, addr.port);
        let mut remote: BoxedStream = if let Some(chain) = &self.opts.chain {
            chain.dial().await?.0
        } else if let Some(t) = &self.opts.target {
            let d = self
                .opts
                .dialer
                .clone()
                .unwrap_or_else(|| Arc::new(DirectDialer::new()));
            d.dial(t).await?
        } else {
            let d = self
                .opts
                .dialer
                .clone()
                .unwrap_or_else(|| Arc::new(DirectDialer::new()));
            d.dial(&target_addr).await?
        };
        let _ = (network_id, FUDP); // UDP 等后续阶段处理

        // 4. 写 Response (StatusOK)
        let mut resp = Response::new(Status::Ok);
        resp.write_to_async(&mut conn).await?;

        // 5. 双向 copy
        let _ = tokio::io::copy_bidirectional(&mut conn, &mut remote).await;
        Ok(())
    }
}

#[async_trait]
impl Handler for RelayHandler {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn handle(&self, conn: BoxedStream) -> io::Result<()> {
        self.handle_full(conn).await
    }
}

/// 异步读 Request 帧（AsyncRead）。
async fn read_request_async<R: AsyncReadExt + Unpin>(r: &mut R) -> io::Result<Request> {
    let mut header = [0u8; 4];
    r.read_exact(&mut header).await?;
    if header[0] != crate::util::relay::frame::VERSION_1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad relay version",
        ));
    }
    let flen = u16::from_be_bytes([header[2], header[3]]) as usize;
    let mut buf = vec![0u8; flen];
    r.read_exact(&mut buf).await?;
    let features = crate::util::relay::feature::decode_features(&buf)?;
    Ok(Request {
        version: header[0],
        cmd: header[1],
        features,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::relay::feature::{AddrFeature, UserAuthFeature};
    use crate::util::relay::frame::{Cmd, Request};
    use std::io::Cursor;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn relay_handler_handle_full() {
        // 1. echo server
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let echo_addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = match listener.accept().await {
                    Ok(x) => x,
                    Err(_) => break,
                };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    loop {
                        match s.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if s.write_all(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            }
        });

        // 2. duplex stream 模拟 client conn
        let (client_end, server_end) = duplex(4096);

        let handler = RelayHandler::new(RelayHandlerOptions {
            chain: None,
            target: Some(echo_addr.to_string()),
            dialer: Some(Arc::new(crate::dialer::TcpDialer::new())),
        });

        // 3. 客户端：写 Request → 读 Response → 写 payload → 读回显
        let server_task = tokio::spawn(async move {
            let mut conn = server_end;
            let mut req = Request::new(Cmd::Connect, 0);
            let mut addr = AddrFeature::default();
            addr.parse_from(&echo_addr.to_string()).unwrap();
            req.add_feature(addr);
            req.write_to_async(&mut conn).await.unwrap();
            let resp = Response::read_from_async(&mut conn).await.unwrap();
            assert_eq!(resp.status, Status::Ok.as_byte());
            conn.write_all(b"PING").await.unwrap();
            let mut buf = [0u8; 4];
            conn.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"PING");
        });

        handler.handle_full(Box::new(client_end)).await.unwrap();
        server_task.await.unwrap();
    }

    #[test]
    fn sync_request_round_trip() {
        let mut req = Request::new(Cmd::Connect, 0);
        req.add_feature(UserAuthFeature {
            username: "u".into(),
            password: "p".into(),
        });
        let mut addr = AddrFeature::default();
        addr.parse_from("127.0.0.1:9999").unwrap();
        req.add_feature(addr);
        let mut buf = Vec::new();
        req.write_to(&mut buf).unwrap();
        let mut cur = Cursor::new(buf);
        let parsed = Request::read_from(&mut cur).unwrap();
        assert_eq!(parsed.cmd & 0x0F, Cmd::Connect as u8);
        assert_eq!(parsed.features.len(), 2);
    }
}