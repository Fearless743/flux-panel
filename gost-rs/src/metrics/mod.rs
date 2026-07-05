//! metrics 模块：基于 prometheus crate。
//!
//! 提供 Counter / Gauge / Histogram 的封装，全局 default 注册表。

pub mod prom;

pub use prom::{Metrics, MetricsRegistry};
