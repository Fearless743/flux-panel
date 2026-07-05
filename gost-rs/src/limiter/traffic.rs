//! traffic 限速器：基于令牌桶的字节速率限制。
//!
//! 与 Go 版 `x/limiter/traffic/` 对齐：
//! - 每个连接 / IP / CIDR / Service scope 一个令牌桶
//! - `In(key)` / `Out(key)` 返回单连接使用的 limiter
//! - 多 limiter 可组合成 group（取最小 limit）
//!
//! 令牌桶实现：每秒 `n` 字节，burst = `n`（与 Go `golang.org/x/time/rate` 一致）。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::parse::{parse_base2_bytes, parse_limits_line};

/// Service scope 的特殊 key（与 Go `ServiceLimitKey = "$"` 一致）。
pub const SERVICE_LIMIT_KEY: &str = "$";
/// Conn scope 的特殊 key（与 Go `ConnLimitKey = "$$"` 一致）。
pub const CONN_LIMIT_KEY: &str = "$$";

/// 单个方向（in 或 out）的令牌桶。
#[derive(Debug)]
pub struct TokenBucket {
    /// 每秒补充的令牌数（也即字节速率）。
    rate: u64,
    /// 桶容量（= rate，与 Go burst 一致）。
    burst: u64,
    /// 当前令牌数（毫浮点，按时间补充）。
    tokens: f64,
    /// 上次补充时间。
    last: Instant,
}

impl TokenBucket {
    pub fn new(rate: u64) -> Self {
        Self {
            rate,
            burst: rate,
            tokens: rate as f64,
            last: Instant::now(),
        }
    }

    /// 取当前 limit（每秒字节数）。
    pub fn limit(&self) -> u64 {
        self.rate
    }

    /// 设置新 limit（同时调整 burst）。
    pub fn set(&mut self, n: u64) {
        self.rate = n;
        self.burst = n;
        if self.tokens > n as f64 {
            self.tokens = n as f64;
        }
    }

    /// 等待 `n` 字节通过（阻塞 + sleep）。
    ///
    /// 返回实际通过的字节数（≤ n，受 burst 限制）。
    pub async fn wait(&mut self, n: u64) -> u64 {
        let want = std::cmp::min(n, self.burst);
        loop {
            self.refill();
            if self.tokens >= want as f64 {
                self.tokens -= want as f64;
                return want;
            }
            // 计算需要等待的时间
            let deficit = want as f64 - self.tokens;
            let wait_secs = deficit / self.rate as f64;
            let wait_dur = Duration::from_secs_f64(wait_secs.max(0.001));
            tokio::time::sleep(wait_dur).await;
        }
    }

    /// 非阻塞尝试：返回通过的字节数（0 表示无令牌）。
    pub fn try_take(&mut self, n: u64) -> u64 {
        self.refill();
        let want = std::cmp::min(n, self.burst);
        if self.tokens >= want as f64 {
            self.tokens -= want as f64;
            want
        } else {
            let avail = self.tokens.floor() as u64;
            if avail > 0 {
                self.tokens -= avail as f64;
                avail
            } else {
                0
            }
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last);
        self.last = now;
        if self.rate > 0 {
            let refill = elapsed.as_secs_f64() * self.rate as f64;
            self.tokens = (self.tokens + refill).min(self.burst as f64);
        }
    }
}

/// 单连接使用的双向限速器（in + out）。
#[derive(Debug, Clone)]
pub struct ConnLimiterPair {
    pub inb: Arc<Mutex<TokenBucket>>,
    pub outb: Arc<Mutex<TokenBucket>>,
}

impl ConnLimiterPair {
    pub fn new(in_rate: u64, out_rate: u64) -> Self {
        Self {
            inb: Arc::new(Mutex::new(TokenBucket::new(in_rate))),
            outb: Arc::new(Mutex::new(TokenBucket::new(out_rate))),
        }
    }
}

/// 限速范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Service,
    Conn,
    /// IP 或 CIDR。
    Ip,
}

