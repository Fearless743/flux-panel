//! tcp dialer。
//!
//! 与 Go 版 `x/dialer/tcp/dialer.go` 对齐：直接 TCP 拨号，可选 SO_MARK。

use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

pub struct TcpDialer;

impl TcpDialer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TcpDialer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Dialer for TcpDialer {
    fn kind(&self) -> &'static str {
        "tcp"
    }

    async fn dial(&self, addr: &str) -> std::io::Result<BoxedStream> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let _ = stream.set_nodelay(true);
        Ok(Box::new(stream))
    }
}