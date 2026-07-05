//! bypass 模块：按 IP/CIDR 决定是否"旁路"流量（与 admission 逻辑对称）。

pub mod matcher;
pub mod wrapper;

pub use matcher::{BypassMatcher, BypassRule};
pub use wrapper::BypassListenerWrapper;
