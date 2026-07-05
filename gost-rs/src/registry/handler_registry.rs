//! Handler 注册表：与 Go 版 `x/registry.HandlerRegistry()` 对齐。
//!
//! Handler 工厂签名为 `fn(config: &serde_json::Value) -> HandlerBuild`，
//! 接收配置 JSON，返回带 Handler 的对象。

use std::sync::Arc;

use dashmap::DashMap;

use crate::core::Handler;

pub struct HandlerBuild {
    pub handler: Arc<dyn Handler>,
}

impl std::fmt::Debug for HandlerBuild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerBuild").finish()
    }
}

pub struct HandlerRegistries {
    inner: DashMap<&'static str, fn(&serde_json::Value) -> HandlerBuild>,
}

impl HandlerRegistries {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    pub fn register(&self, kind: &'static str, factory: fn(&serde_json::Value) -> HandlerBuild) {
        self.inner.insert(kind, factory);
    }

    pub fn kinds(&self) -> Vec<&'static str> {
        self.inner.iter().map(|kv| *kv.key()).collect()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for HandlerRegistries {
    fn default() -> Self {
        Self::new()
    }
}

pub fn handler_registry() -> &'static HandlerRegistries {
    use once_cell::sync::Lazy;
    static REG: Lazy<HandlerRegistries> = Lazy::new(|| {
        let reg = HandlerRegistries::new();
        reg.register("forward", build_forward_handler);
        reg
    });
    &REG
}

/// helper：构造一个 forward handler。
pub fn build_forward_handler(_cfg: &serde_json::Value) -> HandlerBuild {
    use crate::handler::forward::{ForwardHandler, ForwardHandlerOptions};
    let opts = ForwardHandlerOptions::default();
    HandlerBuild {
        handler: Arc::new(ForwardHandler::new(opts)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_registry_has_forward() {
        let reg = handler_registry();
        assert!(reg.kinds().contains(&"forward"));
    }
}
