//! logger 模块：tracing 包装。
//!
//! 与 Go 版 `x/logger/logger.go` 对齐：
//! - `info!`、`warn!`、`error!`、`debug!` 宏对应 Info/Warn/Error/Debug
//! - 全局订阅器：`init_logger` 初始化 tracing-subscriber

use std::sync::Once;

static INIT: Once = Once::new();

/// 初始化默认 tracing 订阅器（fmt + EnvFilter）。
/// 可多次调用，仅生效第一次。
pub fn init_logger() {
    INIT.call_once(|| {
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .compact()
            .init();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_can_be_called_twice() {
        init_logger();
        init_logger(); // idempotent
    }
}
