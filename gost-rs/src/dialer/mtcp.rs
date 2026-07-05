use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// mtcp dialer stub（阶段 7）。
pub struct MtcpDialer;

impl MtcpDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for MtcpDialer {
    fn kind(&self) -> &'static str { "mtcp" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'mtcp' is a stub",
        ))
    }
}

impl Default for MtcpDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = MtcpDialer::new();
    }
}
