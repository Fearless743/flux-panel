//! file handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct FileHandler;

#[async_trait]
impl Handler for FileHandler {
    fn kind(&self) -> &'static str {
        "file"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("file"))
    }
}
