//! util 模块入口。
//!
//! 阶段 1 实现：
//! - `aes`：AES-256-GCM（与 Java AESCrypto 兼容：SHA256(secret) → 32B key, 12B nonce, 16B tag, base64）
//! - `duration`：Go `time.Duration` 解析（"300ms" / "1h30m" / 纳秒整数）
//!
//! 后续阶段追加：relay、port、addr、net、sniffer、socks、stats、sysinfo、url、pprof 等。

pub mod aes;
pub mod duration;
pub mod net;
pub mod port;
pub mod relay;