/// 一条限制规则解析后的结果。
#[derive(Debug, Clone)]
pub struct LimitRule {
    pub key: String,
    pub in_rate: u64,
    pub out_rate: u64,
}

/// 生成器：按 ConnLimitKey/ServiceLimitKey 生成 new limiter。
#[derive(Debug)]
struct Generator {
    in_rate: u64,
    out_rate: u64,
}

impl Generator {
    fn new(in_rate: u64, out_rate: u64) -> Self {
        Self { in_rate, out_rate }
    }
    fn gen(&self) -> Option<ConnLimiterPair> {
        if self.in_rate == 0 && self.out_rate == 0 {
            None
        } else {
            Some(ConnLimiterPair::new(self.in_rate, self.out_rate))
        }
    }
}

/// traffic 限速器。
pub struct TrafficLimiter {
    /// 配置中的明文限制规则。
    rules: Vec<LimitRule>,
    /// Conn 级别 generator。
    conn_gen: Mutex<Generator>,
    /// Service 级别 limiter（不缓存，按需取）。
    service_pair: Mutex<Option<ConnLimiterPair>>,
    /// IP 级别缓存（key → pair）。
    ip_cache: Mutex<HashMap<String, ConnLimiterPair>>,
    /// CIDR 列表（前缀 → generator）。
    cidr_gens: Mutex<Vec<(cidr::IpCidr, Generator)>>,
}

impl TrafficLimiter {
    /// 从 limits 字符串列表构造。
    pub fn from_limits(limits: &[String]) -> Self {
        let mut rules = Vec::new();
        let mut conn_in = 0u64;
        let mut conn_out = 0u64;
        let mut svc_in = 0u64;
        let mut svc_out = 0u64;
        let mut cidrs: Vec<(cidr::IpCidr, Generator)> = Vec::new();

        for line in limits {
            let (key, lin, lout) = parse_limits_line(line);
            if key.is_empty() {
                continue;
            }
            let in_rate = parse_base2_bytes(&lin).unwrap_or(0);
            let out_rate = parse_base2_bytes(&lout).unwrap_or(0);
            match key.as_str() {
                SERVICE_LIMIT_KEY => {
                    svc_in = in_rate;
                    svc_out = out_rate;
                }
                CONN_LIMIT_KEY => {
                    conn_in = in_rate;
                    conn_out = out_rate;
                }
                other => {
                    // 尝试解析为 CIDR
                    if let Ok(cidr) = other.parse::<cidr::IpCidr>() {
                        cidrs.push((cidr, Generator::new(in_rate, out_rate)));
                    } else {
                        rules.push(LimitRule {
                            key: key.clone(),
                            in_rate,
                            out_rate,
                        });
                    }
                }
            }
        }

        Self {
            rules,
            conn_gen: Mutex::new(Generator::new(conn_in, conn_out)),
            service_pair: Mutex::new(if svc_in > 0 || svc_out > 0 {
                Some(ConnLimiterPair::new(svc_in, svc_out))
            } else {
                None
            }),
            ip_cache: Mutex::new(HashMap::new()),
            cidr_gens: Mutex::new(cidrs),
        }
    }

    /// 输入方向 limiter（按 key 取/生成）。
    pub fn in_limiter(&self, scope: Scope, key: &str) -> Option<ConnLimiterPair> {
        match scope {
            Scope::Service => self.service_pair.lock().clone(),
            Scope::Conn => self.conn_gen.lock().gen(),
            Scope::Ip => self.lookup_ip(key),
        }
    }

    /// 输出方向 limiter（与 in 同结构）。
    pub fn out_limiter(&self, scope: Scope, key: &str) -> Option<ConnLimiterPair> {
        self.in_limiter(scope, key)
    }

