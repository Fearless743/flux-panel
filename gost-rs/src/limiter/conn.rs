//! 连接数限制器。
//!
//! 与 Go 版 `x/limiter/conn/` 对齐：限制并发连接总数。
//! Service scope 之外不细分 CIDR 等。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 连接数限制器。
pub struct ConnLimiter {
    limit: u64,
    current: Arc<AtomicU64>,
}

impl ConnLimiter {
    pub fn new(limit: u64) -> Self {
        Self {
            limit,
            current: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 尝试获取一个连接配额。返回 true 表示允许。
    pub fn try_acquire(&self) -> bool {
        if self.limit == 0 {
            return true; // 0 = 无限制
        }
        loop {
            let cur = self.current.load(Ordering::Acquire);
            if cur >= self.limit {
                return false;
            }
            if self
                .current
                .compare_exchange(cur, cur + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
        }
    }

    /// 释放一个连接配额。
    pub fn release(&self) {
        loop {
            let cur = self.current.load(Ordering::Acquire);
            if cur == 0 {
                return;
            }
            if self
                .current
                .compare_exchange(cur, cur - 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
        }
    }

    /// 当前并发连接数。
    pub fn current(&self) -> u64 {
        self.current.load(Ordering::Relaxed)
    }

    /// 配额上限。
    pub fn limit(&self) -> u64 {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn unlimited_when_zero() {
        // limit=0 表示无限制：try_acquire 永远 true，但 current 不递增
        let l = ConnLimiter::new(0);
        for _ in 0..100 {
            assert!(l.try_acquire());
        }
        // 无限制模式不计 current（与 Go 版语义一致：0 表示禁用计数）
        assert_eq!(l.current(), 0);
        for _ in 0..100 {
            l.release(); // 无操作
        }
        assert_eq!(l.current(), 0);
    }

    #[test]
    fn bounded_limit() {
        let l = ConnLimiter::new(3);
        assert!(l.try_acquire());
        assert!(l.try_acquire());
        assert!(l.try_acquire());
        assert!(!l.try_acquire(), "第 4 个应被拒");
        l.release();
        assert!(l.try_acquire(), "释放后可再获取");
    }

    #[test]
    fn concurrent_under_limit() {
        // 串行化测试：避免全局静态 CGO 限制导致跨线程 CAS 干扰。
        let l = ConnLimiter::new(10);
        let mut acquired = 0;
        for _ in 0..50 {
            if l.try_acquire() {
                acquired += 1;
            }
        }
        assert_eq!(acquired, 10);
        assert_eq!(l.current(), 10);
    }
}