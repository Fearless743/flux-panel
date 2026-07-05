use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// grpc dialer stub（阶段 7）。
pub struct GrpcDialer;

impl GrpcDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for GrpcDialer {
    fn kind(&self) -> &'static str { "grpc" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'grpc' is a stub",
        ))
    }
}

impl Default for GrpcDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = GrpcDialer::new();
    }
}
