//! direct dialer：按 addr scheme 自动选择 tcp/udp 直连。
//!
//! 与 Go 版 `x/dialer/direct/dialer.go` 对齐。

use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

pub struct DirectDialer;

impl DirectDialer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DirectDialer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Dialer for DirectDialer {
    fn kind(&self) -> &'static str {
        "direct"
    }

    async fn dial(&self, addr: &str) -> std::io::Result<BoxedStream> {
        // 简单策略：UDP if addr 中包含 "udp://"，否则 TCP
        if addr.starts_with("udp://") {
            let real = addr.trim_start_matches("udp://");
            let _ = real;
            // 包装为 stream；UdpSocket 本身实现 AsyncRead/AsyncWrite 需要 trait
            // 这里直接返回错误（forward handler 应使用专门 udp dialer）
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "udp:// scheme not supported by direct dialer in phase 2; use UdpDialer",
            ))
        } else {
            let stream = tokio::net::TcpStream::connect(addr).await?;
            let _ = stream.set_nodelay(true);
            Ok(Box::new(stream))
        }
    }
}