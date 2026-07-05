use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// ss connector stub（阶段 7）。
pub struct SsConnector;

impl SsConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for SsConnector {
    fn kind(&self) -> &'static str { "ss" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'ss' is a stub",
        ))
    }
}

impl Default for SsConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = SsConnector::new();
    }
}
