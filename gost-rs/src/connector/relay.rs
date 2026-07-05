//! relay connector。
//!
//! 与 Go 版 `x/connector/relay/connector.go` 对齐：
//! 1. TCP 拨号到目标 relay server
//! 2. 写 Request 帧（VER + CMD/FLAGS + Features）
//! 3. 读 Response 帧（Status=OK 表示已建立）
//! 4. 返回包装的 conn（之后可直接用于代理流量）
//!
//! Features:
//! - UserAuth (可选)
//! - Addr (target)
//! - Network (tcp/udp/unix)

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use crate::core::{BoxedStream, Connector};
use crate::util::relay::feature::{AddrFeature, NetworkFeature, NetworkID, UserAuthFeature};
use crate::util::relay::frame::{Cmd, Request, Response, Status};

/// Relay connector 选项。
#[derive(Default, Clone)]
pub struct RelayConnectorOptions {
    /// relay server 地址，例如 `1.2.3.4:8443`。
    pub server: String,
    /// 可选用户名/密码。
    pub username: Option<String>,
    pub password: Option<String>,
    /// connect 超时（秒）。
    pub timeout_secs: u64,
}

pub struct RelayConnector {
    opts: RelayConnectorOptions,
}

impl RelayConnector {
    pub fn new(opts: RelayConnectorOptions) -> Self {
        Self { opts }
    }
}

#[async_trait]
impl Connector for RelayConnector {
    fn kind(&self) -> &'static str {
        "relay"
    }

    async fn connect(&self, target: &str) -> std::io::Result<BoxedStream> {
        // 1. 拨号 relay server
        let stream = if self.opts.timeout_secs > 0 {
            let timeout = std::time::Duration::from_secs(self.opts.timeout_secs);
            match tokio::time::timeout(timeout, TcpStream::connect(&self.opts.server)).await {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => {
                    return Err(std::io::Error::new(
                        e.kind(),
                        format!("relay connect {}: {}", self.opts.server, e),
                    ));
                }
                Err(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("relay connect {} timeout", self.opts.server),
                    ));
                }
            }
        } else {
            TcpStream::connect(&self.opts.server).await?
        };

        // 2. 写 Request
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

        let mut boxed: BoxedStream = Box::new(stream);
        req.write_to_async(&mut boxed).await?;

        // 3. 读 Response
        let mut resp = Response::read_from_async(&mut boxed).await?;
        if resp.status != Status::Ok.as_byte() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("relay server status {}", resp.status),
            ));
        }
        let _ = resp;

        // 4. 已建立，返回 conn（后续流量由调用方处理）
        Ok(boxed)
    }
}

/// 写 Request 后同步 read_exact 头部 4 字节以验证 handshake（用 sync Read）。
pub async fn write_request_and_wait<R>(conn: &mut R, req: &Request) -> std::io::Result<Response>
where
    R: AsyncReadExt + Unpin + tokio::io::AsyncWriteExt,
{
    req.write_to_async(conn).await?;
    Response::read_from_async(conn).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Handler;
    use crate::handler::relay::{RelayHandler, RelayHandlerOptions};
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    /// 端到端：connector 拨号到 relay server → relay server 转发到 echo server。
    #[tokio::test]
    async fn relay_connector_end_to_end() {
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

        // 2. 真实 relay server：accept → 读 Request → 写 Response OK → 把后续字节双向转发到 echo_addr
        let relay_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let relay_addr = relay_listener.local_addr().unwrap();
        let echo_addr_str = echo_addr.to_string();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = match relay_listener.accept().await {
                    Ok(x) => x,
                    Err(_) => break,
                };
                let echo_target = echo_addr_str.clone();
                tokio::spawn(async move {
                    // 读 Request
                    let _ = Request::read_from_async(&mut s).await;
                    // 写 Response OK
                    let mut resp = Response::ok();
                    let _ = resp.write_to_async(&mut s).await;
                    // 拨号到 echo server
                    let upstream = match tokio::net::TcpStream::connect(&echo_target).await {
                        Ok(c) => c,
                        Err(_) => return,
                    };
                    let mut upboxed: BoxedStream = Box::new(upstream);
                    let mut client_boxed: BoxedStream = Box::new(s);
                    let _ = tokio::io::copy_bidirectional(&mut client_boxed, &mut upboxed).await;
                });
            }
        });

        // 3. 用 connector 拨号到 relay_addr → 目标填 echo_addr（信息给 server 用于转发，实际上 server 已固定转发到 echo_addr）
        let conn = RelayConnector::new(RelayConnectorOptions {
            server: relay_addr.to_string(),
            timeout_secs: 5,
            ..Default::default()
        });
        let mut stream = conn.connect(&echo_addr.to_string()).await.unwrap();

        // 4. 通过 stream 发 echo 验证
        stream.write_all(b"ECHO-ME").await.unwrap();
        let mut buf = [0u8; 7];
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read_exact(&mut buf),
        )
        .await
        .expect("echo 应在 2s 内返回")
        .unwrap();
        assert_eq!(n, 7);
        assert_eq!(&buf, b"ECHO-ME");
    }

    #[test]
    fn kind_returns_relay() {
        let c = RelayConnector::new(RelayConnectorOptions::default());
        assert_eq!(c.kind(), "relay");
    }
}