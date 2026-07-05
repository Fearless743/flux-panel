//! 全局注册表。
//!
//! 与 Go 版 `x/registry/registry.go` 对应：保存所有运行时对象（service/chain/...）。
//! Rust 用 `DashMap<String, Arc<T>>` 替代 `sync.Map`。
//!
//! 子模块：
//! - `services`：service 注册表（阶段 2）
//! - `listener_registry`：listener 类型注册表（阶段 6）
//! - `handler_registry`：handler 类型注册表（阶段 6）
//!
//! 全局注册入口：`services_registry()`、`listener_registry()`、`handler_registry()`。

pub mod handler_registry;
pub mod listener_registry;

use std::sync::Arc;

use dashmap::DashMap;

use crate::service::ServiceImpl;

pub use handler_registry::{handler_registry, HandlerRegistries};
pub use listener_registry::{listener_registry, ListenerRegistries};

/// Service 注册表。
pub struct Services {
    inner: DashMap<String, Arc<ServiceImpl>>,
}

impl Services {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    pub fn register(&self, name: impl Into<String>, svc: Arc<ServiceImpl>) -> Option<Arc<ServiceImpl>> {
        self.inner.insert(name.into(), svc)
    }

    pub fn get(&self, name: &str) -> Option<Arc<ServiceImpl>> {
        self.inner.get(name).map(|s| s.value().clone())
    }

    pub fn unregister(&self, name: &str) -> Option<Arc<ServiceImpl>> {
        self.inner.remove(name).map(|(_, v)| v)
    }

    pub fn names(&self) -> Vec<String> {
        self.inner.iter().map(|kv| kv.key().clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&self) {
        self.inner.clear();
    }
}

impl Default for Services {
    fn default() -> Self {
        Self::new()
    }
}

pub fn services_registry() -> &'static Services {
    use once_cell::sync::Lazy;
    static REG: Lazy<Services> = Lazy::new(Services::new);
    &REG
}

/// 注册全部 listener 类型到 registry。
pub(crate) fn register_all_listeners(reg: &ListenerRegistries) {
    use crate::core::Listener;
    use crate::listener::stub_error;
    use std::future::Future;
    use std::pin::Pin;

    type ListenerFut = Pin<Box<dyn Future<Output = std::io::Result<Box<dyn Listener>>> + Send>>;
    type Factory = fn(&str) -> ListenerFut;

    // helper：把 addr 转为 owned String 并构造 Listener
    fn owned_fut<F, T>(addr_owned: String, fut: F) -> ListenerFut
    where
        F: Future<Output = std::io::Result<T>> + Send + 'static,
        T: Listener + Send + Sync + 'static,
    {
        let _ = addr_owned;
        Box::pin(async move {
            let l = fut.await?;
            Ok(Box::new(l) as Box<dyn Listener>)
        })
    }

    // helper：固定 kind 的 stub factory（必须是 fn pointer，所以不能用 closure 捕获 kind）
    // 用一个全局 dispatcher：#[track_caller] 通过 Send/Sync 安全。
    // 这里改用一个线程局部 kind + 单线程注册策略：每个 kind inline 写死。

    reg.register("tcp", |addr| {
        let s = addr.to_string();
        owned_fut(s.clone(), async move {
            crate::listener::TcpListenerImpl::bind(&s).await
        })
    });

    reg.register("rtcp", |addr| {
        let s = addr.to_string();
        owned_fut(s.clone(), async move {
            crate::listener::RtcpListenerImpl::bind(&s).await
        })
    });

    reg.register("udp", |addr| {
        let s = addr.to_string();
        owned_fut(s.clone(), async move {
            crate::listener::UdpListenerImpl::bind(&s).await
        })
    });

    reg.register("ws", |addr| {
        let s = addr.to_string();
        owned_fut(s.clone(), async move {
            crate::listener::WsListenerImpl::bind(&s).await
        })
    });

    reg.register("unix", |addr| {
        let s = addr.to_string();
        owned_fut(s.clone(), async move {
            crate::listener::UnixListenerImpl::bind(&s).await
        })
    });

    // 全局 stub dispatcher：不要 closure，改为 fn pointer
    fn stub_rudp(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("rudp")) })
    }
    fn stub_tls(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("tls")) })
    }
    fn stub_mtls(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("mtls")) })
    }
    fn stub_wss(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("wss")) })
    }
    fn stub_mws(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("mws")) })
    }
    fn stub_mtcp(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("mtcp")) })
    }
    fn stub_dtls(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("dtls")) })
    }
    fn stub_ftcp(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("ftcp")) })
    }
    fn stub_grpc(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("grpc")) })
    }
    fn stub_http2(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("http2")) })
    }
    fn stub_http3(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("http3")) })
    }
    fn stub_icmp(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("icmp")) })
    }
    fn stub_kcp(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("kcp")) })
    }
    fn stub_obfs_http(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("obfs-http")) })
    }
    fn stub_obfs_tls(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("obfs-tls")) })
    }
    fn stub_pht(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("pht")) })
    }
    fn stub_quic(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("quic")) })
    }
    fn stub_redirect_tcp(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("redirect/tcp")) })
    }
    fn stub_redirect_udp(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("redirect/udp")) })
    }
    fn stub_serial(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("serial")) })
    }
    fn stub_ssh(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("ssh")) })
    }
    fn stub_sshd(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("sshd")) })
    }
    fn stub_tap(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("tap")) })
    }
    fn stub_tun(_addr: &str) -> ListenerFut {
        Box::pin(async move { Err(stub_error("tun")) })
    }

    reg.register("rudp", stub_rudp as Factory);
    reg.register("tls", stub_tls as Factory);
    reg.register("mtls", stub_mtls as Factory);
    reg.register("wss", stub_wss as Factory);
    reg.register("mws", stub_mws as Factory);
    reg.register("mtcp", stub_mtcp as Factory);
    reg.register("dtls", stub_dtls as Factory);
    reg.register("ftcp", stub_ftcp as Factory);
    reg.register("grpc", stub_grpc as Factory);
    reg.register("http2", stub_http2 as Factory);
    reg.register("http3", stub_http3 as Factory);
    reg.register("icmp", stub_icmp as Factory);
    reg.register("kcp", stub_kcp as Factory);
    reg.register("obfs-http", stub_obfs_http as Factory);
    reg.register("obfs-tls", stub_obfs_tls as Factory);
    reg.register("pht", stub_pht as Factory);
    reg.register("quic", stub_quic as Factory);
    reg.register("redirect/tcp", stub_redirect_tcp as Factory);
    reg.register("redirect/udp", stub_redirect_udp as Factory);
    reg.register("serial", stub_serial as Factory);
    reg.register("ssh", stub_ssh as Factory);
    reg.register("sshd", stub_sshd as Factory);
    reg.register("tap", stub_tap as Factory);
    reg.register("tun", stub_tun as Factory);
}

/// 注册全部 handler 类型到 registry（占位，阶段 7 扩展）。
pub(crate) fn register_all_handlers(_reg: &HandlerRegistries) {
    // forward 在 handler_registry::handler_registry() 内注册
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_registry_is_singleton() {
        assert!(std::ptr::eq(services_registry() as *const _, services_registry() as *const _));
    }

    #[test]
    fn listener_registry_has_all_kinds() {
        let reg = listener_registry();
        let kinds = reg.kinds();
        assert!(kinds.len() >= 27, "应有 27+ listener 类型, 实际 {}", kinds.len());
        for required in ["tcp", "rtcp", "udp", "ws", "tls", "unix", "quic", "ssh"] {
            assert!(kinds.contains(&required), "{} 必须注册", required);
        }
    }
}
