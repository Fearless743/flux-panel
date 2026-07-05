//! metrics handler stub（prometheus scrape endpoint）。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct MetricsHandler;

#[async_trait]
impl Handler for MetricsHandler {
    fn kind(&self) -> &'static str {
        "metrics"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("metrics"))
    }
}
