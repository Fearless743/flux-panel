//! api handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct ApiHandler;

#[async_trait]
impl Handler for ApiHandler {
    fn kind(&self) -> &'static str {
        "api"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("api"))
    }
}
