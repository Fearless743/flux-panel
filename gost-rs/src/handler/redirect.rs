//! redirect handler stub（redirect-tcp / redirect-udp）。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct RedirectTcpHandler;
pub struct RedirectUdpHandler;

#[async_trait]
impl Handler for RedirectTcpHandler {
    fn kind(&self) -> &'static str {
        "redirect-tcp"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("redirect-tcp"))
    }
}

#[async_trait]
impl Handler for RedirectUdpHandler {
    fn kind(&self) -> &'static str {
        "redirect-udp"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("redirect-udp"))
    }
}
