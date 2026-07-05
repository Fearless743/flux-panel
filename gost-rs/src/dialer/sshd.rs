use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// sshd dialer stub（阶段 7）。
pub struct SshdDialer;

impl SshdDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for SshdDialer {
    fn kind(&self) -> &'static str { "sshd" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'sshd' is a stub",
        ))
    }
}

impl Default for SshdDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = SshdDialer::new();
    }
}
