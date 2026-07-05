//! Service 运行状态。

use serde::{Deserialize, Serialize};

/// 服务事件（创建/暂停/恢复/错误等）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceEvent {
    pub time: i64,
    pub msg: String,
}

/// 服务统计。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceStats {
    #[serde(rename = "totalConns", default)]
    pub total_conns: u64,
    #[serde(rename = "currentConns", default)]
    pub current_conns: u64,
    #[serde(rename = "totalErrs", default)]
    pub total_errs: u64,
    #[serde(rename = "inputBytes", default)]
    pub input_bytes: u64,
    #[serde(rename = "outputBytes", default)]
    pub output_bytes: u64,
}

/// 服务状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceStatus {
    #[serde(rename = "createTime")]
    pub create_time: i64,
    pub state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<ServiceEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<ServiceStats>,
}

impl ServiceStatus {
    pub fn new_running() -> Self {
        Self {
            create_time: chrono::Utc::now().timestamp_millis(),
            state: "running".into(),
            events: vec![ServiceEvent {
                time: chrono::Utc::now().timestamp_millis(),
                msg: "service created".into(),
            }],
            stats: Some(ServiceStats::default()),
        }
    }

    pub fn event(&mut self, msg: impl Into<String>) {
        self.events.push(ServiceEvent {
            time: chrono::Utc::now().timestamp_millis(),
            msg: msg.into(),
        });
    }

    pub fn set_state(&mut self, state: impl Into<String>) {
        self.state = state.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serde_round_trip() {
        let mut s = ServiceStatus::new_running();
        s.event("paused by user");
        s.set_state("paused");
        let json = serde_json::to_string(&s).unwrap();
        let parsed: ServiceStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.state, "paused");
        assert_eq!(parsed.events.len(), 2);
    }
}