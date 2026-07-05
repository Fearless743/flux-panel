//! Chain 实现。
//!
//! - [`HopImpl`]：单跳，包含 selector 和 dialer/connector；
//! - [`ChainImpl`]：多跳链，按 hop 顺序逐跳拨号，每跳用上一跳的 conn 作为出口。

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::types::{ChainConfig, ChainNodeConfig, HopConfig};
use crate::core::{ArcChain, BoxedStream, Chain, Connector, Dialer};
use crate::selector::{ArcSelector, SelectorImpl};

use super::route::Route;

/// 单跳实现。
pub struct HopImpl {
    pub name: String,
    pub selector: ArcSelector,
    pub dialer: Option<Arc<dyn Dialer>>,
    pub connector: Option<Arc<dyn Connector>>,
}

impl HopImpl {
    pub fn new(
        hop_cfg: &HopConfig,
        nodes: &[ChainNodeConfig],
    ) -> anyhow::Result<Self> {
        let sel_cfg = hop_cfg
            .selector
            .clone()
            .unwrap_or(crate::config::types::SelectorConfig {
                strategy: "round".into(),
                max_fails: 1,
                fail_timeout: 30_000_000_000, // 30s
            });
        let selector: ArcSelector = Arc::new(SelectorImpl::new(sel_cfg, nodes));

        // 简化：每个节点的 dialer/connector 在节点列表中隐式存在。
        // 本阶段不解析节点的 dialer/connector 字段；用 direct dialer 拨号。
        let dialer: Arc<dyn Dialer> = Arc::new(crate::dialer::DirectDialer::new());
        let connector: Arc<dyn Connector> = Arc::new(crate::connector::DirectConnector::new());

        Ok(Self {
            name: hop_cfg.name.clone(),
            selector,
            dialer: Some(dialer),
            connector: Some(connector),
        })
    }

    /// 拨号并返回 Route（用 connector；如未配置则用 dialer）。
    pub async fn dial(&self) -> anyhow::Result<Route> {
        let (node, addr) = self
            .selector
            .select()
            .ok_or_else(|| anyhow::anyhow!("no node available"))?;

        // 尝试 connector → dialer → 直接
        let conn = if let Some(c) = &self.connector {
            c.connect(&addr).await?
        } else if let Some(d) = &self.dialer {
            d.dial(&addr).await?
        } else {
            anyhow::bail!("no dialer/connector configured");
        };

        self.selector.mark_success(&node);
        Ok(Route {
            node,
            addr,
            conn,
        })
    }
}

/// Chain 实现。
pub struct ChainImpl {
    pub name: String,
    pub hops: Vec<HopImpl>,
}

impl ChainImpl {
    /// 从 ChainConfig 构造 chain。
    pub fn new(cfg: &ChainConfig) -> anyhow::Result<Self> {
        let mut hops = Vec::with_capacity(cfg.hops.len());
        for h in &cfg.hops {
            let hop = HopImpl::new(h, &h.nodes)?;
            hops.push(hop);
        }
        Ok(Self {
            name: cfg.name.clone(),
            hops,
        })
    }

    /// 逐跳拨号并把每跳的 conn 作为下一跳的 transport 通道。
    ///
    /// 阶段 2 简化：只实现单跳（第一跳拨号成功即返回）。
    /// 多跳 chain 阶段 4（relay）会用到。
    pub async fn dial_first_hop(&self) -> anyhow::Result<Route> {
        if self.hops.is_empty() {
            anyhow::bail!("chain has no hops");
        }
        self.hops[0].dial().await
    }
}

#[async_trait]
impl Chain for ChainImpl {
    async fn dial(&self) -> std::io::Result<(BoxedStream, String)> {
        self.dial_first_hop()
            .await
            .map(|r| (r.conn, r.node))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }

    async fn mark_failed(&self, node: &str) {
        for h in &self.hops {
            h.selector.mark_failed(node);
        }
    }

    async fn mark_success(&self, node: &str) {
        for h in &self.hops {
            h.selector.mark_success(node);
        }
    }

    async fn close(&self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Chain 工厂辅助。
pub fn build_chain_arc(cfg: &ChainConfig) -> anyhow::Result<ArcChain> {
    Ok(Arc::new(ChainImpl::new(cfg)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ChainNodeConfig;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn chain_dials_first_node() {
        // 起一个 dummy TCP server
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut s, _)) = server.accept().await {
                let mut buf = [0u8; 5];
                let _ = s.read_exact(&mut buf).await;
                let _ = s.write_all(b"PONG\n").await;
            }
        });

        let cfg = ChainConfig {
            name: "c1".into(),
            hops: vec![HopConfig {
                name: "h1".into(),
                selector: Some(crate::config::types::SelectorConfig {
                    strategy: "round".into(),
                    max_fails: 1,
                    fail_timeout: 60_000_000_000,
                }),
                nodes: vec![ChainNodeConfig {
                    name: "n1".into(),
                    addr: Some(addr.to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        let chain = ChainImpl::new(&cfg).unwrap();
        let mut route = chain.dial_first_hop().await.unwrap();
        assert_eq!(route.node, "n1");

        route.conn.write_all(b"PING\n").await.unwrap();
        let mut buf = [0u8; 5];
        route.conn.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"PONG\n");
    }
}