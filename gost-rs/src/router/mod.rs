//! router 模块：路由表（gateway/dst 模式）。
//!
//! 阶段 8 实现：仅按 destination 路由表匹配（Linux netlink/iptables 未实现）。

pub mod route;

pub use route::{Route, Router, RouterSelector};
