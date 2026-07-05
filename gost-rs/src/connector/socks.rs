use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// socks connector stub（阶段 7）。
pub struct SocksConnector;

impl SocksConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for SocksConnector {
    fn kind(&self) -> &'static str { "socks" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'socks' is a stub",
        ))
    }
}

impl Default for SocksConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = SocksConnector::new();
    }
}
