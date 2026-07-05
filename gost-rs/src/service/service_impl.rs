//! Service 实现。
//!
//! ServiceImpl 把 `listener` + `handler`（+可选 chain）组合成可运行的服务。
//!
//! 阶段 2 仅支持：
//! - listener.type = "tcp" / "udp"
//! - handler.type  = "forward"
//!
//! 阶段 6+ 扩展其他 listener/handler 类型。
//!
//! 与 Go 版 `x/service/service.go` 的 service lifecycle 行为对齐：
//! - 创建 → 立即监听 + spawn accept loop
//! - 关闭 → close listener + 等待 in-flight conn 退出
//! - 暂停 → 标记 metadata.paused=true，listener 继续监听但 accept 后立即 close 连接
//! - 恢复 → 清除 paused 标记，正常处理

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::sync::Notify;

use crate::config::types::ServiceConfig;
use crate::core::{ArcHandler, ArcListener, Service};
use crate::metadata;

use super::status::{ServiceStats, ServiceStatus};

/// Service 运行实例。
pub struct ServiceImpl {
    pub config: ServiceConfig,
    pub listener: Option<ArcListener>,
    pub handler: Option<ArcHandler>,
    pub status: Mutex<ServiceStatus>,
    pub stats: Arc<Mutex<ServiceStats>>,
    /// 已 spawn 的 accept task 数量。
    pub in_flight: Arc<AtomicU64>,
    pub paused: Arc<AtomicBool>,
    pub shutdown: Arc<Notify>,
}

impl ServiceImpl {
    /// 构造 service（不启动 listener）。
    pub fn new(config: ServiceConfig) -> Self {
        let paused = metadata::is_paused(&config.metadata);
        Self {
            config,
            listener: None,
            handler: None,
            status: Mutex::new(ServiceStatus::new_running()),
            stats: Arc::new(Mutex::new(ServiceStats::default())),
            in_flight: Arc::new(AtomicU64::new(0)),
            paused: Arc::new(AtomicBool::new(paused)),
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// 设置 listener（构造后绑定）。
    pub fn set_listener(&mut self, listener: ArcListener) {
        self.listener = Some(listener);
    }

    /// 设置 handler。
    pub fn set_handler(&mut self, handler: ArcHandler) {
        self.handler = Some(handler);
    }

    /// 启动 accept loop（spawn 后立即返回）。
    pub fn spawn_serve(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let me = self.clone();
        tokio::spawn(async move { me.serve_loop().await })
    }

    /// accept loop（持有 self）。
    async fn serve_loop(self: Arc<Self>) {
        let listener = match &self.listener {
            Some(l) => l.clone(),
            None => {
                tracing::error!(service = %self.config.name, "no listener configured");
                return;
            }
        };
        let handler = match &self.handler {
            Some(h) => h.clone(),
            None => {
                tracing::error!(service = %self.config.name, "no handler configured");
                return;
            }
        };

        loop {
            let conn = match listener.accept().await {
                Ok(c) => c,
                Err(e) => {
                    let kind = e.kind();
                    if kind == std::io::ErrorKind::Interrupted {
                        tracing::debug!(service = %self.config.name, "listener interrupted, exit");
                        break;
                    }
                    tracing::warn!(service = %self.config.name, error = %e, "accept error");
                    // 等 10ms 或 shutdown 信号
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {}
                        _ = self.shutdown.notified() => { break; }
                    }
                    continue;
                }
            };

            self.in_flight.fetch_add(1, Ordering::Relaxed);
            {
                let mut s = self.stats.lock();
                s.total_conns += 1;
                s.current_conns += 1;
            }

            // 暂停状态：直接关闭
            if self.paused.load(Ordering::Relaxed) {
                drop(conn);
                self.in_flight.fetch_sub(1, Ordering::Relaxed);
                {
                    let mut s = self.stats.lock();
                    s.current_conns = s.current_conns.saturating_sub(1);
                }
                continue;
            }

            let handler = handler.clone();
            let stats = self.stats.clone();
            let in_flight = self.in_flight.clone();
            tokio::spawn(async move {
                let res = handler.handle(conn).await;
                if let Err(e) = res {
                    tracing::debug!(error = %e, "handler error");
                    let mut s = stats.lock();
                    s.total_errs += 1;
                }
                let mut s = stats.lock();
                s.current_conns = s.current_conns.saturating_sub(1);
                in_flight.fetch_sub(1, Ordering::Relaxed);
            });
        }
    }

    /// 暂停（不 unregister，只标记 paused）。
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
        let mut s = self.status.lock();
        s.event("paused");
        s.set_state("paused");
    }

