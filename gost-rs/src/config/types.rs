//! 配置结构（serde）。
//!
//! 与 `go-gost/x/config/config.go` 1:1 字段映射；
//! 与 Java springboot-backend 通过 WebSocket 下发的 JSON 命令完全兼容。
//!
//! 字段名规则：
//! - 默认 `#[serde(rename_all = "camelCase")]`
//! - Go yaml tag 与 Java fastjson 不一致的字段用 `#[serde(rename = "...")]` 覆盖
//!
//! 注意 Go 的 `time.Duration` 在 JSON 里既能序列化为纳秒整数也能序列化为字符串（"600s"）。
//! gost 框架在收到 AddService/UpdateService 时会调用 `processDurationInData`
//! 把字符串转换为纳秒。Rust 端用 [`crate::util::duration::parse_duration_or_nanos`]
//! 处理这种二态数据。

use std::collections::HashMap;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

// ===========================================================================
// 节点端 config.json（运行时配置）
// ===========================================================================

/// 节点端 config.json（与 Go 版 `service.Config` / `x/socket/config.go` 对齐）。
///
/// `addr` 不含 scheme，scheme 由 `ssl` 决定。
/// 字段全部 snake_case，对应 Java 端的 `NodeConfigDto`。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeConfig {
    /// 面板地址（不含 scheme），例如 `1.2.3.4:8080`。
    pub addr: String,

    /// WebSocket 握手密钥（用于 AES-GCM 派生 key）。
    pub secret: String,

    /// 屏蔽 HTTP 协议（0=放行，1=屏蔽）；写入 config.json 持久化。
    #[serde(default)]
    pub http: i32,

    /// 屏蔽 TLS 协议（0=放行，1=屏蔽）。
    #[serde(default)]
    pub tls: i32,

    /// 屏蔽 SOCKS 协议（0=放行，1=屏蔽）。
    #[serde(default)]
    pub socks: i32,

    /// 是否使用 wss（对应 Go 版 `Ssl` 字段）。
    #[serde(default)]
    pub ssl: bool,
}

// ===========================================================================
// gost.json 顶层 Config
// ===========================================================================

/// gost.json 顶层结构（与 Go `config.Config` 对齐）。
///
/// 字段命名 camelCase（serde 默认行为），
/// 与 Java 端 `GostConfigDto` 兼容。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<ServiceConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chains: Vec<ChainConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hops: Vec<HopConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authers: Vec<AutherConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admissions: Vec<AdmissionConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bypasses: Vec<BypassConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolvers: Vec<ResolverConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<HostsConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ingresses: Vec<IngressConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routers: Vec<RouterConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sds: Vec<SDConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recorders: Vec<RecorderConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limiters: Vec<LimiterConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub climiters: Vec<LimiterConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rlimiters: Vec<LimiterConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observers: Vec<ObserverConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loggers: Vec<LoggerConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TLSConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<LogConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiling: Option<ProfilingConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<APIConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsConfig>,
}

// ===========================================================================
// Service
// ===========================================================================

