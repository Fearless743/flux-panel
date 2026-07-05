//! rudp listener：remote UDP（与本地 UDP 等价，主要用于 chain 语义区分）。

use crate::core::{BoxedStream, Listener};

/// UDP 的 accept 即读取一帧数据。rudp 与 udp 区别仅语义（route/chain 用）。
pub struct RudpListenerImpl {
    inner: crate::listener::udp::UdpListenerImpl,
}

impl RudpListenerImpl {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        Ok(Self {
            inner: crate::listener::udp::UdpListenerImpl::bind(addr).await?,
        })
    }
}

// Delegation: RudpListenerImpl 利用 udp listener；通过 Arc 共享状态已经具备。
// 由于 UdpListenerImpl 字段不是 pub-Arc, 这里重新实现核心 accept 路径（简化）：
// 当前 udp listener 的 accept 接受一帧并包装为 BoxedStream。
// rudp 只是语义标记，实际接受路径复用 udp。
#[async_trait::async_trait]
impl Listener for RudpListenerImpl {
    fn kind(&self) -> &'static str {
        "rudp"
    }
    async fn accept(&self) -> std::io::Result<BoxedStream> {
        // udp listener 内部 mutable，无法通过 &self 调用 accept —— 走 forwarding 模式
        // 这里返回错误，提示 udp 转 rudp 暂未实现（UDT 在 phase 6+ 后续阶段完成）
        Err(crate::listener::stub_error("rudp"))
    }
    async fn close(&self) -> std::io::Result<()> {
        Ok(())
    }
}
