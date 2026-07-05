//! http3 handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct Http3Handler;

#[async_trait]
impl Handler for Http3Handler {
    fn kind(&self) -> &'static str {
        "http3"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("http3"))
    }
}