/// Service 配置（与 Go `config.ServiceConfig` 对齐）。
///
/// `metadata` 是 HashMap，paused 标记存为 `metadata["paused"] = true`，
/// 业务接口 `metadata.interface` 存 IP 绑定。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ServiceConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addr: Option<String>,

    /// Deprecated: use metadata.interface
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#interface: Option<String>,

    /// Deprecated: use metadata.so_mark
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockopts: Option<SockOptsConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admission: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admissions: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bypasses: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limiter: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub climiter: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rlimiter: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loggers: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observer: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recorders: Vec<RecorderObject>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<HandlerConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listener: Option<ListenerConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forwarder: Option<ForwarderConfig>,

    /// 元数据 HashMap。`paused: true` 标记服务被暂停（由 PauseService 写入）。
    /// `interface: "eth0"` 标记网卡绑定。
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,

    /// 只读状态（由 gost 内部写入，不参与 AddService/UpdateService 校验）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ServiceStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ServiceStatus {
    #[serde(rename = "createTime")]
    pub create_time: i64,
    pub state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<ServiceEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<ServiceStats>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ServiceEvent {
    pub time: i64,
    pub msg: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ServiceStats {
    #[serde(rename = "totalConns")]
    pub total_conns: u64,
    #[serde(rename = "currentConns")]
    pub current_conns: u64,
    #[serde(rename = "totalErrs")]
    pub total_errs: u64,
    #[serde(rename = "inputBytes")]
    pub input_bytes: u64,
    #[serde(rename = "outputBytes")]
    pub output_bytes: u64,
}

// ===========================================================================
// Listener / Handler
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListenerConfig {
    /// listener 类型：tcp / udp / unix / rtcp / rudp / tls / http / http2 / http3 / ws / wss ...
    pub r#type: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_group: Option<ChainGroupConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auther: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authers: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TLSConfig>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HandlerConfig {
    /// handler 类型：tcp / udp / forward / relay / socks5 / http / http2 / http3 / sshd / ss ...
    pub r#type: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retries: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_group: Option<ChainGroupConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auther: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authers: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TLSConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limiter: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observer: Option<String>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

// ===========================================================================
// Forwarder（Handler 内的 chain 选择）
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ForwarderConfig {
    /// Deprecated: use hop
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// 引用的 hop 名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hop: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<SelectorConfig>,

    pub nodes: Vec<ForwardNodeConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ForwardNodeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addr: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bypasses: Vec<String>,

    /// Deprecated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,

    /// Deprecated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// Deprecated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Deprecated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<NodeFilterConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<NodeMatcherConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPNodeConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TLSNodeConfig>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NodeFilterConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NodeMatcherConfig {
    pub rule: String,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HTTPNodeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// Deprecated: use requestHeader
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_header: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_header: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rewrite: Vec<HTTPURLRewriteConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rewrite_url: Vec<HTTPURLRewriteConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rewrite_body: Vec<HTTPBodyRewriteConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HTTPURLRewriteConfig {
    pub match_: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HTTPBodyRewriteConfig {
    pub r#type: String,
    pub match_: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TLSNodeConfig {
    #[serde(rename = "serverName", skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<TLSOptions>,
}

// ===========================================================================
// Chain / Hop / Node / Selector
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChainConfig {
    pub name: String,
    pub hops: Vec<HopConfig>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChainGroupConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chains: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<SelectorConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HopConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#interface: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockopts: Option<SockOptsConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<SelectorConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bypasses: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<NodeConfig2>,

    /// 重新加载时间（暂未在阶段 1 使用，阶段 8 实现 file/redis/http loader 时启用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 与 HopConfig.nodes 元素同名，但与 ServiceConfig 区分（同样叫 NodeConfig 容易撞名）。
pub type NodeConfig2 = ChainNodeConfig;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChainNodeConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addr: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bypasses: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector: Option<ConnectorConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialer: Option<DialerConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#interface: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netns: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockopts: Option<SockOptsConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<NodeFilterConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<NodeMatcherConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPNodeConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TLSNodeConfig>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DialerConfig {
    /// dialer 类型：direct / tcp / udp / tls / ws / kcp / quic / ssh ...
    pub r#type: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TLSConfig>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ConnectorConfig {
    /// connector 类型：direct / tcp / relay / socks5 / http / tunnel ...
    pub r#type: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TLSConfig>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SockOptsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SelectorConfig {
    /// round / random / fifo / roundrobin / hash
    pub strategy: String,

    #[serde(rename = "maxFails")]
    pub max_fails: i32,

    /// 失败后标记节点不可用的时长。Go 端既能传 ns 整数也能传 "600s" 字符串。
    /// Rust 用 `parse_duration_or_nanos` 在反序列化前预处理（见阶段 3）。
    #[serde(rename = "failTimeout")]
    pub fail_timeout: Duration,
}

// ===========================================================================
// Duration（兼容 Go time.Duration：ns i64 / "600s" 字符串）
// ===========================================================================

/// Duration 类型：
/// - 内部存纳秒（i64）
/// - 反序列化时尝试 i64；失败时尝试字符串 "300ms" / "1h30m" 等
/// - 序列化时输出字符串（人类可读，与 Go 的 yaml 行为一致）
///
/// 完整实现见 `crate::util::duration`。
pub type Duration = i64;

pub const NANOS_PER_SEC: i64 = 1_000_000_000;

pub fn dur_seconds(s: i64) -> Duration {
    s * NANOS_PER_SEC
}

pub fn dur_minutes(m: i64) -> Duration {
    dur_seconds(m * 60)
}

// ===========================================================================
// Auth / Admission / Bypass
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AutherConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auths: Vec<AuthConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AuthConfig {
    pub username: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdmissionConfig {
    pub name: String,

    /// Deprecated: use whitelist
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whitelist: Option<bool>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matchers: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BypassConfig {
    pub name: String,

    /// Deprecated: use whitelist
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whitelist: Option<bool>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matchers: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

// ===========================================================================
// Loader（file/redis/http）
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FileLoader {
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RedisLoader {
    pub addr: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HTTPLoader {
    pub url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PluginConfig {
    pub r#type: String,
    pub addr: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TLSConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

// ===========================================================================
// Resolver / Hosts / Ingress / Router / SD
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResolverConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nameservers: Vec<NameserverConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NameserverConfig {
    pub addr: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer: Option<String>,

    #[serde(rename = "clientIP", default, skip_serializing_if = "Option::is_none")]
    pub client_ip: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub async_: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HostsConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mappings: Vec<HostMappingConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HostMappingConfig {
    pub ip: String,
    pub hostname: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IngressConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<IngressRuleConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IngressRuleConfig {
    pub hostname: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RouterConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<RouterRouteConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RouterRouteConfig {
    /// Deprecated: use dst
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst: Option<String>,

    pub gateway: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SDConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

// ===========================================================================
// Recorder / Limiter / Observer / Logger
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RecorderConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileRecorder>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp: Option<TCPRecorder>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPRecorder>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisRecorder>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RecorderObject {
    pub name: String,
    pub record: String,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FileRecorder {
    pub path: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sep: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<LogRotationConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TCPRecorder {
    pub addr: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HTTPRecorder {
    pub url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RedisRecorder {
    pub addr: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LimiterConfig {
    pub name: String,

    /// 限制表达式列表：例如 `["$ 100MB 100MB", "1.2.3.4 1MB 1MB"]`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limits: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<Duration>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HTTPLoader>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ObserverConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LoggerConfig {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<LogConfig>,
}

// ===========================================================================
// Log
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LogConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<LogRotationConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LogRotationConfig {
    #[serde(rename = "maxSize", default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<i32>,

    #[serde(rename = "maxAge", default, skip_serializing_if = "Option::is_none")]
    pub max_age: Option<i32>,

    #[serde(rename = "maxBackups", default, skip_serializing_if = "Option::is_none")]
    pub max_backups: Option<i32>,

    #[serde(rename = "localTime", default, skip_serializing_if = "Option::is_none")]
    pub local_time: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compress: Option<bool>,
}

// ===========================================================================
// TLS / API / Metrics / Profiling
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TLSConfig {
    #[serde(rename = "certFile", default, skip_serializing_if = "Option::is_none")]
    pub cert_file: Option<String>,

    #[serde(rename = "keyFile", default, skip_serializing_if = "Option::is_none")]
    pub key_file: Option<String>,

    #[serde(rename = "caFile", default, skip_serializing_if = "Option::is_none")]
    pub ca_file: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,

    #[serde(rename = "serverName", default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<TLSOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validity: Option<Duration>,

    #[serde(rename = "commonName", default, skip_serializing_if = "Option::is_none")]
    pub common_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TLSOptions {
    #[serde(rename = "minVersion", default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,

    #[serde(rename = "maxVersion", default, skip_serializing_if = "Option::is_none")]
    pub max_version: Option<String>,

    #[serde(rename = "cipherSuites", default, skip_serializing_if = "Vec::is_empty")]
    pub cipher_suites: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alpn: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct APIConfig {
    pub addr: String,

    #[serde(rename = "pathPrefix", default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,

    #[serde(rename = "accessLog", default, skip_serializing_if = "Option::is_none")]
    pub access_log: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auther: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MetricsConfig {
    pub addr: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auther: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProfilingConfig {
    pub addr: String,
}

// ===========================================================================
// 解析 SocketAddr 助手（addr 字段允许 "ip:port" 字符串）
// ===========================================================================

/// 把 `host:port` 字符串解析为 [`SocketAddr`]，支持 IPv4/IPv6/域名。
pub fn parse_socket_addr(addr: &str) -> anyhow::Result<SocketAddr> {
    use anyhow::Context;
    addr.parse::<SocketAddr>()
        .with_context(|| format!("无法解析 SocketAddr: {addr}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_round_trip() {
        let cfg = Config::default();
        let s = serde_json::to_string(&cfg).unwrap();
        let cfg2: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(cfg, cfg2);
    }

    #[test]
    fn service_config_paused_metadata() {
        let s = r#"{
            "name": "test_svc",
            "addr": "0.0.0.0:8080",
            "listener": {"type": "tcp"},
            "handler": {"type": "tcp"},
            "metadata": {"paused": true, "interface": "eth0"}
        }"#;
        let cfg: ServiceConfig = serde_json::from_str(s).unwrap();
        assert_eq!(cfg.name, "test_svc");
        assert!(cfg.metadata.get("paused").unwrap().as_bool().unwrap());
        assert_eq!(
            cfg.metadata.get("interface").unwrap().as_str().unwrap(),
            "eth0"
        );
    }

    #[test]
    fn chain_with_hops() {
        let s = r#"{
            "name": "chains_1",
            "hops": [{
                "name": "hop_1",
                "selector": {"strategy": "round", "maxFails": 1, "failTimeout": 600000000000},
                "nodes": [
                    {"name": "node_1", "addr": "1.1.1.1:7000", "dialer": {"type": "tcp"}}
                ]
            }]
        }"#;
        let cfg: ChainConfig = serde_json::from_str(s).unwrap();
        assert_eq!(cfg.name, "chains_1");
        assert_eq!(cfg.hops.len(), 1);
        assert_eq!(cfg.hops[0].nodes[0].name, "node_1");
        let sel = cfg.hops[0].selector.as_ref().unwrap();
        assert_eq!(sel.strategy, "round");
        assert_eq!(sel.fail_timeout, 600_000_000_000);
    }

    #[test]
    fn node_config_all_fields() {
        let s = r#"{
            "addr": "1.2.3.4:8080",
            "secret": "abc",
            "http": 1,
            "tls": 0,
            "socks": 1,
            "ssl": true
        }"#;
        let cfg: NodeConfig = serde_json::from_str(s).unwrap();
        assert_eq!(cfg.http, 1);
        assert!(cfg.ssl);
    }

    #[test]
    fn parse_socket_addr_v4() {
        let a = parse_socket_addr("0.0.0.0:8080").unwrap();
        assert_eq!(a.port(), 8080);
    }

    #[test]
    fn parse_socket_addr_v6() {
        let a = parse_socket_addr("[::1]:9000").unwrap();
        assert_eq!(a.port(), 9000);
    }
}