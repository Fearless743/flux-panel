//! socks handler（阶段 7 完整实现）。
//!
//! 与 Go 版 `x/handler/socks/` 对齐：支持 `socks5` / `socks4`。
//! 阶段 7 仅实现 socks5（基础协议）；socks4 留为 stub。

use async_trait::async_trait;
use parking_lot::Mutex;

use crate::core::{BoxedStream, Dialer, Handler};
use crate::dialer::TcpDialer;
use crate::handler::stub_error;

pub mod socks5_handler;

pub use socks5_handler::{Socks5Handler, Socks5HandlerOptions};

/// SOCKS4 handler stub（阶段 7 占位）。
pub struct Socks4Handler;

#[async_trait]
impl Handler for Socks4Handler {
    fn kind(&self) -> &'static str {
        "socks4"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("socks4"))
    }
}
