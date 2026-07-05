//! listener 模块入口。
//!
//! 与 Go 版 `x/listener/` 对齐，提供全部 listener 类型实现 + 注册接口。
//!
//! 注册接口 `register(kind, factory)` 在 [`crate::listener_registry`] 中提供；
//! 每个 listener 模块在 `init` 函数中自动注册自身。
//!
//! ## 阶段 6 完整 listener 列表（与 Go `x/listener/` 1:1）：
//!
//! | 类型         | 状态             | 文件           |
//! |--------------|------------------|----------------|
//! | `tcp`        | ✅ 完整          | `tcp.rs`       |
//! | `udp`        | ✅ 完整          | `udp.rs`       |
//! | `rtcp`       | ✅ 完整          | `rtcp.rs`      |
//! | `rudp`       | ✅ 完整          | `rudp.rs`      |
//! | `tls`        | ✅ 完整          | `tls.rs`       |
//! | `mtls`       | ✅ 完整          | `tls.rs`       |
//! | `ws`         | ✅ 完整          | `ws.rs`        |
//! | `wss`        | ✅ 完整          | `ws.rs`        |
//! | `mws`        | ✅ 完整          | `ws.rs`        |
//! | `mtcp`       | ⚠ stub          | `mtcp.rs`      |
//! | `unix`       | ✅ 完整          | `unix.rs`      |
//! | `redirect/tcp` | ⚠ stub        | `redirect_tcp.rs` |
//! | `redirect/udp` | ⚠ stub        | `redirect_udp.rs` |
//! | `quic`       | ⚠ stub          | `quic.rs`      |
//! | `dtls`       | ⚠ stub          | `dtls.rs`      |
//! | `kcp`        | ⚠ stub          | `kcp.rs`       |
//! | `http2`      | ⚠ stub          | `http2.rs`     |
//! | `http3`      | ⚠ stub          | `http3.rs`     |
//! | `grpc`       | ⚠ stub          | `grpc.rs`      |
//! | `ssh`        | ⚠ stub          | `ssh.rs`       |
//! | `sshd`       | ⚠ stub          | `sshd.rs`      |
//! | `dns`        | ⚠ stub          | `dns.rs`       |
//! | `icmp`       | ⚠ stub          | `icmp.rs`      |
//! | `serial`     | ⚠ stub          | `serial.rs`    |
//! | `tap`        | ⚠ stub          | `tap.rs`       |
//! | `tun`        | ⚠ stub          | `tun.rs`       |
//! | `ftcp`       | ⚠ stub          | `ftcp.rs`      |
//! | `obfs`       | ⚠ stub          | `obfs.rs`      |
//! | `pht`        | ⚠ stub          | `pht.rs`       |
//!
//! Stub 文件提供构造器和占位实现，可以被 Service 注册，
//! 运行时返回 `Err(Unimplemented)`（与 Go 版 "listener not implemented" 语义一致）。

pub mod dtls;
pub mod ftcp;
pub mod grpc;
pub mod http2;
pub mod http3;
pub mod icmp;
pub mod kcp;
pub mod mtcp;
pub mod mws;
pub mod obfs;
pub mod pht;
pub mod quic;
pub mod redirect_tcp;
pub mod redirect_udp;
pub mod rtcp;
pub mod rudp;
pub mod serial;
pub mod ssh;
pub mod sshd;
pub mod tap;
pub mod tcp;
pub mod tls;
pub mod tun;
pub mod udp;
pub mod unix;
pub mod ws;

pub use tcp::TcpListenerImpl;
pub use udp::UdpListenerImpl;
pub use rtcp::RtcpListenerImpl;
pub use rudp::RudpListenerImpl;
pub use tls::{MtlsListenerImpl, TlsListenerImpl};
pub use unix::UnixListenerImpl;
pub use ws::{MwsListenerImpl, WsListenerImpl, WssListenerImpl};

// Registry 已存在：见 crate::registry::listener_registry（阶段 6 新增）
pub use crate::registry::listener_registry;

use crate::core::BoxedStream;

/// Listener stub 的通用错误：返回未实现特性。
pub fn stub_error(kind: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("listener '{kind}' is a stub (compiled but runtime not implemented)"),
    )
}

/// 通用 async stub trait：所有 listener 都用这个，stub 返回错误。
pub async fn stub_accept(kind: &'static str) -> std::io::Result<BoxedStream> {
    Err(stub_error(kind))
}
