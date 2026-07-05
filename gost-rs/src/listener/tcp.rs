//! TCP listener。
//!
//! 与 Go 版 `x/listener/tcp/listener.go` 对齐：
//! - bind `addr`
//! - accept 出 [`tokio::net::TcpStream`]
//! - 可选 SO_REUSEADDR / interface 绑定（接口绑定阶段 5 再加，依赖 rtnetlink）

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::TcpListener;
use tokio::sync::Notify;

use crate::core::{BoxedStream, Listener};

/// TCP listener 实现。
pub struct TcpListenerImpl {
    kind: &'static str,
    inner: TcpListener,
    addr: SocketAddr,
    close_notify: Arc<Notify>,
    closed: Arc<AtomicBool>,
}

impl TcpListenerImpl {
    /// 绑定 addr 并构造 listener。
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        let inner = TcpListener::bind(addr).await?;
        let local = inner.local_addr()?;
        Ok(Self {
            kind: "tcp",
            inner,
            addr: local,
            close_notify: Arc::new(Notify::new()),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }

    /// 绑定的本地地址。
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

#[async_trait]
impl Listener for TcpListenerImpl {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn accept(&self) -> std::io::Result<BoxedStream> {
        // 已关闭则立即返回错误
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
                // 关闭 Nagle（小包合并）对代理影响不大，关闭即可
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
    async fn accept_returns_stream() {
        let listener = TcpListenerImpl::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr();

        let handle = tokio::spawn(async move {
            listener.accept().await
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"hello").await.unwrap();

        let mut server = handle.await.unwrap().unwrap();
        let mut buf = [0u8; 5];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
    }

    #[tokio::test]
    async fn close_signals_pending_accept() {
        let listener = Arc::new(TcpListenerImpl::bind("127.0.0.1:0").await.unwrap());
        let l2 = listener.clone();

        // spawn accept，先建立 notified future
        let h = tokio::spawn(async move { l2.accept().await });

        // 等待 accept task 进入 poll（短 sleep）
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // close
        listener.close().await.unwrap();

        // accept 应在 1s 内返回错误
        let res = tokio::time::timeout(std::time::Duration::from_secs(1), h)
            .await
            .expect("close 后 accept 必须立即返回")
            .unwrap();
        assert!(res.is_err(), "close 后 accept 应返回错误");
    }

    #[tokio::test]
    async fn accept_after_close_returns_err() {
        let listener = TcpListenerImpl::bind("127.0.0.1:0").await.unwrap();
        listener.close().await.unwrap();
        let res = listener.accept().await;
        assert!(res.is_err());
    }
}