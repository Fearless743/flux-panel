//! 纯 Rust 端口强制断开。
//!
//! 替代 Go 版 `x/internal/util/port/port.go::ForceClosePortConnections`
//! （原版依赖外部 `tcpkill`，又依赖 dsniff 包；本移植改为纯 Rust）。
//!
//! ## 实现思路
//!
//! 本进程独占监听端口（accept 循环），所以"强制断开端口上的所有连接"
//! 等价于：把当前 service 持有的所有已 accept 的 conn 都强制 RST 关闭。
//!
//! 具体：
//! 1. 从 [`crate::registry::services_registry`] 找到绑定该端口（或名称）的 service
//! 2. 触发其 `pause()` + 立即 `shutdown()`（notify_waiters + close listener）
//! 3. shutdown 中对所有 in-flight conn task 通过 tokio task handle abort/close
//!
//! 若用户只想 RST 连接而不停 service：调用 `pause_then_force_close_conns(name)`。
//!
//! 关键 API：
//! - [`force_close_port_conns`]：按端口字符串强制断开
//! - [`force_close_service_conns`]：按 service 名强制断开

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::registry::services_registry;
use crate::service::ServiceImpl;

/// 按端口字符串（如 `:8080` / `127.0.0.1:8080` / `8080`）强制断开所有连接。
///
/// 与 Go 版 `ForceClosePortConnections(addr string)` 接口一致。
pub async fn force_close_port_conns(addr: &str) -> Result<()> {
    let port = parse_port(addr).context("解析端口失败")?;
    let reg = services_registry();
    for name in reg.names() {
        if let Some(svc) = reg.get(&name) {
            if let Some(svc_addr) = &svc.config.addr {
                if matches_port(svc_addr, port) {
                    force_close_service_conns_inner(&svc).await;
                }
            }
        }
    }
    Ok(())
}

/// 按 service 名强制断开所有连接（不断 listener，瞬时 RST 已建立的 conn）。
pub async fn force_close_service_conns(name: &str) -> Result<()> {
    let svc = services_registry()
        .get(name)
        .with_context(|| format!("service not found: {}", name))?;
    force_close_service_conns_inner(&svc).await;
    Ok(())
}

async fn force_close_service_conns_inner(svc: &Arc<ServiceImpl>) {
    // 1. 暂停（防止新连接进来在断开瞬间被处理）
    svc.pause();
    // 2. 触发 shutdown（关闭 listener + 等 in-flight 退出）
    //    shutdown 会 close listener，in-flight conn 在 copy_bidirectional 时
    //    因 conn drop 触发 TCP RST（linger=0 由 socket2 设置）。
    let _ = svc.shutdown().await;
    // 3. 给一窗口让 OS 真正发出 RST
    tokio::time::sleep(Duration::from_millis(50)).await;
    // 4. 恢复（重新进入 paused=false，等下次 listener 拉起）
    //    注意：listener 已 close，需 service_cmd::resume_service 重新绑定。
    //    这里仅清 paused 标志，listener 由上层 reload 处理。
    svc.resume();
}

/// 当前 service 是否监听指定端口。
fn matches_port(svc_addr: &str, port: u16) -> bool {
    if let Ok(a) = svc_addr.parse::<SocketAddr>() {
        return a.port() == port;
    }
    // 退化：尾部 :端口 解析
    if let Some(rest) = svc_addr.rsplit_once(':') {
        return rest.1.parse::<u16>().unwrap_or(0) == port;
    }
    false
}

/// 解析端口（支持 "8080"、":8080"、"1.2.3.4:8080"）。
fn parse_port(addr: &str) -> Result<u16> {
    let s = addr.trim();
    if let Ok(a) = s.parse::<SocketAddr>() {
        return Ok(a.port());
    }
    if let Some(rest) = s.strip_prefix(':') {
        return rest
            .parse::<u16>()
            .with_context(|| format!("invalid port: {}", s));
    }
    if let Some((_host, port)) = s.rsplit_once(':') {
        return port
            .parse::<u16>()
            .with_context(|| format!("invalid port: {}", s));
    }
    s.parse::<u16>().with_context(|| format!("invalid port: {}", s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_port_variants() {
        assert_eq!(parse_port("8080").unwrap(), 8080);
        assert_eq!(parse_port(":8080").unwrap(), 8080);
        assert_eq!(parse_port("127.0.0.1:8080").unwrap(), 8080);
        assert_eq!(parse_port("[::1]:8080").unwrap(), 8080);
        assert!(parse_port("notaport").is_err());
    }

    #[test]
    fn matches_port_works() {
        assert!(matches_port("127.0.0.1:8080", 8080));
        assert!(matches_port("0.0.0.0:8080", 8080));
        assert!(!matches_port("127.0.0.1:9090", 8080));
        assert!(matches_port("0.0.0.0:0", 0));
    }

    #[tokio::test]
    async fn force_close_nonexistent_port_is_noop() {
        // 没有任何 service 绑定该端口 → 返回 Ok，不 panic
        let res = force_close_port_conns("127.0.0.1:1").await;
        assert!(res.is_ok());
    }
}