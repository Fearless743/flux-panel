use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// tls dialer stub（阶段 7）。
pub struct TlsDialer;

impl TlsDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for TlsDialer {
    fn kind(&self) -> &'static str { "tls" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'tls' is a stub",
        ))
    }
}

impl Default for TlsDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = TlsDialer::new();
    }
}
