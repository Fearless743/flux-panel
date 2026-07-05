//! 系统信息（uptime/cpu/mem）。
//!
//! 与 Go 版 `x/internal/util/system/` 对齐，包装 `sysinfo` crate。
//!
//! Java 端 GetConfig 响应附带节点信息时，会附带：
//! - uptime（秒）
//! - cpu 使用率（百分比）
//! - 内存总量 / 已用（字节）

use std::sync::Arc;
use std::time::SystemTime;

#[cfg(test)]
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sysinfo::System;

/// 系统快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    /// 进程启动至今的秒数（uptime）。
    pub uptime: u64,
    /// CPU 总使用率（百分比，0-100）。
    pub cpu_usage: f32,
    /// 内存总量（字节）。
    pub mem_total: u64,
    /// 内存已用（字节）。
    pub mem_used: u64,
    /// 时间戳（毫秒）。
    pub timestamp_ms: i64,
}

/// 全局 Sysinfo 状态（一次性 refresh，复用）。
pub struct SysinfoState {
    sys: Arc<parking_lot::Mutex<System>>,
    started: SystemTime,
}

impl SysinfoState {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys: Arc::new(parking_lot::Mutex::new(sys)),
            started: SystemTime::now(),
        }
    }

    /// 取当前快照。
    pub fn snapshot(&self) -> SystemSnapshot {
        let mut sys = self.sys.lock();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let cpu = sys.global_cpu_usage();
        let uptime = SystemTime::now()
            .duration_since(self.started)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mem_total = sys.total_memory();
        let mem_used = sys.used_memory();
        let ts = chrono::Utc::now().timestamp_millis();
        SystemSnapshot {
            uptime,
            cpu_usage: cpu,
            mem_total,
            mem_used,
            timestamp_ms: ts,
        }
    }
}

impl Default for SysinfoState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_valid() {
        let s = SysinfoState::new();
        let snap = s.snapshot();
        assert!(snap.uptime < 3600 * 24);
        assert!(snap.cpu_usage >= 0.0);
        assert!(snap.mem_total > 0);
        assert!(snap.timestamp_ms > 0);
    }

    #[test]
    fn snapshot_serde() {
        let s = SysinfoState::new();
        let snap = s.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let back: SystemSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.uptime, snap.uptime);
    }

    #[test]
    fn started_before_now() {
        let s = SysinfoState::new();
        let snap = s.snapshot();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        assert!(snap.timestamp_ms as u64 <= now.as_millis() as u64);
    }
}