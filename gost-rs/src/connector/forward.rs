use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// forward connector stub（阶段 7）。
pub struct ForwardConnector;

impl ForwardConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for ForwardConnector {
    fn kind(&self) -> &'static str { "forward" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'forward' is a stub",
        ))
    }
}

impl Default for ForwardConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = ForwardConnector::new();
    }
}
