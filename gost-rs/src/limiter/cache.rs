//! 限速器缓存：每个 key 一个 limiter，TTL 过期清理。
//!
//! 与 Go 版 `patricmn/go-cache` 用法对应。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// 带 TTL 的缓存。
pub struct TtlCache<V> {
    map: Mutex<HashMap<String, (V, Instant)>>,
    ttl: Duration,
}

impl<V: Clone> TtlCache<V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        let mut map = self.map.lock();
        if let Some((v, exp)) = map.get(key) {
            if exp > &Instant::now() {
                // 续期
                let v = v.clone();
                map.insert(key.to_string(), (v.clone(), Instant::now() + self.ttl));
                return Some(v);
            }
            map.remove(key);
        }
        None
    }

    pub fn set(&self, key: &str, v: V) {
        self.map
            .lock()
            .insert(key.to_string(), (v, Instant::now() + self.ttl));
    }

    pub fn remove(&self, key: &str) {
        self.map.lock().remove(key);
    }

    /// 清理过期项。返回被移除的数量。
    pub fn cleanup(&self) -> usize {
        let now = Instant::now();
        let mut map = self.map.lock();
        let before = map.len();
        map.retain(|_, (_, exp)| *exp > now);
        before - map.len()
    }

    pub fn len(&self) -> usize {
        self.map.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_remove() {
        let c = TtlCache::new(Duration::from_secs(60));
        c.set("a", 1);
        assert_eq!(c.get("a"), Some(1));
        c.remove("a");
        assert_eq!(c.get("a"), None);
    }

    #[test]
    fn ttl_expires() {
        let c = TtlCache::new(Duration::from_millis(50));
        c.set("a", 1);
        assert_eq!(c.get("a"), Some(1));
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(c.get("a"), None);
    }

    #[test]
    fn cleanup_removes_expired() {
        let c = TtlCache::new(Duration::from_millis(30));
        c.set("a", 1);
        c.set("b", 2);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(c.cleanup(), 2);
        assert_eq!(c.len(), 0);
    }
}