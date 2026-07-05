use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// ssh dialer stub（阶段 7）。
pub struct SshDialer;

impl SshDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for SshDialer {
    fn kind(&self) -> &'static str { "ssh" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'ssh' is a stub",
        ))
    }
}

impl Default for SshDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = SshDialer::new();
    }
}
