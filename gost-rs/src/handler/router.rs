//! router handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct RouterHandler;

#[async_trait]
impl Handler for RouterHandler {
    fn kind(&self) -> &'static str {
        "router"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("router"))
    }
}
