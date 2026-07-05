//! Hosts resolver：维护 IP→hostname 静态映射。

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use parking_lot::RwLock;

#[derive(Clone, Default)]
pub struct HostsResolver {
    inner: Arc<RwLock<HashMap<IpAddr, Vec<String>>>>,
}

impl HostsResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// 解析 hosts 内容（`1.2.3.4 host1 host2`）。
    pub fn from_str(&self, content: &str) {
        let mut w = self.inner.write();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            if let Some(ip) = parts.next().and_then(|s| s.parse::<IpAddr>().ok()) {
                let names: Vec<String> = parts.map(String::from).collect();
                w.entry(ip).or_default().extend(names);
            }
        }
    }

    pub fn add(&self, ip: IpAddr, name: &str) {
        self.inner
            .write()
            .entry(ip)
            .or_default()
            .push(name.to_string());
    }

    pub fn lookup_by_ip(&self, ip: IpAddr) -> Vec<String> {
        self.inner
            .read()
            .get(&ip)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hosts_file() {
        let h = HostsResolver::new();
        h.from_str("# comment\n1.2.3.4 example.com alias.example\n");
        let names = h.lookup_by_ip("1.2.3.4".parse().unwrap());
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"example.com".to_string()));
    }
}
