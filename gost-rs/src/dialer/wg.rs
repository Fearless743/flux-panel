use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// wg dialer stub（阶段 7）。
pub struct WgDialer;

impl WgDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for WgDialer {
    fn kind(&self) -> &'static str { "wg" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'wg' is a stub",
        ))
    }
}

impl Default for WgDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = WgDialer::new();
    }
}
