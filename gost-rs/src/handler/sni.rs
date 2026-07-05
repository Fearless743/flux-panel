//! sni handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct SniHandler;

#[async_trait]
impl Handler for SniHandler {
    fn kind(&self) -> &'static str {
        "sni"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("sni"))
    }
}
