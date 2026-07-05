//! Metadata 模块：gost 服务/节点的 key-value 元数据。
//!
//! 与 Go 版 `x/metadata/metadata.go` 对应：
//! - 用 `HashMap<String, serde_json::Value>` 存储任意类型值；
//! - `paused` 键（bool）标记服务被暂停；
//! - `interface` 键（string）标记网卡绑定；
//! - `so_mark` 键（int）标记 SO_MARK；
//! - `host` 键（string）覆盖 host header；
//!
//! 阶段 1 的 `Config` 已把 metadata 字段全定义为 `HashMap<String, serde_json::Value>`，
//! 所以本模块主要提供**类型安全的 getter** 与常用工具方法。

use std::collections::HashMap;
use std::net::IpAddr;

use serde_json::Value;

pub type MetadataMap = HashMap<String, Value>;

/// 从 metadata 取 bool（缺省 false）。
pub fn get_bool(meta: &MetadataMap, key: &str) -> bool {
    meta.get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// 从 metadata 取字符串（缺省 None）。
pub fn get_str<'a>(meta: &'a MetadataMap, key: &str) -> Option<&'a str> {
    meta.get(key).and_then(|v| v.as_str())
}

/// 从 metadata 取 i64（缺省 None）。
pub fn get_i64(meta: &MetadataMap, key: &str) -> Option<i64> {
    meta.get(key).and_then(|v| v.as_i64())
}

/// 从 metadata 取 IP（字符串解析）。
pub fn get_ip(meta: &MetadataMap, key: &str) -> Option<IpAddr> {
    get_str(meta, key)
        .and_then(|s| s.parse::<IpAddr>().ok())
}

/// 服务是否被暂停（`metadata.paused == true`）。
pub fn is_paused(meta: &MetadataMap) -> bool {
    get_bool(meta, "paused")
}

/// 标记服务暂停。
pub fn mark_paused(meta: &mut MetadataMap) {
    meta.insert("paused".into(), Value::Bool(true));
}

/// 清除暂停标记。
pub fn clear_paused(meta: &mut MetadataMap) {
    meta.insert("paused".into(), Value::Bool(false));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paused_round_trip() {
        let mut m = MetadataMap::new();
        assert!(!is_paused(&m));
        mark_paused(&mut m);
        assert!(is_paused(&m));
        clear_paused(&mut m);
        assert!(!is_paused(&m));
    }

    #[test]
    fn typed_getters() {
        let mut m = MetadataMap::new();
        m.insert("interface".into(), json!("eth0"));
        m.insert("so_mark".into(), json!(0x123));
        m.insert("rate_limit".into(), json!("1MB"));

        assert_eq!(get_str(&m, "interface"), Some("eth0"));
        assert_eq!(get_i64(&m, "so_mark"), Some(0x123));
        assert_eq!(get_str(&m, "rate_limit"), Some("1MB"));

        // 缺省值
        assert!(!get_bool(&m, "missing"));
        assert!(get_str(&m, "missing").is_none());
    }

    #[test]
    fn ip_getter() {
        let mut m = MetadataMap::new();
        m.insert("ip".into(), json!("192.168.1.1"));
        assert_eq!(get_ip(&m, "ip").unwrap().to_string(), "192.168.1.1");
        // 无效 IP 不应 panic
        m.insert("ip".into(), json!("not-an-ip"));
        assert!(get_ip(&m, "ip").is_none());
    }
}