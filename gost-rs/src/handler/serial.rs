//! serial handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct SerialHandler;

#[async_trait]
impl Handler for SerialHandler {
    fn kind(&self) -> &'static str {
        "serial"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("serial"))
    }
}
