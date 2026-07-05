use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// sshd connector stub（阶段 7）。
pub struct SshdConnector;

impl SshdConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for SshdConnector {
    fn kind(&self) -> &'static str { "sshd" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'sshd' is a stub",
        ))
    }
}

impl Default for SshdConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = SshdConnector::new();
    }
}
