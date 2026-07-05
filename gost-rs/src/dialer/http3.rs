use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// http3 dialer stub（阶段 7）。
pub struct Http3Dialer;

impl Http3Dialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for Http3Dialer {
    fn kind(&self) -> &'static str { "http3" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'http3' is a stub",
        ))
    }
}

impl Default for Http3Dialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = Http3Dialer::new();
    }
}
