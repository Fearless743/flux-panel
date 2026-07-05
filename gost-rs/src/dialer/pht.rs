use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// pht dialer stub（阶段 7）。
pub struct PhtDialer;

impl PhtDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for PhtDialer {
    fn kind(&self) -> &'static str { "pht" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'pht' is a stub",
        ))
    }
}

impl Default for PhtDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = PhtDialer::new();
    }
}
