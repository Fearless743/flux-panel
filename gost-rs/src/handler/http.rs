//! http handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct HttpHandler;

#[async_trait]
impl Handler for HttpHandler {
    fn kind(&self) -> &'static str {
        "http"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("http"))
    }
}

#[cfg(test)] mod tests { #[tokio::test] async fn construct() {} }
