use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// sni connector stub（阶段 7）。
pub struct SniConnector;

impl SniConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for SniConnector {
    fn kind(&self) -> &'static str { "sni" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'sni' is a stub",
        ))
    }
}

impl Default for SniConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = SniConnector::new();
    }
}
