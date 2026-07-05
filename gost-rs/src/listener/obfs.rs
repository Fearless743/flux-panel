//! obfs listener（阶段 6 stub）。

use async_trait::async_trait;

use crate::core::{BoxedStream, Listener};

pub struct ObfsHttpListenerImpl;

impl ObfsHttpListenerImpl {
    pub async fn bind(_addr: &str) -> std::io::Result<Self> {
        Ok(Self)
    }
}

pub struct ObfsTlsListenerImpl;

impl ObfsTlsListenerImpl {
    pub async fn bind(_addr: &str) -> std::io::Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl Listener for ObfsHttpListenerImpl {
    fn kind(&self) -> &'static str {
        "obfs-http"
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        Err(crate::listener::stub_error("obfs-http"))
    }
    async fn close(&self) -> std::io::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl Listener for ObfsTlsListenerImpl {
    fn kind(&self) -> &'static str {
        "obfs-tls"
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        Err(crate::listener::stub_error("obfs-tls"))
    }
    async fn close(&self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn both_construct() {
        let _ = ObfsHttpListenerImpl::bind("127.0.0.1:0").await.unwrap();
        let _ = ObfsTlsListenerImpl::bind("127.0.0.1:0").await.unwrap();
    }
}
