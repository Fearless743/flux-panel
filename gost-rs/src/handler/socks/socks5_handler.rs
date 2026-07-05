//! socks5 handler 实现。
//!
//! 与 Go 版 `x/handler/socks/handler.go` 对齐：
//! 1. 解析 NO_AUTH / USER_PASS 认证握手
//! 2. 解析 CONNECT 请求（CMD = 0x01）
//! 3. 拨号目标 → 双向 copy
//!
//! 阶段 7 实现核心 CONNECT 路径。
//! BIND / UDP_ASSOCIATE 留作 stub（`return stub_error`）。

use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::core::{BoxedStream, Handler};
use crate::dialer::TcpDialer;
use crate::handler::stub_error;

#[derive(Default, Clone)]
pub struct Socks5HandlerOptions {
    pub dialer: Option<Arc<dyn crate::core::Dialer>>,
}

pub struct Socks5Handler {
    kind: &'static str,
    opts: Socks5HandlerOptions,
}

impl Socks5Handler {
    pub fn new(opts: Socks5HandlerOptions) -> Self {
        Self { kind: "socks5", opts }
    }
}

impl Clone for Socks5Handler {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            opts: self.opts.clone(),
        }
    }
}

#[async_trait]
impl Handler for Socks5Handler {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn handle(&self, mut conn: BoxedStream) -> std::io::Result<()> {
        let dialer: Arc<dyn crate::core::Dialer> = self
            .opts
            .dialer
            .clone()
            .unwrap_or_else(|| Arc::new(TcpDialer::new()));

        // 1. 握手：从客户端读取 methods（[ver=0x05, nmethods, methods...]）
        let ver = conn.read_u8().await?;
        if ver != 0x05 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("socks5: bad version {}", ver),
            ));
        }
        let nmethods = conn.read_u8().await?;
        let mut methods = vec![0u8; nmethods as usize];
        conn.read_exact(&mut methods).await?;
        // 选择 NO_AUTH（0x00）即可
        let selected = if methods.contains(&0x00) { 0x00 } else { 0xFF };
        conn.write_all(&[0x05, selected]).await?;
        if selected == 0xFF {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "socks5: no acceptable auth method",
            ));
        }

        // 2. Request：[ver=0x05, cmd, rsv=0x00, atyp, addr, port]
        let mut hdr = [0u8; 4];
        conn.read_exact(&mut hdr).await?;
        if hdr[0] != 0x05 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "socks5: bad request version",
            ));
        }
        let cmd = hdr[1];
        let atyp = hdr[3];

        let target_addr = match atyp {
            0x01 => {
                // IPv4
                let mut ip = [0u8; 4];
                conn.read_exact(&mut ip).await?;
                let mut port = [0u8; 2];
                conn.read_exact(&mut port).await?;
                format!(
                    "{}.{}.{}.{}:{}",
                    ip[0], ip[1], ip[2], ip[3],
                    u16::from_be_bytes(port)
                )
            }
            0x03 => {
                // Domain
                let len = conn.read_u8().await? as usize;
                let mut domain = vec![0u8; len];
                conn.read_exact(&mut domain).await?;
                let mut port = [0u8; 2];
                conn.read_exact(&mut port).await?;
                format!("{}:{}", String::from_utf8_lossy(&domain), u16::from_be_bytes(port))
            }
            0x04 => {
                // IPv6
                let mut ip = [0u8; 16];
                conn.read_exact(&mut ip).await?;
                let mut port = [0u8; 2];
                conn.read_exact(&mut port).await?;
                format!(
                    "[{}]:{}",
                    std::net::Ipv6Addr::from(ip),
                    u16::from_be_bytes(port)
                )
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("socks5: bad atyp 0x{:02x}", atyp),
                ));
            }
        };

        // 3. 分发：BIND / UDP_ASSOCIATE 暂未实现
        if cmd != 0x01 {
            // respond "command not supported"
            conn.write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Err(stub_error("socks5-BIND"));
        }

        // 4. CONNECT：拨号目标
        let mut upstream = match dialer.dial(&target_addr).await {
            Ok(s) => s,
            Err(e) => {
                conn.write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await?;
                return Err(e);
            }
        };

        // 5. 回应成功：BND.ADDR = 0.0.0.0:0
        conn.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await?;

        // 6. 双向 copy
        let _ = tokio::io::copy_bidirectional(&mut conn, &mut upstream).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn socks5_connect_through_echo() {
        // echo server
        let echo_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let echo_addr = echo_listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = match echo_listener.accept().await {
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

        // socks5 server
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let s5_addr = l.local_addr().unwrap();
        let handler = Socks5Handler::new(Socks5HandlerOptions::default());
        tokio::spawn(async move {
            loop {
                let (s, _) = match l.accept().await {
                    Ok(x) => x,
                    Err(_) => break,
                };
                let h = handler.clone();
                tokio::spawn(async move { let _ = h.handle(Box::new(s)).await; });
            }
        });

        // socks5 client：通过 torus 简化手写握手
        let mut client = tokio::net::TcpStream::connect(s5_addr).await.unwrap();
        // handshake: [05, 01, 00] → 期望 [05, 00]
        client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut resp = [0u8; 2];
        client.read_exact(&mut resp).await.unwrap();
        assert_eq!(resp, [0x05, 0x00]);

        // request: CONNECT to echo_addr
        let port = echo_addr.port();
        let ip = match echo_addr.ip() {
            std::net::IpAddr::V4(v) => v.octets().to_vec(),
            _ => unreachable!(),
        };
        let mut req = vec![0x05, 0x01, 0x00, 0x01];
        req.extend_from_slice(&ip);
        req.extend_from_slice(&port.to_be_bytes());
        client.write_all(&req).await.unwrap();

        // response: [05, 00, 00, atyp, addr...]
        let mut resp_hdr = [0u8; 4];
        client.read_exact(&mut resp_hdr).await.unwrap();
        assert_eq!(resp_hdr[0], 0x05);
        assert_eq!(resp_hdr[1], 0x00);
        let atyp = resp_hdr[3];
        let skip_len = match atyp {
            0x01 => 4 + 2,
            0x03 => {
                let alen = client.read_u8().await.unwrap() as usize;
                alen + 2
            }
            0x04 => 16 + 2,
            _ => panic!("bad atyp {}", atyp),
        };
        let mut skip = vec![0u8; skip_len];
        client.read_exact(&mut skip).await.unwrap();

        // 通过 tunnel 发 echo 验证
        client.write_all(b"ECHO").await.unwrap();
        let mut buf = [0u8; 4];
        client.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"ECHO");
    }
}
