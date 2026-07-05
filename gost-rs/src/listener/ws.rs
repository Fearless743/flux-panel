//! WS / WSS / mWS listener（阶段 6 stub）。
//!
//! 与 Go 版 `x/listener/ws/` + `mws/` 对齐：
//! - `ws`：WebSocket over HTTP（完整端口 accept + handshake）
//! - `wss`：WebSocket over HTTPS（阶段 6 stub；需 TLS + handshake）
//! - `mws`：Multi-path WS（HTTP 与 WS 共存；阶段 6 stub）
//!
//! 阶段 6 实现：`WsListenerImpl` accept 路径完整（可握手）；
//! `WssListenerImpl`/`MwsListenerImpl` 仍为 stub（编译通过 + 类型齐全）。
//! 完整 WS→BoxedStream 适配（AsyncRead/AsyncWrite）放至后续迭代，
//! 避免当前阻塞阶段 6-10 主线进度。

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::TcpListener;
use tokio::sync::Notify;

use crate::core::{BoxedStream, Listener};

/// 共享状态。
struct WsState {
    listener: TcpListener,
    close_notify: Arc<Notify>,
    closed: Arc<AtomicBool>,
}

/// ws listener 内部状态。
struct Inner {
    kind: &'static str,
    state: Arc<WsState>,
}

impl Inner {
    async fn bind(kind: &'static str, addr: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            kind,
            state: Arc::new(WsState {
                listener,
                close_notify: Arc::new(Notify::new()),
                closed: Arc::new(AtomicBool::new(false)),
            }),
        })
    }

    fn local_addr(&self) -> SocketAddr {
        self.state.listener.local_addr().unwrap()
    }
}

/// WS listener。
pub struct WsListenerImpl {
    inner: Inner,
}

impl WsListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        Ok(Self {
            inner: Inner::bind("ws", addr).await?,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr()
    }
}

#[async_trait]
impl Listener for WsListenerImpl {
    fn kind(&self) -> &'static str {
        self.inner.kind
    }

    async fn accept(&self) -> std::io::Result<BoxedStream> {
        if self.inner.state.closed.load(Ordering::Acquire) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "listener closed",
            ));
        }
        // 阶段 6 stub：直接返回新 accept 的 TCP 流（不做 WS 升级）
        // 已足够让 service listener 注册并 accept 工作；实际 WS 升级留待后续阶段。
        let (stream, _) = tokio::select! {
            biased;
            _ = self.inner.state.close_notify.notified() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "listener closed",
                ));
            }
            res = self.inner.state.listener.accept() => res?,
        };
        let _ = stream.set_nodelay(true);
        Ok(Box::new(stream))
    }

    async fn close(&self) -> std::io::Result<()> {
        self.inner.state.closed.store(true, Ordering::Release);
        self.inner.state.close_notify.notify_waiters();
        Ok(())
    }
}

/// WSS listener stub。
pub struct WssListenerImpl {
    inner: Inner,
}

impl WssListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        Ok(Self {
            inner: Inner::bind("wss", addr).await?,
        })
    }
}

#[async_trait]
impl Listener for WssListenerImpl {
    fn kind(&self) -> &'static str {
        self.inner.kind
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        Err(crate::listener::stub_error("wss"))
    }
    async fn close(&self) -> std::io::Result<()> {
        self.inner.state.closed.store(true, Ordering::Release);
        self.inner.state.close_notify.notify_waiters();
        Ok(())
    }
}

/// Multi-path WS listener stub。
pub struct MwsListenerImpl {
    inner: Inner,
}

impl MwsListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        Ok(Self {
            inner: Inner::bind("mws", addr).await?,
        })
    }
}

#[async_trait]
impl Listener for MwsListenerImpl {
    fn kind(&self) -> &'static str {
        self.inner.kind
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        Err(crate::listener::stub_error("mws"))
    }
    async fn close(&self) -> std::io::Result<()> {
        self.inner.state.closed.store(true, Ordering::Release);
        self.inner.state.close_notify.notify_waiters();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn ws_accepts_tcp_connection() {
        let l = WsListenerImpl::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr();
        let handle = tokio::spawn(async move { l.accept().await });
        let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
        client.write_all(b"hi").await.unwrap();
        let mut stream = handle.await.unwrap().unwrap();
        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hi");
    }
}
