//! admission 模块：listener wrap，按 IP/CIDR 允许/拒绝。

pub mod matcher;
pub mod wrapper;

pub use matcher::{AdmissionMatcher, IpRule};
pub use wrapper::AdmissionListenerWrapper;
