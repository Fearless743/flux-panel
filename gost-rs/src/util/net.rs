//! net 模块：地址解析、TCP/UDP 操作辅助。
//!
//! 与 Go 版 `x/internal/util/net/` 对齐。

use std::net::{IpAddr, SocketAddr};

use anyhow::{Context, Result};

/// 解析 "ip:port" 或 "[ipv6]:port" 形式的地址。
pub fn parse_addr(s: &str) -> Result<SocketAddr> {
    s.parse::<SocketAddr>()
        .with_context(|| format!("解析 SocketAddr 失败: {s}"))
}

/// 从 "ip:port" 中提取 ip 字符串（保留 v6 方括号）。
pub fn split_host_port(addr: &str) -> (&str, &str) {
    if let Some(rest) = addr.strip_prefix('[') {
        // IPv6 [::1]:8080
        if let Some(end) = rest.find(']') {
            let host = &rest[..end];
            let after = &rest[end + 1..];
            let port = after.strip_prefix(':').unwrap_or("");
            return (host, port);
        }
    }
    // IPv4 / 域名
    match addr.rfind(':') {
        Some(idx) => (&addr[..idx], &addr[idx + 1..]),
        None => (addr, ""),
    }
}

/// 解析 host 字符串为 IpAddr（如果是 IP）。
pub fn parse_ip(s: &str) -> Option<IpAddr> {
    s.parse::<IpAddr>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_v4() {
        assert_eq!(split_host_port("1.2.3.4:80"), ("1.2.3.4", "80"));
    }

    #[test]
    fn split_v6() {
        assert_eq!(split_host_port("[::1]:8080"), ("::1", "8080"));
    }

    #[test]
    fn split_no_port() {
        assert_eq!(split_host_port("example.com"), ("example.com", ""));
    }

    #[test]
    fn parse_v4() {
        let a = parse_addr("127.0.0.1:9000").unwrap();
        assert_eq!(a.port(), 9000);
        assert!(a.ip().is_ipv4());
    }

    #[test]
    fn parse_v6() {
        let a = parse_addr("[::1]:9000").unwrap();
        assert_eq!(a.port(), 9000);
        assert!(a.ip().is_ipv6());
    }

    #[test]
    fn parse_ip_v4() {
        assert!(parse_ip("1.2.3.4").is_some());
        assert!(parse_ip("not-ip").is_none());
        assert!(parse_ip("::1").is_some());
    }
}