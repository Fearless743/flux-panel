//! 配置上报循环（每 10 分钟推送 gost.json）。

use std::sync::Arc;
use std::time::Duration;

use tokio::time::interval;

use crate::report::websocket::WsReporter;

/// 每 10 分钟上传 gost.json 全量配置。
pub async fn run_config_reporter_loop(reporter: Arc<WsReporter>) {
    let mut tick = interval(Duration::from_secs(600));
    loop {
        tick.tick().await;
        if let Err(e) = reporter.upload_gost_config().await {
            tracing::warn!(error = %e, "配置上报失败");
        }
    }
}