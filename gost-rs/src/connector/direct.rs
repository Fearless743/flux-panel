//! direct connector。
//!
//! 与 Go 版 `x/connector/direct/connector.go` 对齐：与 dialer 直连等价，
//! 不做任何协议握手。

use async_trait::async_trait;

use crate::core::{BoxedStream, Connector, Dialer};
use crate::dialer::DirectDialer;

pub struct DirectConnector;

impl DirectConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DirectConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for DirectConnector {
    fn kind(&self) -> &'static str {
        "direct"
    }

    async fn connect(&self, addr: &str) -> std::io::Result<BoxedStream> {
        DirectDialer::new().dial(addr).await
    }
}