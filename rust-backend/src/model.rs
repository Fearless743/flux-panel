use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// UnlimitedFlow / UnlimitedNum 为“无限流量/无限转发数量”的魔数标记。
pub const UNLIMITED_FLOW: i64 = 99999;
pub const UNLIMITED_NUM: i64 = 99999;

pub fn is_unlimited_flow(flow: i64) -> bool {
    flow >= UNLIMITED_FLOW
}

pub fn is_unlimited_num(num: i64) -> bool {
    num >= UNLIMITED_NUM
}

/// Go encoding/json 会把整数值的 float64 输出为 `1` 而不是 `1.0`。
pub fn serialize_f64_go<S>(v: &f64, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if v.is_finite() && v.fract() == 0.0 && *v >= i64::MIN as f64 && *v <= i64::MAX as f64 {
        s.serialize_i64(*v as i64)
    } else {
        s.serialize_f64(*v)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub user: String,
    #[serde(skip)]
    pub pwd: String,
    pub role_id: i32,
    pub exp_time: i64,
    pub flow: i64,
    pub in_flow: i64,
    pub out_flow: i64,
    pub flow_reset_time: i64,
    pub num: i64,
    pub created_time: i64,
    pub updated_time: Option<i64>,
    pub status: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: i64,
    pub name: String,
    pub secret: String,
    pub server_ip: String,
    // 历史版本遗留列（仅用于 SELECT * 扫描兼容）
    #[sqlx(default)]
    #[serde(rename = "autoDetectIP")]
    pub auto_detect_ip: bool,
    #[sqlx(default)]
    pub detected_ip: String,
    #[sqlx(default)]
    pub node_ips: String,
    #[sqlx(default)]
    pub support_brutal: bool,
    #[sqlx(default)]
    pub down_bw: i32,
    #[sqlx(default)]
    pub up_bw: i32,
    pub port: String,
    pub interface_name: Option<String>,
    pub version: Option<String>,
    pub http: i32,
    pub tls: i32,
    pub socks: i32,
    pub created_time: i64,
    pub updated_time: Option<i64>,
    pub status: i32,
    pub tcp_listen_addr: String,
    pub udp_listen_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct Tunnel {
    pub id: i64,
    pub name: String,
    #[serde(serialize_with = "serialize_f64_go")]
    pub traffic_ratio: f64,
    #[serde(rename = "type")]
    pub r#type: i32,
    pub protocol: String,
    pub flow: i64,
    pub created_time: i64,
    pub updated_time: i64,
    pub status: i32,
    pub in_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChainTunnel {
    pub id: i64,
    pub tunnel_id: i64,
    pub chain_type: String,
    pub node_id: i64,
    pub port: Option<i32>,
    pub strategy: Option<String>,
    pub inx: Option<i32>,
    pub protocol: Option<String>,
    #[sqlx(default)]
    pub brutal: bool,
    pub exit_node_ids: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct Forward {
    pub id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub name: String,
    pub tunnel_id: i64,
    pub remote_addr: String,
    pub strategy: String,
    pub in_flow: i64,
    pub out_flow: i64,
    pub created_time: i64,
    pub updated_time: i64,
    pub status: i32,
    pub inx: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct ForwardPort {
    pub id: i64,
    pub forward_id: i64,
    pub node_id: i64,
    pub port: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserTunnel {
    pub id: i64,
    pub user_id: i64,
    pub tunnel_id: i64,
    pub speed_id: Option<i64>,
    pub num: i64,
    pub flow: i64,
    pub in_flow: i64,
    pub out_flow: i64,
    pub flow_reset_time: i64,
    pub exp_time: i64,
    pub status: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpeedLimit {
    pub id: i64,
    pub name: String,
    pub speed: i32,
    pub tunnel_id: i64,
    pub tunnel_name: String,
    pub created_time: i64,
    pub updated_time: Option<i64>,
    pub status: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsFlow {
    pub id: i64,
    pub user_id: i64,
    pub flow: i64,
    pub total_flow: i64,
    pub time: String,
    pub created_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
pub struct ViteConfig {
    pub id: i64,
    pub name: String,
    pub value: String,
    pub time: i64,
}
