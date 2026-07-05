use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// mtls dialer stub（阶段 7）。
pub struct MtlsDialer;

impl MtlsDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for MtlsDialer {
    fn kind(&self) -> &'static str { "mtls" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'mtls' is a stub",
        ))
    }
}

impl Default for MtlsDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = MtlsDialer::new();
    }
}
