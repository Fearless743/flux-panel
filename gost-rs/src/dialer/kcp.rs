use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// kcp dialer stub（阶段 7）。
pub struct KcpDialer;

impl KcpDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for KcpDialer {
    fn kind(&self) -> &'static str { "kcp" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'kcp' is a stub",
        ))
    }
}

impl Default for KcpDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = KcpDialer::new();
    }
}
