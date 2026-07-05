use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// http connector stub（阶段 7）。
pub struct HttpConnector;

impl HttpConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for HttpConnector {
    fn kind(&self) -> &'static str { "http" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'http' is a stub",
        ))
    }
}

impl Default for HttpConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = HttpConnector::new();
    }
}
