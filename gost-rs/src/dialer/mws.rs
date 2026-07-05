use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// mws dialer stub（阶段 7）。
pub struct MwsDialer;

impl MwsDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for MwsDialer {
    fn kind(&self) -> &'static str { "mws" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'mws' is a stub",
        ))
    }
}

impl Default for MwsDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = MwsDialer::new();
    }
}
