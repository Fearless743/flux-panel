//! router 路由表。

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct Route {
    pub net: cidr::IpCidr,
    pub gateway: Option<IpAddr>,
    pub interface: Option<String>,
}

#[derive(Default)]
pub struct Router {
    routes: Arc<RwLock<Vec<Route>>>,
    default_gateway: Arc<RwLock<Option<IpAddr>>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, route: Route) {
        self.routes.write().push(route);
    }

    pub fn set_default_gateway(&self, gw: IpAddr) {
        *self.default_gateway.write() = Some(gw);
    }

    /// 根据目标 IP 选最长匹配 prefix 的 route。
    pub fn select(&self, dest: IpAddr) -> Option<Route> {
        let r = self.routes.read();
        let mut best: Option<&Route> = None;
        let mut best_len: u8 = 0;
        for r in r.iter() {
            if r.net.contains(&dest) {
                let l = net_length(&r.net);
                if l >= best_len {
                    best = Some(r);
                    best_len = l;
                }
            }
        }
        best.cloned()
    }

    pub fn default_gateway(&self) -> Option<IpAddr> {
        *self.default_gateway.read()
    }
}

fn net_length(cidr: &cidr::IpCidr) -> u8 {
    match cidr {
        cidr::IpCidr::V4(_) => 32,
        cidr::IpCidr::V6(_) => 128,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_prefix_match() {
        let r = Router::new();
        r.add(Route {
            net: "10.0.0.0/8".parse().unwrap(),
            gateway: None,
            interface: Some("eth0".into()),
        });
        r.add(Route {
            net: "10.1.0.0/16".parse().unwrap(),
            gateway: None,
            interface: Some("eth1".into()),
        });
        let selected = r.select("10.1.2.3".parse().unwrap()).unwrap();
        assert_eq!(selected.interface.as_deref(), Some("eth1"));
    }

    #[test]
    fn default_gateway_fallback() {
        let r = Router::new();
        r.set_default_gateway("192.168.1.1".parse().unwrap());
        assert_eq!(r.default_gateway(), Some("192.168.1.1".parse().unwrap()));
    }
}

/// Router 工厂类型别名（阶段 8 简化）：单 router 实例。
pub type RouterSelector = HashMap<String, Arc<Router>>;
