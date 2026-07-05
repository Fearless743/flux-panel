//! mtcp listener（阶段 6 stub）：mux TCP = 等价于 TCP，协议层后续。

use async_trait::async_trait;

use crate::core::{BoxedStream, Listener};

pub struct MtcpListenerImpl {
    inner: crate::listener::tcp::TcpListenerImpl,
}

impl MtcpListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        Ok(Self {
            inner: crate::listener::tcp::TcpListenerImpl::bind(addr).await?,
        })
    }
}

#[async_trait]
impl Listener for MtcpListenerImpl {
    fn kind(&self) -> &'static str {
        "mtcp"
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
        let l = MtcpListenerImpl::bind("127.0.0.1:0").await.unwrap();
        assert_eq!(l.kind(), "mtcp");
    }
}
