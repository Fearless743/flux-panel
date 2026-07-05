//! api 模块：gost RESTful API（次要；flux-panel 不使用）。
//!
//! 阶段 9 仅提供最小可工作 HTTP 端点：
//! - `GET  /api/config` → 返回 gost.json
//! - `POST /api/config` → 替换 gost.json
//! - `GET  /api/services` → 返回 service 列表（含 status/stats）
//! - `GET  /metrics` → 返回 prometheus text format

pub mod http;

pub use http::{ApiServer, ApiServerOptions};
