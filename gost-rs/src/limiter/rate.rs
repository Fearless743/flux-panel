//! 请求速率限制器。
//!
//! 与 Go 版 `x/limiter/rate/` 对齐：限制每秒请求数。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use parking_lot::Mutex;

/// 请求速率限制器（每秒 N 次）。
pub struct RateLimiter {
    rate: u64,
    inner: Mutex<Inner>,
}

struct Inner {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(rate: u64) -> Self {
        Self {
            rate,
            inner: Mutex::new(Inner {
                tokens: rate as f64,
                last: Instant::now(),
            }),
        }
    }

    /// 非阻塞尝试：返回是否允许 1 次请求。
    pub fn allow(&self) -> bool {
        if self.rate == 0 {
            return true;
        }
        let mut inner = self.inner.lock();
        let now = Instant::now();
        let elapsed = now.duration_since(inner.last);
        inner.last = now;
        inner.tokens = (inner.tokens + elapsed.as_secs_f64() * self.rate as f64)
            .min(self.rate as f64);
        if inner.tokens >= 1.0 {
            inner.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn rate(&self) -> u64 {
        self.rate
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self::new(self.rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn allow_within_rate() {
        let r = RateLimiter::new(5);
        for _ in 0..5 {
            assert!(r.allow());
        }
    }

    #[test]
    fn exceeds_rate_rejected() {
        let r = RateLimiter::new(2);
        assert!(r.allow());
        assert!(r.allow());
        assert!(!r.allow(), "第 3 个应被拒");
    }

    #[test]
    fn refills_after_wait() {
        let r = RateLimiter::new(2);
        assert!(r.allow());
        assert!(r.allow());
        assert!(!r.allow());
        thread::sleep(Duration::from_millis(600));
        assert!(r.allow(), "0.6s 后应已补充 1 个令牌");
    }

    #[test]
    fn zero_means_unlimited() {
        let r = RateLimiter::new(0);
        for _ in 0..1000 {
            assert!(r.allow());
        }
    }
}

/// 全局计数器（用于 stat 报告）。
pub struct Counter {
    n: AtomicU64,
}

impl Counter {
    pub fn new() -> Self {
        Self { n: AtomicU64::new(0) }
    }
    pub fn inc(&self) {
        self.n.fetch_add(1, Ordering::Relaxed);
    }
    pub fn get(&self) -> u64 {
        self.n.load(Ordering::Relaxed)
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}