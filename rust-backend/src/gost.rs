use serde_json::{json, Value};

// 服务命名约定 — 节点对账依赖，不可改

pub fn forward_service_base(forward_id: i64, user_id: i64, user_tunnel_id: i64) -> String {
    format!("{}_{}_{}", forward_id, user_id, user_tunnel_id)
}

pub fn forward_tcp(base: &str) -> String {
    format!("{}_tcp", base)
}

pub fn forward_udp(base: &str) -> String {
    format!("{}_udp", base)
}

/// 解析 n 字段: "{forwardId}_{userId}_{userTunnelId}_tcp|udp"
pub fn parse_forward_service_name(n: &str) -> Option<(i64, i64, i64)> {
    if n.is_empty() || n == "web_api" {
        return None;
    }
    let mut base = n;
    if let Some(s) = n.strip_suffix("_tcp") {
        base = s;
    } else if let Some(s) = n.strip_suffix("_udp") {
        base = s;
    }
    let parts: Vec<&str> = base.split('_').collect();
    if parts.len() < 3 {
        return None;
    }
    let p = &parts[parts.len() - 3..];
    let f = p[0].parse::<i64>().ok()?;
    let u = p[1].parse::<i64>().ok()?;
    let t = p[2].parse::<i64>().ok()?;
    Some((f, u, t))
}

pub fn is_missing(msg: &str) -> bool {
    msg.contains("not found")
}

pub fn is_exists(msg: &str) -> bool {
    msg.contains("exists")
}

pub fn is_ok(msg: &str) -> bool {
    msg == "OK" || is_exists(msg)
}

pub fn normalize_ok(msg: &str) -> String {
    if is_exists(msg) {
        "OK".to_string()
    } else {
        msg.to_string()
    }
}

pub fn limiter_data(name: i64, speed: &str) -> Value {
    json!({
        "name": name.to_string(),
        "limits": [format!("$ {}MB {}MB", speed, speed)],
    })
}

pub fn process_server_address(server_addr: &str) -> String {
    if server_addr.is_empty() || server_addr.starts_with('[') {
        return server_addr.to_string();
    }
    let Some(last_colon) = server_addr.rfind(':') else {
        if is_ipv6(server_addr) {
            return format!("[{}]", server_addr);
        }
        return server_addr.to_string();
    };
    let host = &server_addr[..last_colon];
    let port = &server_addr[last_colon..];
    if is_ipv6(host) {
        return format!("[{}]{}", host, port);
    }
    server_addr.to_string()
}

fn is_ipv6(address: &str) -> bool {
    address.matches(':').count() >= 2
}

#[allow(clippy::too_many_arguments)]
pub fn build_forward_services(
    base_name: &str,
    limiter: Option<i64>,
    tcp_listen: &str,
    udp_listen: &str,
    port: i32,
    interface_name: &str,
    remote_addr: &str,
    strategy: &str,
) -> Vec<Value> {
    let mut services = Vec::with_capacity(2);
    for protocol in ["tcp", "udp"] {
        let addr = if protocol == "udp" {
            udp_listen
        } else {
            tcp_listen
        };
        let mut svc = json!({
            "name": format!("{}_{}", base_name, protocol),
            "addr": format!("{}:{}", addr, port),
        });
        if !interface_name.is_empty() {
            svc["metadata"] = json!({ "interface": interface_name });
        }
        if let Some(limiter) = limiter {
            svc["limiter"] = json!(limiter.to_string());
        }
        let handler = json!({ "type": protocol });
        svc["handler"] = handler;

        let mut listener = json!({ "type": protocol });
        if protocol == "udp" {
            listener["metadata"] = json!({ "keepAlive": true });
        }
        svc["listener"] = listener;
        svc["forwarder"] = build_forwarder(remote_addr, strategy);
        services.push(svc);
    }
    services
}

fn build_forwarder(remote_addr: &str, strategy: &str) -> Value {
    let strategy = if strategy.is_empty() { "fifo" } else { strategy };
    let nodes: Vec<Value> = remote_addr
        .split(',')
        .enumerate()
        .map(|(i, addr)| {
            json!({
                "name": format!("node_{}", i + 1),
                "addr": addr.trim(),
            })
        })
        .collect();
    json!({
        "nodes": nodes,
        "selector": {
            "strategy": strategy,
            "maxFails": 1,
            "failTimeout": "600s",
        },
    })
}



pub fn delete_service_payload(names: Vec<String>) -> Value {
    json!({ "services": names })
}

pub fn pause_resume_payload(base_name: &str) -> Value {
    json!({
        "services": [
            format!("{}_tcp", base_name),
            format!("{}_udp", base_name),
        ],
    })
}
