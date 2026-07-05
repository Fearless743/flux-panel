//! sshd handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct SshdHandler;

#[async_trait]
impl Handler for SshdHandler {
    fn kind(&self) -> &'static str {
        "sshd"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("sshd"))
    }
}
