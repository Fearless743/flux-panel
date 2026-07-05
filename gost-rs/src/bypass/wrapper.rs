//! bypass listener wrapper（语义与 admission 对称）。

use std::sync::Arc;

use crate::bypass::matcher::BypassMatcher;
use crate::core::Listener;

pub struct BypassListenerWrapper {
    inner: Box<dyn Listener>,
    matcher: Arc<BypassMatcher>,
}

impl BypassListenerWrapper {
    pub fn new(inner: Box<dyn Listener>, matcher: Arc<BypassMatcher>) -> Self {
        Self { inner, matcher }
    }
}

#[async_trait::async_trait]
impl Listener for BypassListenerWrapper {
    fn kind(&self) -> &'static str {
        self.inner.kind()
    }

    async fn accept(&self) -> std::io::Result<crate::core::BoxedStream> {
        let stream = self.inner.accept().await?;
        // bypass 当前阶段 stub：直接放行
        let _ = &self.matcher;
        Ok(stream)
    }
    async fn close(&self) -> std::io::Result<()> {
        self.inner.close().await
    }
}
