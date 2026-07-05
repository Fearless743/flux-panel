use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// ws dialer stub（阶段 7）。
pub struct WsDialer;

impl WsDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for WsDialer {
    fn kind(&self) -> &'static str { "ws" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'ws' is a stub",
        ))
    }
}

impl Default for WsDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = WsDialer::new();
    }
}
