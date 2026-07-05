//! tun handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct TunHandler;

#[async_trait]
impl Handler for TunHandler {
    fn kind(&self) -> &'static str {
        "tun"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("tun"))
    }
}
