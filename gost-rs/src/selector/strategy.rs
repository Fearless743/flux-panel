//! Selector 策略实现。
//!
//! 通用流程：
//! 1. 根据 strategy 从候选节点中选一个；
//! 2. 过滤掉 `fails >= max_fails` 且未过 `fail_timeout` 的节点；
//! 3. `mark_failed` / `mark_success` 维护失败计数。

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use rand::seq::SliceRandom;

use crate::config::types::{ChainNodeConfig, SelectorConfig};

/// 单节点状态。
#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub name: String,
    pub addr: String,
    pub fails: u32,
    /// 标记不可用的截止时间；`None` 表示可用。
    pub blocked_until: Option<Instant>,
}

/// Selector trait。
pub trait Selector: Send + Sync {
    /// 从 nodes 中选择一个节点；返回 `(name, addr)`。
    fn select(&self) -> Option<(String, String)>;

    /// 节点拨号成功 → 重置失败计数。
    fn mark_success(&self, node: &str);

    /// 节点拨号失败 → 计数 +1，达到 max_fails 则标记 blocked。
    fn mark_failed(&self, node: &str);

    /// 当前候选节点数。
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 默认实现：维护节点状态池，按 strategy 选择。
pub struct SelectorImpl {
    cfg: SelectorConfig,
    states: Mutex<Vec<NodeStatus>>,
    cursor: Mutex<usize>,
}

impl SelectorImpl {
    /// 构造 selector。
    pub fn new(cfg: SelectorConfig, nodes: &[ChainNodeConfig]) -> Self {
        let states = nodes
            .iter()
            .map(|n| NodeStatus {
                name: n.name.clone(),
                addr: n.addr.clone().unwrap_or_default(),
                fails: 0,
                blocked_until: None,
            })
            .collect();
        Self {
            cfg,
            states: Mutex::new(states),
            cursor: Mutex::new(0),
        }
    }

    /// 当前可用节点（未 blocked）。
    fn available(&self, states: &[NodeStatus]) -> Vec<usize> {
        let now = Instant::now();
        states
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s.blocked_until {
                Some(t) if t > now => None,
                _ => Some(i),
            })
            .collect()
    }

    /// reset 过期 blocked。
    fn cleanup_blocked(states: &mut Vec<NodeStatus>) {
        let now = Instant::now();
        for s in states.iter_mut() {
            if let Some(t) = s.blocked_until {
                if t <= now {
                    s.blocked_until = None;
                }
            }
        }
    }
}

impl Selector for SelectorImpl {
    fn select(&self) -> Option<(String, String)> {
        let mut states = self.states.lock();
        Self::cleanup_blocked(&mut states);
        let avail = self.available(&states);
        if avail.is_empty() {
            return None;
        }
        let idx = match self.cfg.strategy.as_str() {
            "random" => {
                let mut rng = rand::thread_rng();
                *avail.choose(&mut rng).unwrap()
            }
            "round" | "fifo" | "roundrobin" => {
                let mut cur = self.cursor.lock();
                let i = *cur % avail.len();
                *cur = (*cur + 1) % avail.len();
                avail[i]
            }
            // 未知策略 → 退化为 round
            _ => {
                let mut cur = self.cursor.lock();
                let i = *cur % avail.len();
                *cur = (*cur + 1) % avail.len();
                avail[i]
            }
        };
        Some((states[idx].name.clone(), states[idx].addr.clone()))
    }

    fn mark_success(&self, node: &str) {
        let mut states = self.states.lock();
        for s in states.iter_mut() {
            if s.name == node {
                s.fails = 0;
                s.blocked_until = None;
            }
        }
    }

    fn mark_failed(&self, node: &str) {
        let max_fails = self.cfg.max_fails.max(1) as u32;
        let fail_timeout_ns = self.cfg.fail_timeout;
        let blocked_for = if fail_timeout_ns > 0 {
            Some(Duration::from_nanos(fail_timeout_ns as u64))
        } else {
            None
        };

        let mut states = self.states.lock();
        for s in states.iter_mut() {
            if s.name == node {
                s.fails = s.fails.saturating_add(1);
                if s.fails >= max_fails {
                    if let Some(d) = blocked_for {
                        s.blocked_until = Some(Instant::now() + d);
                        s.fails = 0;
                    }
                }
            }
        }
    }

    fn len(&self) -> usize {
        self.states.lock().len()
    }
}

/// 包装 `Arc<Selector>` 给 chain 使用。
pub type ArcSelector = Arc<dyn Selector>;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_nodes(n: usize) -> Vec<ChainNodeConfig> {
        (0..n)
            .map(|i| ChainNodeConfig {
                name: format!("node_{i}"),
                addr: Some(format!("10.0.0.{i}:80")),
                ..Default::default()
            })
            .collect()
    }

    fn make_cfg(strategy: &str) -> SelectorConfig {
        SelectorConfig {
            strategy: strategy.into(),
            max_fails: 3,
            fail_timeout: 60_000_000_000, // 60s
        }
    }

    #[test]
    fn round_distributes_evenly() {
        let sel = SelectorImpl::new(make_cfg("round"), &make_nodes(3));
        let mut counts = [0u32; 3];
        for _ in 0..9 {
            let (name, _) = sel.select().unwrap();
            let idx: usize = name.trim_start_matches("node_").parse().unwrap();
            counts[idx] += 1;
        }
        for c in counts {
            assert_eq!(c, 3, "round 应均匀分布");
        }
    }

    #[test]
    fn random_covers_all() {
        let sel = SelectorImpl::new(make_cfg("random"), &make_nodes(5));
        let mut seen = std::collections::HashSet::new();
        for _ in 0..50 {
            if let Some((n, _)) = sel.select() {
                seen.insert(n);
            }
        }
        assert!(seen.len() >= 3, "随机选择 50 次应至少看到 3 个节点");
    }

    #[test]
    fn empty_nodes_returns_none() {
        let sel = SelectorImpl::new(make_cfg("round"), &[]);
        assert!(sel.select().is_none());
        assert_eq!(sel.len(), 0);
    }

    #[test]
    fn mark_failed_blocks_node() {
        let sel = SelectorImpl::new(make_cfg("round"), &make_nodes(3));
        for _ in 0..3 {
            sel.mark_failed("node_0");
        }
        for _ in 0..10 {
            let (name, _) = sel.select().unwrap();
            assert_ne!(name, "node_0", "blocked 节点不应被选中");
        }
    }

    #[test]
    fn mark_success_resets_fails() {
        let sel = SelectorImpl::new(make_cfg("round"), &make_nodes(2));
        sel.mark_failed("node_0");
        sel.mark_failed("node_0");
        sel.mark_success("node_0");
        let (name, _) = sel.select().unwrap();
        assert_eq!(name, "node_0");
    }
}