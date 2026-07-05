//! selector 模块入口。
//!
//! 节点选择策略（与 Go 版 `x/selector/selector.go` 对齐）：
//! - `round`：固定顺序轮询
//! - `random`：随机
//! - `fifo`：先进先出（顺序轮询，无 backoff）
//! - `roundrobin`：加权轮询
//! - `hash`：按 key 哈希（暂不实现，阶段 7+）
//!
//! 选择器还需维护 `maxFails` 和 `failTimeout`：
//! 节点失败计数 ≥ maxFails 后，在 failTimeout 内不再被选中。

pub mod strategy;

pub use strategy::{ArcSelector, NodeStatus, Selector, SelectorImpl};