    /// 恢复。
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
        let mut s = self.status.lock();
        s.event("resumed");
        s.set_state("running");
    }

    /// 关闭服务（close listener + 等 in-flight 完成）。
    pub async fn shutdown(&self) -> std::io::Result<()> {
        self.shutdown.notify_waiters();
        if let Some(l) = &self.listener {
            let _ = l.close().await;
        }
        // 等所有 in-flight task 退出
        let mut ticks = 0;
        while self.in_flight.load(Ordering::Relaxed) > 0 && ticks < 100 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            ticks += 1;
        }
        let mut s = self.status.lock();
        s.event("shutdown");
        s.set_state("closed");
        Ok(())
    }

    /// 等待关闭信号。
    pub async fn wait_shutdown(&self) {
        self.shutdown.notified().await;
    }
}

#[async_trait]
impl Service for ServiceImpl {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn serve(&self) -> std::io::Result<()> {
        // 单实例 serve：阻塞直到 shutdown
        let me = Arc::new(self.clone_handle());
        let _ = me.serve_loop().await;
        Ok(())
    }

    async fn close(&self) -> std::io::Result<()> {
        self.shutdown().await
    }
}

/// ServiceImpl 不能直接 clone，但持有 Arc 引用可以在外层共享。
/// 提供一个手工 clone（深拷贝 listener/handler Arc）。
impl ServiceImpl {
    fn clone_handle(&self) -> Self {
        Self {
            config: self.config.clone(),
            listener: self.listener.clone(),
            handler: self.handler.clone(),
            status: Mutex::new(self.status.lock().clone()),
            stats: Arc::new(Mutex::new(self.stats.lock().clone())),
            in_flight: self.in_flight.clone(),
            paused: self.paused.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

/// Service 工厂：从 ServiceConfig 构造。
pub async fn build_service(cfg: &ServiceConfig) -> anyhow::Result<Arc<ServiceImpl>> {
    use crate::handler::forward::{ForwardHandler, ForwardHandlerOptions};
    use crate::listener::{TcpListenerImpl, UdpListenerImpl};

    let mut svc = ServiceImpl::new(cfg.clone());

    // 1. 构造 listener
    let addr = cfg.addr.as_deref().unwrap_or("0.0.0.0:0");
    let listener: ArcListener = match cfg.listener.as_ref().map(|l| l.r#type.as_str()) {
        Some("tcp") => Arc::new(TcpListenerImpl::bind(addr).await?),
        Some("udp") => Arc::new(UdpListenerImpl::bind(addr).await?),
        Some(t) => anyhow::bail!("listener type not supported in phase 2: {t}"),
        None => anyhow::bail!("service listener.type is required"),
    };
    svc.set_listener(listener);

    // 2. 构造 handler（仅 forward）
    let handler: ArcHandler = match cfg.handler.as_ref().map(|h| h.r#type.as_str()) {
        Some("forward") => {
            let opts = ForwardHandlerOptions {
                chain: None,
                target: None,
                dialer: Some(Arc::new(crate::dialer::DirectDialer::new())),
                buffer_size: 16 * 1024,
            };
            Arc::new(ForwardHandler::new(opts)) as ArcHandler
        }
        Some(t) => anyhow::bail!("handler type not supported in phase 2: {t}"),
        None => anyhow::bail!("service handler.type is required"),
    };
    svc.set_handler(handler);

    Ok(Arc::new(svc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{HandlerConfig, ListenerConfig};

    #[test]
    fn service_impl_init() {
        let cfg = ServiceConfig {
            name: "test".into(),
            addr: Some("127.0.0.1:0".into()),
            listener: Some(ListenerConfig {
                r#type: "tcp".into(),
                ..Default::default()
            }),
            handler: Some(HandlerConfig {
                r#type: "forward".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let svc = ServiceImpl::new(cfg);
        assert_eq!(svc.config.name, "test");
        assert!(!svc.paused.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn pause_resume_toggle() {
        let mut cfg = ServiceConfig::default();
        cfg.name = "t".into();
        let svc = ServiceImpl::new(cfg);
        svc.pause();
        assert!(svc.paused.load(std::sync::atomic::Ordering::Relaxed));
        svc.resume();
        assert!(!svc.paused.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn paused_from_metadata() {
        let mut cfg = ServiceConfig::default();
        cfg.name = "t".into();
        cfg.metadata
            .insert("paused".into(), serde_json::Value::Bool(true));
        let svc = ServiceImpl::new(cfg);
        assert!(svc.paused.load(std::sync::atomic::Ordering::Relaxed));
    }
}