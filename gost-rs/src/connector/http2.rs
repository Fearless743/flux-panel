use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// http2 connector stub（阶段 7）。
pub struct Http2Connector;

impl Http2Connector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for Http2Connector {
    fn kind(&self) -> &'static str { "http2" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'http2' is a stub",
        ))
    }
}

impl Default for Http2Connector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = Http2Connector::new();
    }
}
