//! resolver 模块：dns / hosts / plugin。

pub mod dns;
pub mod hosts;
pub mod plugin;

pub use dns::DnsResolver;
pub use hosts::HostsResolver;
