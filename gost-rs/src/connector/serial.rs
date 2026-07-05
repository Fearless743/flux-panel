use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// serial connector stub（阶段 7）。
pub struct SerialConnector;

impl SerialConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for SerialConnector {
    fn kind(&self) -> &'static str { "serial" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'serial' is a stub",
        ))
    }
}

impl Default for SerialConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = SerialConnector::new();
    }
}
