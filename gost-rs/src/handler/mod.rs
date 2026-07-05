//! handler 模块入口。
//!
//! 与 Go 版 `x/handler/` 1:1 对齐：
//!
//! - 阶段 2 已实现：`forward`（forward/local + remote）
//! - 阶段 4 实现：`relay`（connect/bind/forward）
//! - 阶段 7 扩展：`socks5`（完整），其余 `socks4/http http2 http3 ss ss-udp sshd sni router auto file api metrics tap tun unix redirect-tcp redirect-udp dns` 提供 stub
//!
//! Stub 文件的语义：构造器返回 `Err(stub_error)`，与 Go 版"handler not implemented"语义一致。

pub mod api;
pub mod auto;
pub mod dns;
pub mod file;
pub mod forward;
pub mod http;
pub mod http2;
pub mod http3;
pub mod metrics;
pub mod redirect;
pub mod relay;
pub mod router;
pub mod serial;
pub mod sni;
pub mod socks;
pub mod ss;
pub mod sshd;
pub mod tap;
pub mod tunnel;
pub mod tun;
pub mod unix;

pub use forward::{ForwardHandler, ForwardHandlerOptions};
pub use relay::{RelayHandler, RelayHandlerOptions};
pub use socks::{Socks5Handler, Socks5HandlerOptions};

/// 公共错误：handler / dialer / connector stub 统一返回。
pub fn stub_error(kind: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("'{kind}' handler is a stub"),
    )
}
