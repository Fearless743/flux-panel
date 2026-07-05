//! flux-agent 库入口。
//!
//! 暴露所有模块供二进制与集成测试引用。

pub mod admission;
pub mod api;
pub mod auth;
pub mod bypass;
pub mod chain;
pub mod config;
pub mod connector;
pub mod core;
pub mod dialer;
pub mod handler;
pub mod ingress;
pub mod limiter;
pub mod listener;
pub mod logger;
pub mod metadata;
pub mod metrics;
pub mod observer;
pub mod plugin;
pub mod recorder;
pub mod registry;
pub mod report;
pub mod resolver;
pub mod router;
pub mod sd;
pub mod selector;
pub mod service;
pub mod service_cmd;
pub mod util;
pub mod version;