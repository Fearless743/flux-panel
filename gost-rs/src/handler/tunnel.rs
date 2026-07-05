//! tunnel handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct TunnelHandler;

#[async_trait]
impl Handler for TunnelHandler {
    fn kind(&self) -> &'static str {
        "tunnel"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("tunnel"))
    }
}
