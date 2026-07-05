//! service 模块入口。

pub mod config_reporter;
pub mod global_traffic;
pub mod service_impl;
pub mod status;

pub use service_impl::{build_service, ServiceImpl};
pub use status::{ServiceEvent, ServiceStats, ServiceStatus};