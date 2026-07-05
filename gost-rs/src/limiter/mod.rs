//! limiter 模块入口。
//!
//! 与 Go 版 `x/limiter/` 对齐：
//! - `traffic`：基于令牌桶的流量限制（in/out），多 scope（Service/Conn/IP/CIDR）
//! - `conn`：连接数限制
//! - `rate`：请求速率限制
//! - `wrapper`：把 limiter 包装到 listener/conn/io 上
//!
//! 单文件实现切换：
//! - 令牌桶（[`mod@token_bucket`]）：governor 等价的速率控制
//! - 多 scope 缓存（[`mod@cache`]）：每个 key 一个 limiter，TTL 失效
//! - rate 解析（[`mod@parse`]）：`"$ 1MB 1MB"` / `"10MB"` 等字符串

pub mod cache;
pub mod conn;
pub mod parse;
pub mod rate;
pub mod traffic;
pub mod wrapper;

pub use traffic::{TrafficLimiter, TrafficLimiterBuilder};
pub use conn::ConnLimiter;
pub use rate::RateLimiter;