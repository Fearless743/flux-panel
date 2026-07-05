use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// ftcp dialer stub（阶段 7）。
pub struct FtcpDialer;

impl FtcpDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for FtcpDialer {
    fn kind(&self) -> &'static str { "ftcp" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'ftcp' is a stub",
        ))
    }
}

impl Default for FtcpDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = FtcpDialer::new();
    }
}
