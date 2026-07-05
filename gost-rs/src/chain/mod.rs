//! chain 模块入口。
//!
//! Chain：多跳链；每跳有自己的 selector + nodes。
//! 当前阶段只实现单跳 chain 的拨号逻辑；多跳 chain 通过递归 select→dial 完成。

pub mod chain_impl;
pub mod route;

pub use chain_impl::{ChainImpl, HopImpl};
pub use route::Route;