//! http2 listener（阶段 6 stub）。

use async_trait::async_trait;

use crate::core::{BoxedStream, Listener};

pub struct Http2ListenerImpl;

impl Http2ListenerImpl {
    pub async fn bind(_addr: &str) -> std::io::Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl Listener for Http2ListenerImpl {
    fn kind(&self) -> &'static str {
        "http2"
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        Err(crate::listener::stub_error("http2"))
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
        let l = Http2ListenerImpl::bind("127.0.0.1:0").await.unwrap();
        assert_eq!(l.kind(), "http2");
    }
}
