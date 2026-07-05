use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// dtls dialer stub（阶段 7）。
pub struct DtlsDialer;

impl DtlsDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for DtlsDialer {
    fn kind(&self) -> &'static str { "dtls" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'dtls' is a stub",
        ))
    }
}

impl Default for DtlsDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = DtlsDialer::new();
    }
}
