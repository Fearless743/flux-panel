use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// http2 dialer stub（阶段 7）。
pub struct Http2Dialer;

impl Http2Dialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for Http2Dialer {
    fn kind(&self) -> &'static str { "http2" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'http2' is a stub",
        ))
    }
}

impl Default for Http2Dialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = Http2Dialer::new();
    }
}
