//! tcp connector。
//!
//! 与 Go 版 `x/connector/tcp/connector.go` 对齐：TCP connect + 简单校验。

use async_trait::async_trait;

use crate::core::{BoxedStream, Connector, Dialer};
use crate::dialer::TcpDialer;

pub struct TcpConnector;

impl TcpConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TcpConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for TcpConnector {
    fn kind(&self) -> &'static str {
        "tcp"
    }

    async fn connect(&self, addr: &str) -> std::io::Result<BoxedStream> {
        TcpDialer::new().dial(addr).await
    }
}