use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// unix dialer stub（阶段 7）。
pub struct UnixDialer;

impl UnixDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for UnixDialer {
    fn kind(&self) -> &'static str { "unix" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'unix' is a stub",
        ))
    }
}

impl Default for UnixDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = UnixDialer::new();
    }
}
