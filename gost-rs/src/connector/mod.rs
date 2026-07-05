//! connector 模块入口。

pub mod direct;
pub mod forward;
pub mod http;
pub mod http2;
pub mod relay;
pub mod router;
pub mod serial;
pub mod sni;
pub mod socks;
pub mod ss;
pub mod sshd;
pub mod tcp;
pub mod tunnel;
pub mod unix;

pub use direct::DirectConnector;
pub use relay::{RelayConnector, RelayConnectorOptions};
pub use tcp::TcpConnector;

/// stub connector 的统一错误。
pub fn stub_error(kind: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("connector '{kind}' is a stub"),
    )
}
