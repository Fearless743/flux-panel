//! ingress 模块：基于 hostname 的入口规则。
//!
//! 与 Go 版 `x/ingress/ingress.go` 对齐：
//! - 从多个 endpoint 中按 hostname 路由
//! - 反代多租户场景

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct IngressRule {
    pub hostname: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Default)]
pub struct IngressTable {
    rules: Vec<IngressRule>,
}

impl IngressTable {
    pub fn new(rules: Vec<IngressRule>) -> Self {
        Self { rules }
    }

    pub fn from_json(rules: &HashMap<String, String>) -> Self {
        let rules: Vec<IngressRule> = rules
            .iter()
            .map(|(h, e)| IngressRule {
                hostname: h.clone(),
                endpoint: e.clone(),
            })
            .collect();
        Self { rules }
    }

    pub fn lookup(&self, hostname: &str) -> Option<&IngressRule> {
        self.rules
            .iter()
            .find(|r| r.hostname.eq_ignore_ascii_case(hostname))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_by_hostname() {
        let t = IngressTable::new(vec![
            IngressRule {
                hostname: "a.com".into(),
                endpoint: "backend-a:80".into(),
            },
            IngressRule {
                hostname: "b.com".into(),
                endpoint: "backend-b:80".into(),
            },
        ]);
        assert_eq!(
            t.lookup("a.com").unwrap().endpoint,
            "backend-a:80"
        );
        assert_eq!(
            t.lookup("A.COM").unwrap().endpoint,
            "backend-a:80"
        );
        assert!(t.lookup("c.com").is_none());
    }
}
