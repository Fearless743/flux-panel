//! bypass matcher：与 admission 一致，但语义上是"已通过的也不处理"。

use std::net::IpAddr;

use super::super::admission::matcher::{AdmissionMatcher, IpRule, Mode};

/// BypassMatcher = AdmissionMatcher 的别名（共用逻辑，语义区分）。
pub type BypassMatcher = AdmissionMatcher;
pub type BypassRule = IpRule;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_excludes_known_cidr() {
        let m = BypassMatcher::parse("192.168.1.0/24", Mode::Blacklist);
        assert!(m.allow("10.0.0.1".parse().unwrap()));
        assert!(!m.allow("192.168.1.5".parse().unwrap()));
    }
}
