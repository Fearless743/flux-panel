//! ftcp listener（阶段 6 stub）：fake tcp = 等同于 tcp。

use async_trait::async_trait;

use crate::core::{BoxedStream, Listener};

pub struct FtcpListenerImpl {
    inner: crate::listener::tcp::TcpListenerImpl,
}

impl FtcpListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        Ok(Self {
            inner: crate::listener::tcp::TcpListenerImpl::bind(addr).await?,
        })
    }
}

#[async_trait]
impl Listener for FtcpListenerImpl {
    fn kind(&self) -> &'static str {
        "ftcp"
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        self.inner.accept().await
    }
    async fn close(&self) -> std::io::Result<()> {
        self.inner.close().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn construct_succeeds() {
        let l = FtcpListenerImpl::bind("127.0.0.1:0").await.unwrap();
        assert_eq!(l.kind(), "ftcp");
    }
}
