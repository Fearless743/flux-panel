//! UDP listener。
//!
//! 与 Go 版 `x/listener/udp/listener.go` 对齐：
//! - bind `addr`
//! - accept = recv 一帧 UDP 数据包，返回的 stream 是该包的元组（peer + buf），
//!   上层 forward handler 收到后写入目标并回复给 peer。
//!
//! 阶段 2 简化版：accept 一次返回 stream，stream 内部保存首个数据包和 peer；
//! 完整 UDP forward（多次 recv 的 session 关联）在阶段 6 实现。

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UdpSocket;

use crate::core::{BoxedStream, Listener, Stream};

/// UDP packet stream：首次 read 返回首包；write 发回 peer。
pub struct UdpPacketStream {
    state: Arc<Mutex<Option<UdpPacketState>>>,
}

struct UdpPacketState {
    buf: Vec<u8>,
    pos: usize,
    peer: SocketAddr,
    sock: Arc<UdpSocket>,
}

impl UdpPacketStream {
    pub fn new(buf: Vec<u8>, peer: SocketAddr, sock: Arc<UdpSocket>) -> Self {
        Self {
            state: Arc::new(Mutex::new(Some(UdpPacketState {
                buf,
                pos: 0,
                peer,
                sock,
            }))),
        }
    }
}

impl AsyncRead for UdpPacketStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let mut guard = this.state.lock();
        match guard.as_mut() {
            Some(s) => {
                let n = (s.buf.len() - s.pos).min(buf.remaining());
                buf.put_slice(&s.buf[s.pos..s.pos + n]);
                s.pos += n;
                std::task::Poll::Ready(Ok(()))
            }
            None => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "no packet",
            ))),
        }
    }
}

impl AsyncWrite for UdpPacketStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        let guard = this.state.lock();
        match guard.as_ref() {
            Some(s) => {
                let n = buf.len();
                let sock = s.sock.clone();
                let peer = s.peer;
                let data = buf.to_vec();
                tokio::spawn(async move {
                    let _ = sock.send_to(&data, peer).await;
                });
                std::task::Poll::Ready(Ok(n))
            }
            None => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "no peer",
            ))),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// UDP listener。
pub struct UdpListenerImpl {
    kind: &'static str,
    sock: Arc<UdpSocket>,
    addr: SocketAddr,
}

impl UdpListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(addr).await?;
        let local = sock.local_addr()?;
        Ok(Self {
            kind: "udp",
            sock: Arc::new(sock),
            addr: local,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

#[async_trait]
impl Listener for UdpListenerImpl {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn accept(&self) -> std::io::Result<BoxedStream> {
        let mut buf = vec![0u8; 65536];
        let (n, peer) = self.sock.recv_from(&mut buf).await?;
        buf.truncate(n);
        let stream: Box<dyn Stream> = Box::new(UdpPacketStream::new(buf, peer, self.sock.clone()));
        Ok(stream)
    }

    async fn close(&self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn udp_recv_returns_packet() {
        let listener = UdpListenerImpl::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr();

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.send_to(b"ping", addr).await.unwrap();

        let mut stream = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"ping");
    }
}