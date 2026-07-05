//! dns handler stub。

use async_trait::async_trait;
use crate::core::{BoxedStream, Handler};
use crate::handler::stub_error;

pub struct DnsHandler;

#[async_trait]
impl Handler for DnsHandler {
    fn kind(&self) -> &'static str {
        "dns"
    }
    async fn handle(&self, _conn: BoxedStream) -> std::io::Result<()> {
        Err(stub_error("dns"))
    }
}
