//! Prometheus 风格的指标注册表。

use prometheus::{
    register_counter_vec_with_registry, register_gauge_vec_with_registry, register_histogram_vec_with_registry,
    CounterVec, GaugeVec, HistogramVec, Registry,
};

/// 全局指标注册表（与 Go 版 `x/metrics/metrics.go` 对齐）。
#[derive(Clone)]
pub struct MetricsRegistry {
    pub registry: Registry,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            registry: Registry::new(),
        }
    }

    pub fn counter(&self, name: &str, labels: &[&str]) -> CounterVec {
        register_counter_vec_with_registry!(name, name, labels, &self.registry).unwrap()
    }

    pub fn gauge(&self, name: &str, labels: &[&str]) -> GaugeVec {
        register_gauge_vec_with_registry!(name, name, labels, &self.registry).unwrap()
    }

    pub fn histogram(&self, name: &str, labels: &[&str]) -> HistogramVec {
        register_histogram_vec_with_registry!(name, name, labels, &self.registry).unwrap()
    }

    /// 导出 prometheus text format。
    pub fn encode(&self) -> String {
        use prometheus::Encoder;
        let metric_families = self.registry.gather();
        let mut buf = Vec::new();
        let encoder = prometheus::TextEncoder::new();
        encoder.encode(&metric_families, &mut buf).unwrap();
        String::from_utf8(buf).unwrap_or_default()
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局默认 MetricsRegistry。
pub struct Metrics;

impl Metrics {
    pub fn registry() -> &'static std::sync::Mutex<MetricsRegistry> {
        use once_cell::sync::Lazy;
        static M: Lazy<std::sync::Mutex<MetricsRegistry>> =
            Lazy::new(|| std::sync::Mutex::new(MetricsRegistry::new()));
        &M
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_basic() {
        let r = MetricsRegistry::new();
        let c = r.counter("test_counter", &["kind"]);
        c.with_label_values(&["a"]).inc();
        let out = r.encode();
        assert!(out.contains("test_counter"));
        assert!(out.contains("kind=\"a\""));
    }

    #[test]
    fn gauge_and_histogram() {
        let r = MetricsRegistry::new();
        let g = r.gauge("test_gauge", &["kind"]);
        g.with_label_values(&["a"]).set(42.0);
        let h = r.histogram("test_histogram", &["kind"]);
        h.with_label_values(&["a"]).observe(1.5);
        let out = r.encode();
        assert!(out.contains("test_gauge"));
        assert!(out.contains("42"));
        assert!(out.contains("test_histogram"));
    }
}
