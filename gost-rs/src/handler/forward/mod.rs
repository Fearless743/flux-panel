//! forward handler 模块入口。
//!
//! - [`ForwardHandler`]：通用 forward，从 chain 选节点并双向转发。

mod handler;

pub use handler::{ForwardHandler, ForwardHandlerOptions};