//! admission matcher：基于 IP/CIDR 的允许/拒绝规则。

use std::net::IpAddr;

use cidr::IpCidr;
use ipnetwork::IpNetwork;

#[derive(Debug, Clone, PartialEq)]
pub enum IpRule {
    /// 单个 IP
    Single(IpAddr),
    /// CIDR（10.0.0.0/8, etc）
    Cidr(IpCidr),
    /// 反向规则（黑名单），仅在名单外才允许
    Invert(Vec<IpRule>),
}

#[derive(Debug, Clone, Default)]
pub struct AdmissionMatcher {
    rules: Vec<IpRule>,
    /// reject mode? default accept everything not matched.
    mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// 黑名单：命中 = reject
    Blacklist,
    /// 白名单：命中 = allow；未命中 = reject
    #[default]
    Whitelist,
}

impl AdmissionMatcher {
    pub fn new(rules: Vec<IpRule>, mode: Mode) -> Self {
        Self { rules, mode }
    }

    /// 解析字符串 `"10.0.0.0/8,127.0.0.1,-192.168.1.0/24"`。
    pub fn parse(spec: &str, mode: Mode) -> Self {
        let mut rules = Vec::new();
        for tok in spec.split(',') {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            if let Some(rest) = tok.strip_prefix('-') {
                // 反向规则：解析为 Cidr/Invert
                if let Ok(cidr) = rest.parse::<IpCidr>() {
                    rules.push(IpRule::Invert(vec![IpRule::Cidr(cidr)]));
                } else if let Ok(ip) = rest.parse::<IpAddr>() {
                    rules.push(IpRule::Invert(vec![IpRule::Single(ip)]));
                }
            } else if let Ok(c) = tok.parse::<IpCidr>() {
                rules.push(IpRule::Cidr(c));
            } else if let Ok(ip) = tok.parse::<IpAddr>() {
                rules.push(IpRule::Single(ip));
            }
        }
        Self { rules, mode }
    }

    /// true = 允许该 IP；false = 拒绝。
    pub fn allow(&self, addr: IpAddr) -> bool {
        let any_matched = self.rules.iter().any(|r| rule_matches(r, addr));
        match self.mode {
            Mode::Whitelist => any_matched,
            Mode::Blacklist => !any_matched,
        }
    }
}

fn rule_matches(rule: &IpRule, addr: IpAddr) -> bool {
    match rule {
        IpRule::Single(ip) => ip == &addr,
        IpRule::Cidr(cidr) => cidr.contains(&addr),
        IpRule::Invert(rules) => rules.iter().all(|r| !rule_matches(r, addr)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn whitelist_only_10net() {
        let m = AdmissionMatcher::parse("10.0.0.0/8", Mode::Whitelist);
        assert!(m.allow("10.1.2.3".parse().unwrap()));
        assert!(!m.allow("11.1.2.3".parse().unwrap()));
    }

    #[test]
    fn blacklist_rejects_listed() {
        let m = AdmissionMatcher::parse("192.168.1.0/24", Mode::Blacklist);
        assert!(m.allow(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(!m.allow(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))));
    }

    #[test]
    fn invert_excludes_subnet() {
        // Whitelist everything but 192.168.1.0/24
        let m =
            AdmissionMatcher::new(vec![IpRule::Invert(vec![IpRule::Cidr("192.168.1.0/24".parse().unwrap())])], Mode::Whitelist);
        assert!(m.allow("10.0.0.1".parse().unwrap()));
        assert!(!m.allow("192.168.1.1".parse().unwrap()));
    }
}
