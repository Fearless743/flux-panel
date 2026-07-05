//! unix handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct UnixHandler;

#[async_trait]
impl Handler for UnixHandler {
    fn kind(&self) -> &'static str {
        "unix"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("unix"))
    }
}
