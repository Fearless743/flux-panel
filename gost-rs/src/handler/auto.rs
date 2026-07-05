//! auto handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct AutoHandler;

#[async_trait]
impl Handler for AutoHandler {
    fn kind(&self) -> &'static str {
        "auto"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("auto"))
    }
}
