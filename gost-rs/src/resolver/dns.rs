//! DNS resolver（asynchronous）。

use std::net::SocketAddr;

use anyhow::Result;

/// DNS resolver 包装 tokio::net::lookup_host。
pub struct DnsResolver;

impl DnsResolver {
    pub fn new() -> Self {
        Self
    }

    pub async fn resolve(&self, addr: &str) -> Result<Vec<SocketAddr>> {
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host(addr).await?.collect();
        Ok(addrs)
    }
}

impl Default for DnsResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolves_localhost() {
        let r = DnsResolver::new();
        let addrs = r.resolve("localhost:80").await.unwrap();
        assert!(!addrs.is_empty());
        assert_eq!(addrs[0].port(), 80);
    }
}
