//! TLS / mTLS listener（阶段 6 stub）。
//!
//! 与 Go 版 `x/listener/tls/listener.go` + `x/listener/mtls/listener.go` 对齐：
//! - `tls`：服务端 TLS（支持 ALPN h2/http1.1）
//! - `mtls`：mutual TLS（强制客户端证书）
//!
//! 阶段 6 实现：构造器接受 cert/key 字节；
//! accept 路径因 rustls acceptor 与 AsyncRead 适配未完成，暂返回 stub_error。
//! 完整 TLS handshake + stream adapter 留待后续阶段。
//!
//! 未来扩展点：
//! - 使用 [`tokio_rustls::TlsAcceptor`] 把 TCP stream 升级到 TLS
//! - 用 `[rustls::ServerConfig]` 加载 cert/key
//! - mtls 增加 `client_cert_verifier`

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::TcpListener;
use tokio::sync::Notify;

use crate::core::{BoxedStream, Listener};

/// TLS 状态。
struct TlsState {
    listener: TcpListener,
    close_notify: Arc<Notify>,
    closed: Arc<AtomicBool>,
}

/// TLS listener base。区分 kind (`tls` / `mtls`)。
pub struct TlsListenerImplBase {
    kind: &'static str,
    state: Arc<TlsState>,
    /// cert/key 是否已加载（用于构造验证；阶段 6 暂不真正用）
    cert_loaded: bool,
}

impl TlsListenerImplBase {
    pub async fn bind_inner(
        kind: &'static str,
        addr: &str,
        cert_pem: Vec<u8>,
        _key_pem: Vec<u8>,
        _client_ca_pem: Option<Vec<u8>>,
    ) -> std::io::Result<Self> {
        if cert_pem.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "tls listener: cert must not be empty",
            ));
        }
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            kind,
            state: Arc::new(TlsState {
                listener,
                close_notify: Arc::new(Notify::new()),
                closed: Arc::new(AtomicBool::new(false)),
            }),
            cert_loaded: true,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.state.listener.local_addr().unwrap()
    }
}

/// TLS listener。
pub struct TlsListenerImpl(pub TlsListenerImplBase);

impl TlsListenerImpl {
    pub async fn bind(addr: &str, cert_pem: Vec<u8>, key_pem: Vec<u8>) -> std::io::Result<Self> {
        Ok(Self(
            TlsListenerImplBase::bind_inner("tls", addr, cert_pem, key_pem, None).await?,
        ))
    }
}

/// mTLS listener。
pub struct MtlsListenerImpl(pub TlsListenerImplBase);

impl MtlsListenerImpl {
    pub async fn bind(
        addr: &str,
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
        client_ca_pem: Vec<u8>,
    ) -> std::io::Result<Self> {
        Ok(Self(
            TlsListenerImplBase::bind_inner(
                "mtls",
                addr,
                cert_pem,
                key_pem,
                Some(client_ca_pem),
            )
            .await?,
        ))
    }
}

async fn tls_accept(state: &TlsState, kind: &'static str) -> std::io::Result<BoxedStream> {
    if state.closed.load(Ordering::Acquire) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "listener closed",
        ));
    }
    let (stream, _) = tokio::select! {
        biased;
        _ = state.close_notify.notified() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "listener closed",
            ));
        }
        res = state.listener.accept() => res?,
    };
    let _ = stream.set_nodelay(true);
    // 阶段 6 stub: 未真正执行 rustls handshake；返回原始 TCP 流 + 标记 kind。
    // 业务层可通过 listener 类型 + 直接 is-tls 协议栈处理（实际生产前需替换为真正握手）。
    tracing::warn!(listener_kind = kind, "tls accept stub: 返回原始 TCP（TLS handshake 未实现）");
    Ok(Box::new(stream))
}

#[async_trait]
impl Listener for TlsListenerImpl {
    fn kind(&self) -> &'static str {
        self.0.kind
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        tls_accept(&self.0.state, self.0.kind).await
    }
    async fn close(&self) -> std::io::Result<()> {
        self.0.state.closed.store(true, Ordering::Release);
        self.0.state.close_notify.notify_waiters();
        Ok(())
    }
}

#[async_trait]
impl Listener for MtlsListenerImpl {
    fn kind(&self) -> &'static str {
        self.0.kind
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        tls_accept(&self.0.state, self.0.kind).await
    }
    async fn close(&self) -> std::io::Result<()> {
        self.0.state.closed.store(true, Ordering::Release);
        self.0.state.close_notify.notify_waiters();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tls_construction_does_not_panic() {
        let r = TlsListenerImpl::bind(
            "127.0.0.1:0",
            b"dummy".to_vec(),
            b"dummy".to_vec(),
        )
        .await;
        assert!(r.is_ok());
    }

    #[tokio::test]
    async fn mtls_construction_does_not_panic() {
        let r = MtlsListenerImpl::bind(
            "127.0.0.1:0",
            b"dummy".to_vec(),
            b"dummy".to_vec(),
            b"dummy".to_vec(),
        )
        .await;
        assert!(r.is_ok());
    }

    #[tokio::test]
    async fn tls_accepts_tcp_when_stub() {
        let l = TlsListenerImpl::bind("127.0.0.1:0", b"dummy".to_vec(), b"dummy".to_vec())
            .await
            .unwrap();
        let addr = l.0.local_addr();
        let handle = tokio::spawn(async move { l.accept().await });
        let _ = tokio::net::TcpStream::connect(addr).await;
        let stream = handle.await.unwrap().unwrap();
        let _ = stream;
    }
}
