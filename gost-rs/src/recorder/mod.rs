//! recorder 模块：事件持久化（file/redis/http）。
//!
//! 与 Go 版 `x/recorder/` 对齐：
//! - `file`：写 JSONL 文件
//! - `redis`：phredis 存 key/event
//! - `http`：POST 到面板 API
//! - `plugin`：子进程

pub mod file;
pub mod http;
pub mod plugin;
pub mod redis;

pub use file::FileRecorder;
pub use http::HttpRecorder;
