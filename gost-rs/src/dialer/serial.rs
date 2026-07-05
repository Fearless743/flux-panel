use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// serial dialer stub（阶段 7）。
pub struct SerialDialer;

impl SerialDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for SerialDialer {
    fn kind(&self) -> &'static str { "serial" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'serial' is a stub",
        ))
    }
}

impl Default for SerialDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = SerialDialer::new();
    }
}
