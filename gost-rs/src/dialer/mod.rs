//! dialer 模块入口。

pub mod direct;
pub mod dtls;
pub mod ftcp;
pub mod grpc;
pub mod http2;
pub mod http3;
pub mod icmp;
pub mod kcp;
pub mod mtcp;
pub mod mtls;
pub mod mws;
pub mod obfs;
pub mod pht;
pub mod quic;
pub mod relay;
pub mod serial;
pub mod ssh;
pub mod sshd;
pub mod tcp;
pub mod tls;
pub mod udp;
pub mod unix;
pub mod wg;
pub mod ws;

pub use direct::DirectDialer;
pub use relay::{RelayDialer, RelayDialerOptions};
pub use tcp::TcpDialer;
pub use udp::UdpDialer;

/// 所有 stub dialer 的统一错误。
pub fn stub_error(kind: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("dialer '{kind}' is a stub"),
    )
}
