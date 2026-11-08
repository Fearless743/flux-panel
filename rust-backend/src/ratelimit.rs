use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

/// 极简固定窗口限流（面板量级够用，无需外部依赖）。
/// key 通常为 "scope:ip"，窗口内超过 max 即拒绝。
struct Bucket {
    count: u32,
    window_start: Instant,
}

static BUCKETS: LazyLock<Mutex<HashMap<String, Bucket>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const MAX_BUCKETS: usize = 8192;

fn check(key: &str, max: u32, window_secs: u64) -> bool {
    let now = Instant::now();
    let mut m = BUCKETS.lock().unwrap();
    if m.len() > MAX_BUCKETS {
        m.clear();
    }
    match m.get_mut(key) {
        Some(b) => {
            if now.duration_since(b.window_start).as_secs() >= window_secs {
                b.count = 1;
                b.window_start = now;
                true
            } else if b.count < max {
                b.count += 1;
                true
            } else {
                false
            }
        }
        None => {
            m.insert(
                key.to_string(),
                Bucket {
                    count: 1,
                    window_start: now,
                },
            );
            true
        }
    }
}

pub fn check_login(ip: &str) -> bool {
    check(&format!("login:{ip}"), 10, 60)
}

pub fn check_captcha(ip: &str) -> bool {
    check(&format!("captcha:{ip}"), 60, 60)
}

pub fn check_openapi(ip: &str) -> bool {
    check(&format!("openapi:{ip}"), 60, 60)
}

pub fn check_flow(secret: &str) -> bool {
    check(&format!("flow:{secret}"), 600, 60)
}

pub fn check_diagnose(user_id: i64) -> bool {
    check(&format!("diagnose:{user_id}"), 10, 60)
}

/// 反代后真实 IP：优先 X-Forwarded-For 首个，其次 X-Real-IP
pub fn client_ip(headers: &axum::http::HeaderMap) -> String {
    if let Some(v) = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
    {
        let first = v.split(',').next().unwrap_or("").trim();
        if !first.is_empty() {
            return first.to_string();
        }
    }
    if let Some(v) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
        let v = v.trim();
        if !v.is_empty() {
            return v.to_string();
        }
    }
    "unknown".to_string()
}
