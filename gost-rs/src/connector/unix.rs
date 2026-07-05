use async_trait::async_trait;

use crate::core::{BoxedStream, Connector};

/// unix connector stub（阶段 7）。
pub struct UnixConnector;

impl UnixConnector {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Connector for UnixConnector {
    fn kind(&self) -> &'static str { "unix" }
    async fn connect(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "connector 'unix' is a stub",
        ))
    }
}

impl Default for UnixConnector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = UnixConnector::new();
    }
}
