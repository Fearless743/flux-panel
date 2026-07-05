//! auth 模块：file / http / redis / plugin 鉴权后端。

pub mod file;
pub mod http;
pub mod plugin;
pub mod redis;

pub trait Authenticator: Send + Sync {
    /// 检查 (user, pass) 是否合法；返回 Ok(true/false)。
    fn authenticate(&self, user: &str, pass: &str) -> bool;
}
