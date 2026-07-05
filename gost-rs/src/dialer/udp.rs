//! udp dialer。
//!
//! 阶段 2 简化版：
//! tokio 1.52 的 `UdpSocket` 不直接实现 `AsyncRead`/`AsyncWrite`，
//! 而 `into_split()` 需要 nightly 或 feature 不可用。
//!
//! 因此阶段 2 的 UDP dialer 仅返回 **错误**，由调用方使用 UdpDialerConn（独立 UDP 转发器）。
//! 阶段 6 重写为完整 UDP forward 处理器。

use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

pub struct UdpDialer;

impl UdpDialer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UdpDialer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Dialer for UdpDialer {
    fn kind(&self) -> &'static str {
        "udp"
    }

    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "UdpDialer: phase 2 stub; use UdpForwarder (stage 6) or chain.connect via direct",
        ))
    }
}