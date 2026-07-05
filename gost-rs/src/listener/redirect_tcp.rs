//! redirect/tcp listener（阶段 6 stub）：客户端 NAT 重定向。

use async_trait::async_trait;

use crate::core::{BoxedStream, Listener};

pub struct RedirectTcpListenerImpl;

impl RedirectTcpListenerImpl {
    pub async fn bind(_addr: &str) -> std::io::Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl Listener for RedirectTcpListenerImpl {
    fn kind(&self) -> &'static str {
        "redirect/tcp"
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        Err(crate::listener::stub_error("redirect/tcp"))
    }
    async fn close(&self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn construct_succeeds() {
        let l = RedirectTcpListenerImpl::bind("127.0.0.1:0").await.unwrap();
        assert_eq!(l.kind(), "redirect/tcp");
    }
}
