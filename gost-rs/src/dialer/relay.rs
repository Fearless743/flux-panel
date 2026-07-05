//! relay dialer：在已建立的底层 conn 上发起 relay 协议握手。
//!
//! 与 Go 版 `x/dialer/relay/dialer.go` 风格对齐：用于 chain 多跳时，
//! 在已建立的 conn 上发起 relay 协议握手。
//!
//! 与 [`crate::connector::relay::RelayConnector`] 的区别：
//! - Dialer 接受一个已建立的下层 conn（host 风格拨号）
//! - Connector 自行 TCP 拨号到 relay server
//!
//! 实现上用 `tokio::sync::Mutex<Option<BoxedStream>>` 持有底层 conn，
//! dial 时 take 出来用完再放回。

use std::sync::Arc;

use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};
use crate::util::relay::feature::{AddrFeature, NetworkFeature, NetworkID, UserAuthFeature};
use crate::util::relay::frame::{Cmd, Request, Response, Status};

/// Relay dialer 选项。
#[derive(Default, Clone)]
pub struct RelayDialerOptions {
    pub username: Option<String>,
    pub password: Option<String>,
}

/// 内部状态：可选的底层 conn（dial 期间被取出）。
#[derive(Default)]
struct Inner {
    base: Option<BoxedStream>,
}

pub struct RelayDialer {
    kind: &'static str,
    opts: RelayDialerOptions,
    inner: Arc<tokio::sync::Mutex<Inner>>,
}

impl RelayDialer {
    pub fn new(opts: RelayDialerOptions) -> Self {
        Self {
            kind: "relay",
            opts,
            inner: Arc::new(tokio::sync::Mutex::new(Inner::default())),
        }
    }

    /// 用一个已建立的下层连接构造（chain 中间跳用）。
    pub fn with_base(opts: RelayDialerOptions, base: BoxedStream) -> Self {
        let inner = Inner { base: Some(base) };
        Self {
            kind: "relay",
            opts,
            inner: Arc::new(tokio::sync::Mutex::new(inner)),
        }
    }
}

#[async_trait]
impl Dialer for RelayDialer {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn dial(&self, target: &str) -> std::io::Result<BoxedStream> {
        // 取出底层 conn
        let mut conn = {
            let mut guard = self.inner.lock().await;
            guard
                .base
                .take()
                .ok_or_else(|| std::io::Error::other("relay dialer: base conn not set"))?
        };

        // 写 Request
        let mut req = Request::new(Cmd::Connect, 0);
        if let (Some(u), Some(p)) = (&self.opts.username, &self.opts.password) {
            req.add_feature(UserAuthFeature {
                username: u.clone(),
                password: p.clone(),
            });
        }
        let mut addr = AddrFeature::default();
        addr.parse_from(target)
            .map_err(|e| std::io::Error::other(format!("addr parse {}: {}", target, e)))?;
        req.add_feature(addr);
        req.add_feature(NetworkFeature {
            network: NetworkID::TCP,
        });
        req.write_to_async(&mut conn).await?;

        // 读 Response
        let resp = Response::read_from_async(&mut conn).await?;
        if resp.status != Status::Ok.as_byte() {
            // 失败：把 conn 放回（虽然已损坏，但保持状态一致）
            let mut guard = self.inner.lock().await;
            guard.base = Some(conn);
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("relay dialer: server status {}", resp.status),
            ));
        }
        // 成功：conn 由调用方接管（不再放回 inner）
        Ok(conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn relay_dialer_handshake() {
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

        // 2. duplex 模拟已建立的 base conn 的两边
        let (client_side, server_side) = duplex(4096);

        // 3. server_side 协程：读 Request → 写 Response OK → 双向转发到 echo server
        let echo_target = echo_addr.to_string();
        tokio::spawn(async move {
            let mut s = server_side;
            let _ = Request::read_from_async(&mut s).await;
            let resp = Response::ok();
            let _ = resp.write_to_async(&mut s).await;
            let upstream = tokio::net::TcpStream::connect(&echo_target).await.unwrap();
            let mut upboxed: BoxedStream = Box::new(upstream);
            let mut client_boxed: BoxedStream = Box::new(s);
            let _ = tokio::io::copy_bidirectional(&mut client_boxed, &mut upboxed).await;
        });

        // 4. dialer 用 client_side 作为 base
        let dialer = RelayDialer::with_base(RelayDialerOptions::default(), Box::new(client_side));
        let mut conn = dialer.dial(&echo_addr.to_string()).await.unwrap();

        // 5. echo 验证
        conn.write_all(b"RELAY-DIALER").await.unwrap();
        let mut buf = [0u8; 12];
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            conn.read_exact(&mut buf),
        )
        .await
        .expect("echo 应在 2s 内返回")
        .unwrap();
        assert_eq!(n, 12);
        assert_eq!(&buf, b"RELAY-DIALER");
    }

    #[test]
    fn kind_returns_relay() {
        let d = RelayDialer::new(RelayDialerOptions::default());
        assert_eq!(d.kind(), "relay");
    }

    #[tokio::test]
    async fn base_missing_errors() {
        let d = RelayDialer::new(RelayDialerOptions::default());
        let res = d.dial("anywhere:0").await;
        assert!(res.is_err());
    }
}