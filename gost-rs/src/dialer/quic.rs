use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// quic dialer stub（阶段 7）。
pub struct QuicDialer;

impl QuicDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for QuicDialer {
    fn kind(&self) -> &'static str { "quic" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'quic' is a stub",
        ))
    }
}

impl Default for QuicDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = QuicDialer::new();
    }
}
