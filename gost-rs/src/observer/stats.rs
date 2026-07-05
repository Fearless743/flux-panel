//! Stats observer：聚合每个连接/service/handler 的统计事件。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatsEvent {
    pub service: String,
    pub client: String,
    pub kind: String, // "input_bytes" / "output_bytes" / "total_conns" / "current_conns"
    pub value: i64,
    pub timestamp_ms: i64,
}

/// 简单全局收集器（线程安全）。
#[derive(Default)]
pub struct StatsObserver {
    pub events: parking_lot::Mutex<Vec<StatsEvent>>,
}

impl StatsObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&self, event: StatsEvent) {
        self.events.lock().push(event);
    }

    pub fn drain(&self) -> Vec<StatsEvent> {
        std::mem::take(&mut *self.events.lock())
    }

    pub fn len(&self) -> usize {
        self.events.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_drain() {
        let o = StatsObserver::new();
        o.observe(StatsEvent {
            service: "s1".into(),
            client: "c1".into(),
            kind: "input_bytes".into(),
            value: 100,
            timestamp_ms: 0,
        });
        o.observe(StatsEvent {
            service: "s1".into(),
            client: "c1".into(),
            kind: "input_bytes".into(),
            value: 50,
            timestamp_ms: 0,
        });
        let drained = o.drain();
        assert_eq!(drained.len(), 2);
        assert!(o.is_empty());
    }
}
