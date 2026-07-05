use async_trait::async_trait;

use crate::core::{BoxedStream, Dialer};

/// obfs dialer stub（阶段 7）。
pub struct ObfsDialer;

impl ObfsDialer {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Dialer for ObfsDialer {
    fn kind(&self) -> &'static str { "obfs" }
    async fn dial(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "dialer 'obfs' is a stub",
        ))
    }
}

impl Default for ObfsDialer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let _ = ObfsDialer::new();
    }
}
