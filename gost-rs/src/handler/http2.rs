//! http2 handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct Http2Handler;

#[async_trait]
impl Handler for Http2Handler {
    fn kind(&self) -> &'static str {
        "http2"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("http2"))
    }
}
