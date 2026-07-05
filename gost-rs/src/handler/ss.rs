//! ss handler stub（shadowsocks）。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct SsHandler;
pub struct SsUdpHandler;

#[async_trait]
impl Handler for SsHandler {
    fn kind(&self) -> &'static str {
        "ss"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("ss"))
    }
}

#[async_trait]
impl Handler for SsUdpHandler {
    fn kind(&self) -> &'static str {
        "ss-udp"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("ss-udp"))
    }
}
