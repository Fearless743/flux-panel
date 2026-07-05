//! observer 模块：监听事件（统计/Stats 事件转发）。
//!
//! 与 Go 版 `x/observer/` 对齐：
//! - `stats`：单实例 level 流量统计
//! - 路由: 可选 plugin（grpc/http）转发

pub mod stats;

pub use stats::StatsObserver;
