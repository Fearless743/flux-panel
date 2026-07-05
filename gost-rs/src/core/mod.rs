//! core 模块：gost 核心抽象 trait。
//!
//! 与 Go 版 `x/registry/` 下的接口对应：
//! - `Listener`  → 监听并接受连接
//! - `Handler`   → 处理一个已接受的连接（forward/relay/socks5/...）
//! - `Dialer`    → 客户端拨号
//! - `Connector` → 客户端拨号但带协议握手（socks5/http/relay/...）
//! - `Service`   → 顶层服务：绑定 Listener + Handler，运行直到 Close
//! - `Chain`     → 链（多跳）
//! - `Hop`       → 链中的一跳
//! - `Node`      → 跳中的一个节点（endpoint）
//! - `Selector`  → 节点选择策略
//!
//! 每个 trait 都提供 `Type()` 返回 gost 风格类型字符串（"tcp" / "rtcp" / "forward" / ...），
//! 用于注册表 lookup。

use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

/// 双向字节流（TCP/Unix/TLS/...）。
///
/// `Stream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static`
/// 任意实现了以上 trait 的具体类型都自动实现 Stream。
pub trait Stream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static {}

impl<T> Stream for T where T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static {}

/// 任意可装箱的 stream。
pub type BoxedStream = Box<dyn Stream>;

/// Listener：绑定 addr，accept 后产出 [`BoxedStream`]。
#[async_trait]
pub trait Listener: Send + Sync {
    /// gost 类型字符串（与 registry 注册的 key 一致）。
    fn kind(&self) -> &'static str;

    /// 接受一个新连接。
    async fn accept(&self) -> std::io::Result<BoxedStream>;

    /// 关闭 listener（释放端口）。
    async fn close(&self) -> std::io::Result<()>;
}

/// Handler：处理一个已建立的连接。
#[async_trait]
pub trait Handler: Send + Sync {
    fn kind(&self) -> &'static str;

    /// 处理 conn 直到任一端关闭。
    async fn handle(&self, conn: BoxedStream) -> std::io::Result<()>;
}

/// Dialer：客户端拨号。
#[async_trait]
pub trait Dialer: Send + Sync {
    fn kind(&self) -> &'static str;

    /// 拨号到 `addr`，返回建立的连接。
    async fn dial(&self, addr: &str) -> std::io::Result<BoxedStream>;

    /// 拨号但不持有（用于 udp 多路复用）
    async fn dial_multi(&self, _addr: &str) -> std::io::Result<BoxedStream> {
        self.dial(_addr).await
    }
}

/// Connector：客户端拨号 + 协议握手（socks5/http/relay 等）。
#[async_trait]
pub trait Connector: Send + Sync {
    fn kind(&self) -> &'static str;

    /// connect 到 `addr` 并完成握手，返回建立的连接。
    async fn connect(&self, addr: &str) -> std::io::Result<BoxedStream>;
}

/// Chain：节点选择 + 拨号包装。
#[async_trait]
pub trait Chain: Send + Sync {
    /// 选择下一跳节点并拨号。
    async fn dial(&self) -> std::io::Result<(BoxedStream, String)>;

    /// 标记节点失败（触发 selector 的 maxFails/failTimeout 计数）。
    async fn mark_failed(&self, node: &str);

    /// 重置节点失败计数。
    async fn mark_success(&self, node: &str);

    /// 关闭 chain。
    async fn close(&self) -> std::io::Result<()>;
}

/// Service：完整服务（Listener + Handler）。
#[async_trait]
pub trait Service: Send + Sync {
    fn name(&self) -> &str;

    /// 启动服务（spawn listener accept loop）。
    async fn serve(&self) -> std::io::Result<()>;

    /// 关闭服务（close listener + 清理 chain）。
    async fn close(&self) -> std::io::Result<()>;
}

/// 可装箱 trait 对象别名。
pub type ArcListener = Arc<dyn Listener>;
pub type ArcHandler = Arc<dyn Handler>;
pub type ArcDialer = Arc<dyn Dialer>;
pub type ArcConnector = Arc<dyn Connector>;
pub type ArcChain = Arc<dyn Chain>;
pub type ArcService = Arc<dyn Service>;