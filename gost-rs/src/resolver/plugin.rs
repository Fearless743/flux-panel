//! resolver/plugin stub。

use std::net::IpAddr;

pub struct PluginResolver;

impl PluginResolver {
    pub fn new() -> Self {
        Self
    }

    pub async fn resolve(&self, _host: &str) -> anyhow::Result<Vec<IpAddr>> {
        Ok(Vec::new())
    }
}

impl Default for PluginResolver {
    fn default() -> Self {
        Self::new()
    }
}
