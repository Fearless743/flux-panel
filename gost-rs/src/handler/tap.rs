//! tap handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct TapHandler;

#[async_trait]
impl Handler for TapHandler {
    fn kind(&self) -> &'static str {
        "tap"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("tap"))
    }
}
