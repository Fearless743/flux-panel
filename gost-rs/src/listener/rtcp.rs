//! rtcp listener：语义为"remote TCP"的 TCP listener。
//!
//! 与 Go 版 `x/listener/rtcp/listener.go` 对齐：实现与 TCP 一样，
//! 仅用于 chain 路由时的语义区分。

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::TcpListener;
use tokio::sync::Notify;

use crate::core::{BoxedStream, Listener};

pub struct RtcpListenerImpl {
    kind: &'static str,
    inner: TcpListener,
    addr: SocketAddr,
    close_notify: Arc<Notify>,
    closed: Arc<AtomicBool>,
}

impl RtcpListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        let inner = TcpListener::bind(addr).await?;
        let local = inner.local_addr()?;
        Ok(Self {
            kind: "rtcp",
            inner,
            addr: local,
            close_notify: Arc::new(Notify::new()),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

#[async_trait]
impl Listener for RtcpListenerImpl {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn accept(&self) -> std::io::Result<BoxedStream> {
        if self.closed.load(Ordering::Acquire) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "listener closed",
            ));
        }
        let accept_fut = self.inner.accept();
        tokio::select! {
            biased;
            _ = self.close_notify.notified() => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "listener closed",
                ))
            }
            res = accept_fut => {
                let (stream, _peer) = res?;
                let _ = stream.set_nodelay(true);
                Ok(Box::new(stream))
            }
        }
    }

    async fn close(&self) -> std::io::Result<()> {
        self.closed.store(true, Ordering::Release);
        self.close_notify.notify_waiters();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn rtcp_accept() {
        let l = RtcpListenerImpl::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr();
        let handle = tokio::spawn(async move { l.accept().await });
        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"hi").await.unwrap();
        let mut server = handle.await.unwrap().unwrap();
        let mut buf = [0u8; 2];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hi");
    }
}
