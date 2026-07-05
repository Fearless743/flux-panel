use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// tunnel connector stub（阶段 7）。
pub struct TunnelConnector;

impl TunnelConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for TunnelConnector {
    fn kind(&self) -> &'static str { "tunnel" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'tunnel' is a stub",
        ))
    }
}

impl Default for TunnelConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = TunnelConnector::new();
    }
}
