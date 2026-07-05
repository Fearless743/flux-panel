//! 全局流量管理器。
//!
//! 与 Go 版 `x/service/global_traffic_manager.go` 对齐：
//! - 每 5s 累加各 service 的流量 → 触发 HTTP /flow/upload
//! - 累加器维护 delta：上传成功后清零
//!
//! 阶段 3 实现：直接调用 service_cmd 的 service list 计算 delta。

use std::sync::Arc;
use std::time::Duration;

use tokio::time::interval;

use crate::registry::services_registry;
use crate::report::http_upload::ServiceTraffic;
use crate::report::websocket::WsReporter;

/// 全局流量上报循环。
///
/// 每 5s 触发 [`WsReporter::upload_traffic`]，把 delta 推给面板。
pub async fn run_global_traffic_loop(reporter: Arc<WsReporter>) {
    let mut tick = interval(Duration::from_secs(5));
    loop {
        tick.tick().await;
        if let Err(e) = reporter.upload_traffic().await {
            tracing::warn!(error = %e, "流量上报失败");
        }
    }
}

/// 计算当前所有 service 的 delta 流量。
///
/// 用于测试与单元验证。
pub fn snapshot_traffic() -> Vec<ServiceTraffic> {
    let reg = services_registry();
    let mut out = Vec::new();
    for name in reg.names() {
        if let Some(svc) = reg.get(&name) {
            let s = svc.stats.lock().clone();
            out.push(ServiceTraffic {
                name: svc.config.name.clone(),
                input_bytes: s.input_bytes,
                output_bytes: s.output_bytes,
                total_conns: s.total_conns,
                current_conns: s.current_conns,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_empty() {
        let s = snapshot_traffic();
        // 全局注册表可能有其他测试残留；这里只验证调用不 panic
        let _ = s;
    }
}