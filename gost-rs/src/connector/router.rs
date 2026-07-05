use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// router connector stub（阶段 7）。
pub struct RouterConnector;

impl RouterConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for RouterConnector {
    fn kind(&self) -> &'static str { "router" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'router' is a stub",
        ))
    }
}

impl Default for RouterConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = RouterConnector::new();
    }
}
