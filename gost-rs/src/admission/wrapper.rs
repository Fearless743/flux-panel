//! admission listener wrapper。

use std::sync::Arc;

use crate::admission::matcher::AdmissionMatcher;
use crate::core::Listener;

/// 包装 listener，按 AdmissionMatcher 过滤连接。
pub struct AdmissionListenerWrapper {
    inner: Box<dyn Listener>,
    matcher: Arc<AdmissionMatcher>,
}

impl AdmissionListenerWrapper {
    pub fn new(inner: Box<dyn Listener>, matcher: Arc<AdmissionMatcher>) -> Self {
        Self { inner, matcher }
    }
}

#[async_trait::async_trait]
impl Listener for AdmissionListenerWrapper {
    fn kind(&self) -> &'static str {
        self.inner.kind()
    }

    async fn accept(&self) -> std::io::Result<crate::core::BoxedStream> {
        let stream = self.inner.accept().await?;
        let peer = match stream_p2p(&stream) {
            Some(p) => p,
            None => return Ok(stream), // 拿不到 peer 就放行
        };
        if self.matcher.allow(peer.ip()) {
            Ok(stream)
        } else {
            // reject：直接返回错误，service 层会丢弃
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "admission denied",
            ))
        }
    }

    async fn close(&self) -> std::io::Result<()> {
        self.inner.close().await
    }
}

/// 从 stream 推测 peer（适用于 TcpStream）。
fn stream_p2p(_s: &crate::core::BoxedStream) -> Option<std::net::SocketAddr> {
    // BoxedStream 已 trait-object erase，无法精确 inspect；
    // 阶段 8 stub：返回 None（不拒绝）。
    None
}
