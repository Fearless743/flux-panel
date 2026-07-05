//! dtls listener（阶段 6 stub）。
//!
//! 与 Go 版 `x/listener/dtls/listener.go` 对齐：编译通过 + 类型齐全；
//! 运行时返回 `Unsupported` 错误。

use async_trait::async_trait;

use crate::core::{BoxedStream, Listener};

/// stub listener。
pub struct DtlsListenerImpl;

impl DtlsListenerImpl {
    pub async fn bind(_addr: &str) -> std::io::Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl Listener for DtlsListenerImpl {
    fn kind(&self) -> &'static str {
        "dtls"
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        Err(crate::listener::stub_error("dtls"))
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
        let l = DtlsListenerImpl::bind("127.0.0.1:0").await.unwrap();
        assert_eq!(l.kind(), "dtls");
    }

    #[tokio::test]
    async fn accept_returns_unsupported() {
        let l = DtlsListenerImpl::bind("127.0.0.1:0").await.unwrap();
        let res = l.accept().await;
        assert!(res.is_err());
    }
}
