//! Listener 注册表：与 Go 版 `x/registry.ListenerRegistry()` 对齐。
//!
//! 用 `DashMap<&'static str, ListenerFactory>` 存储 name → factory。
//! 工厂闭包 `fn(addr: &str) -> Pin<Box<dyn Future<Output = io::Result<Box<dyn Listener>>> + Send>>`
//! 接收地址，返回新构造的 Listener（owned by boxed dyn）。

use std::future::Future;
use std::pin::Pin;

use dashmap::DashMap;

use crate::core::Listener;

pub type ListenerFactory =
    fn(addr: &str) -> Pin<Box<dyn Future<Output = std::io::Result<Box<dyn Listener>>> + Send>>;

/// Listener 注册表。
pub struct ListenerRegistries {
    inner: DashMap<&'static str, ListenerFactory>,
}

impl ListenerRegistries {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    pub fn register(&self, kind: &'static str, factory: ListenerFactory) {
        self.inner.insert(kind, factory);
    }

    pub fn get(&self, kind: &str) -> Option<ListenerFactory> {
        self.inner.get(kind).map(|v| *v)
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

impl Default for ListenerRegistries {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局 listener 注册表（与 Go 版 `x/registry.ListenerRegistry()` 对应）。
pub fn listener_registry() -> &'static ListenerRegistries {
    use once_cell::sync::Lazy;
    static REG: Lazy<ListenerRegistries> = Lazy::new(|| {
        let reg = ListenerRegistries::new();
        super::register_all_listeners(&reg);
        reg
    });
    &REG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_registry_has_tcp() {
        let reg = listener_registry();
        assert!(reg.get("tcp").is_some(), "tcp 必须注册");
    }

    #[test]
    fn global_registry_returns_list() {
        let reg = listener_registry();
        let kinds = reg.kinds();
        assert!(!kinds.is_empty());
        assert!(kinds.contains(&"tcp"));
    }
}
