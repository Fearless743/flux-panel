//! ssh listener（阶段 6 stub）：客户端 ssh dialer。

use async_trait::async_trait;

use crate::core::{BoxedStream, Listener};

pub struct SshListenerImpl;

impl SshListenerImpl {
    pub async fn bind(_addr: &str) -> std::io::Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl Listener for SshListenerImpl {
    fn kind(&self) -> &'static str {
        "ssh"
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        Err(crate::listener::stub_error("ssh"))
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
        let l = SshListenerImpl::bind("127.0.0.1:0").await.unwrap();
        assert_eq!(l.kind(), "ssh");
    }
}