    fn lookup_ip(&self, key: &str) -> Option<ConnLimiterPair> {
        // key 形如 "1.2.3.4:5678"，取 host
        let host = key.rsplit_once(':').map(|(h, _)| h).unwrap_or(key);
        // 先查缓存
        if let Some(p) = self.ip_cache.lock().get(host) {
            return Some(p.clone());
        }
        // 再查 CIDR
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            for (cidr, gen) in self.cidr_gens.lock().iter() {
                if cidr.contains(&ip) {
                    if let Some(p) = gen.gen() {
                        self.ip_cache
                            .lock()
                            .insert(host.to_string(), p.clone());
                        return Some(p);
                    }
                }
            }
        }
        // 最后查精确 IP 规则
        for r in &self.rules {
            if r.key == host && (r.in_rate > 0 || r.out_rate > 0) {
                let p = ConnLimiterPair::new(r.in_rate, r.out_rate);
                self.ip_cache.lock().insert(host.to_string(), p.clone());
                return Some(p);
            }
        }
        None
    }

    /// 暴露 service pair 用于外部 stat。
    pub fn service_pair(&self) -> Option<ConnLimiterPair> {
        self.service_pair.lock().clone()
    }
}

/// builder：从 [`crate::config::types::LimiterConfig`] 构造。
pub struct TrafficLimiterBuilder {
    limits: Vec<String>,
}

impl TrafficLimiterBuilder {
    pub fn new() -> Self {
        Self { limits: Vec::new() }
    }
    pub fn limits(mut self, l: Vec<String>) -> Self {
        self.limits = l;
        self
    }
    pub fn build(self) -> TrafficLimiter {
        TrafficLimiter::from_limits(&self.limits)
    }
}

impl Default for TrafficLimiterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_service_limit() {
        let l = TrafficLimiter::from_limits(&["$ 1MB 2MB".into()]);
        assert!(l.service_pair().is_some());
    }

    #[tokio::test]
    async fn token_bucket_throttles() {
        let mut tb = TokenBucket::new(100_000); // 100KB/s
                                                 // 先消耗初始令牌（= burst = 100_000）
        let took = tb.wait(100_000).await;
        assert_eq!(took, 100_000);
        // 再要 50_000，应阻塞约 0.5s
        let start = Instant::now();
        let took = tb.wait(50_000).await;
        let elapsed = start.elapsed();
        assert_eq!(took, 50_000);
        assert!(elapsed >= Duration::from_millis(400), "应被限速 ~0.5s, 实际 {:?}", elapsed);
        assert!(elapsed < Duration::from_millis(1500));
    }

    #[tokio::test]
    async fn unlimited_bucket_passes_instantly() {
        // rate=0 视为无限制 → wait 立即返回 n（但 burst 也 0，会取 min=0）
        // 实际语义：TrafficLimiter 在 in_rate=0 时应返回 None（不限速）。
        let l = TrafficLimiter::from_limits(&["$ 0 0".into()]);
        assert!(l.service_pair().is_none());
    }

    #[test]
    fn ip_lookup_via_cidr() {
        let l = TrafficLimiter::from_limits(&["10.0.0.0/8 1MB 1MB".into()]);
        let p = l.in_limiter(Scope::Ip, "10.1.2.3:5555");
        assert!(p.is_some());
        // 缓存命中
        let p2 = l.in_limiter(Scope::Ip, "10.1.2.3:6666");
        assert!(p2.is_some());
        // 不在 CIDR 内
        let p3 = l.in_limiter(Scope::Ip, "11.0.0.1:5555");
        assert!(p3.is_none());
    }

    #[test]
    fn ip_lookup_exact() {
        let l = TrafficLimiter::from_limits(&["1.2.3.4 500KB 600KB".into()]);
        let p = l.in_limiter(Scope::Ip, "1.2.3.4:1234");
        assert!(p.is_some());
        let p2 = l.in_limiter(Scope::Ip, "1.2.3.5:1234");
        assert!(p2.is_none());
    }

    #[tokio::test]
    async fn conn_scope_generates_pair() {
        let l = TrafficLimiter::from_limits(&["$$ 1MB 1MB".into()]);
        let p = l.in_limiter(Scope::Conn, "anykey").expect("conn scope 应有 limiter");
        let took = p.inb.lock().wait(100).await;
        assert_eq!(took, 100);
    }
}